/// DTLS Keylog Service
///
/// This module provides storage and retrieval of DTLS encryption keys
/// for decryption of captured packets in Wireshark.
/// 
/// Keys are stored in the SSLKEYLOGFILE format:
/// CLIENT_RANDOM <client_random_hex> <master_secret_hex>
///
/// This format is compatible with Wireshark's "Pre-Master-Secret log filename"
/// feature for decrypting DTLS traffic.

use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;

/// DTLS key log entry containing the data needed for Wireshark decryption
#[derive(Clone, Debug)]
pub struct DtlsKeylogEntry {
    /// Client random bytes (32 bytes, displayed as hex)
    pub client_random: Vec<u8>,
    /// Master secret bytes (48 bytes, displayed as hex)
    pub master_secret: Vec<u8>,
    /// Timestamp when this key was recorded
    pub timestamp: std::time::SystemTime,
}

impl DtlsKeylogEntry {
    /// Create a new keylog entry
    pub fn new(client_random: Vec<u8>, master_secret: Vec<u8>) -> Self {
        Self {
            client_random,
            master_secret,
            timestamp: std::time::SystemTime::now(),
        }
    }
    
    /// Format the entry as SSLKEYLOGFILE line
    /// Uses lowercase hex encoding which is compatible with Wireshark
    pub fn to_sslkeylog_line(&self) -> String {
        let client_random_hex = hex::encode(&self.client_random);
        let master_secret_hex = hex::encode(&self.master_secret);
        format!("CLIENT_RANDOM {} {}", client_random_hex, master_secret_hex)
    }
}

/// Configuration for DTLS keylog service
#[derive(Clone, Debug)]
pub struct DtlsKeylogConfig {
    /// Maximum number of sessions to store keys for
    pub max_sessions: usize,
    /// Enable keylog storage
    pub enabled: bool,
}

impl Default for DtlsKeylogConfig {
    fn default() -> Self {
        Self {
            max_sessions: 1000,
            enabled: true,
        }
    }
}

/// Storage for DTLS keylogs organized by survey session
struct KeylogStorage {
    /// Map of survey_session_id -> Vec of keylog entries
    /// Each session may have multiple entries (one per WebRTC connection)
    sessions: HashMap<String, Vec<DtlsKeylogEntry>>,
    /// Order of sessions for LRU eviction (oldest first)
    session_order: Vec<String>,
    /// Configuration
    config: DtlsKeylogConfig,
}

impl KeylogStorage {
    fn new(config: DtlsKeylogConfig) -> Self {
        Self {
            sessions: HashMap::new(),
            session_order: Vec::new(),
            config,
        }
    }
    
    /// Add a keylog entry for a survey session
    fn add_entry(&mut self, survey_session_id: String, entry: DtlsKeylogEntry) {
        if survey_session_id.is_empty() {
            tracing::debug!("Skipping DTLS keylog storage for empty session ID");
            return;
        }
        
        // Check if this is a new session
        if !self.sessions.contains_key(&survey_session_id) {
            // Evict oldest session if we've reached the limit
            while self.sessions.len() >= self.config.max_sessions && !self.session_order.is_empty() {
                let oldest = self.session_order.remove(0);
                self.sessions.remove(&oldest);
                tracing::debug!("Evicted oldest DTLS keylog session: {}", oldest);
            }
            
            self.session_order.push(survey_session_id.clone());
        }
        
        // Add entry to the session
        self.sessions
            .entry(survey_session_id.clone())
            .or_insert_with(Vec::new)
            .push(entry);
        
        tracing::debug!(
            "Added DTLS keylog entry for session {} (total entries for session: {})",
            survey_session_id,
            self.sessions.get(&survey_session_id).map(|v| v.len()).unwrap_or(0)
        );
    }
    
    /// Get all keylog entries for a survey session
    fn get_entries(&self, survey_session_id: &str) -> Option<&Vec<DtlsKeylogEntry>> {
        self.sessions.get(survey_session_id)
    }
    
    /// Clear all stored keylogs
    fn clear(&mut self) {
        self.sessions.clear();
        self.session_order.clear();
    }
    
    /// Get statistics
    fn stats(&self) -> KeylogStats {
        let total_entries: usize = self.sessions.values().map(|v| v.len()).sum();
        KeylogStats {
            sessions_stored: self.sessions.len(),
            total_entries,
            max_sessions: self.config.max_sessions,
        }
    }
}

/// Statistics about stored keylogs
#[derive(Clone, Debug, serde::Serialize)]
pub struct KeylogStats {
    /// Number of sessions with stored keylogs
    pub sessions_stored: usize,
    /// Total number of keylog entries
    pub total_entries: usize,
    /// Maximum sessions that can be stored
    pub max_sessions: usize,
}

/// Thread-safe DTLS keylog service
pub struct DtlsKeylogService {
    /// Storage protected by RwLock
    storage: RwLock<KeylogStorage>,
    /// Configuration
    config: DtlsKeylogConfig,
}

impl DtlsKeylogService {
    /// Create a new DTLS keylog service
    pub fn new(config: DtlsKeylogConfig) -> Arc<Self> {
        Arc::new(Self {
            storage: RwLock::new(KeylogStorage::new(config.clone())),
            config,
        })
    }
    
    /// Check if keylog storage is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }
    
    /// Add a DTLS keylog entry for a survey session
    pub fn add_keylog(&self, survey_session_id: String, client_random: Vec<u8>, master_secret: Vec<u8>) {
        if !self.config.enabled {
            return;
        }
        
        let entry = DtlsKeylogEntry::new(client_random, master_secret);
        self.storage.write().add_entry(survey_session_id, entry);
    }
    
    /// Get keylog entries for a survey session
    pub fn get_keylogs(&self, survey_session_id: &str) -> Vec<DtlsKeylogEntry> {
        self.storage
            .read()
            .get_entries(survey_session_id)
            .cloned()
            .unwrap_or_default()
    }
    
    /// Generate SSLKEYLOGFILE content for a survey session
    pub fn generate_keylog_file(&self, survey_session_id: &str) -> String {
        let entries = self.get_keylogs(survey_session_id);
        
        if entries.is_empty() {
            return String::new();
        }
        
        entries
            .iter()
            .map(|e| e.to_sslkeylog_line())
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    }
    
    /// Clear all stored keylogs
    pub fn clear(&self) {
        self.storage.write().clear();
    }
    
    /// Get statistics about stored keylogs
    pub fn stats(&self) -> KeylogStats {
        self.storage.read().stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_keylog_entry_format() {
        let entry = DtlsKeylogEntry::new(
            vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
                 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10,
                 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
                 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x20],
            vec![0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x11,
                 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99,
                 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x11,
                 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99,
                 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x11,
                 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99],
        );
        
        let line = entry.to_sslkeylog_line();
        assert!(line.starts_with("CLIENT_RANDOM "));
        assert!(line.contains(" "));
        
        // Verify hex encoding
        let parts: Vec<&str> = line.split(' ').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "CLIENT_RANDOM");
        assert_eq!(parts[1].len(), 64); // 32 bytes * 2 hex chars
        assert_eq!(parts[2].len(), 96); // 48 bytes * 2 hex chars
    }
    
    #[test]
    fn test_keylog_service() {
        let config = DtlsKeylogConfig {
            max_sessions: 10,
            enabled: true,
        };
        let service = DtlsKeylogService::new(config);
        
        let client_random = vec![0u8; 32];
        let master_secret = vec![0xaau8; 48];
        
        service.add_keylog(
            "session-1".to_string(),
            client_random.clone(),
            master_secret.clone(),
        );
        
        let entries = service.get_keylogs("session-1");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].client_random, client_random);
        assert_eq!(entries[0].master_secret, master_secret);
        
        // Test file generation
        let file_content = service.generate_keylog_file("session-1");
        assert!(file_content.starts_with("CLIENT_RANDOM"));
        assert!(file_content.ends_with("\n"));
        
        // Test stats
        let stats = service.stats();
        assert_eq!(stats.sessions_stored, 1);
        assert_eq!(stats.total_entries, 1);
    }
    
    #[test]
    fn test_keylog_service_disabled() {
        let config = DtlsKeylogConfig {
            max_sessions: 10,
            enabled: false,
        };
        let service = DtlsKeylogService::new(config);
        
        service.add_keylog(
            "session-1".to_string(),
            vec![0u8; 32],
            vec![0xaau8; 48],
        );
        
        let entries = service.get_keylogs("session-1");
        assert!(entries.is_empty());
    }
    
    #[test]
    fn test_keylog_service_eviction() {
        let config = DtlsKeylogConfig {
            max_sessions: 2,
            enabled: true,
        };
        let service = DtlsKeylogService::new(config);
        
        // Add 3 sessions (should evict oldest)
        service.add_keylog("session-1".to_string(), vec![0u8; 32], vec![0xaau8; 48]);
        service.add_keylog("session-2".to_string(), vec![0u8; 32], vec![0xbbu8; 48]);
        service.add_keylog("session-3".to_string(), vec![0u8; 32], vec![0xccu8; 48]);
        
        // session-1 should have been evicted
        let entries1 = service.get_keylogs("session-1");
        assert!(entries1.is_empty());
        
        // session-2 and session-3 should still exist
        let entries2 = service.get_keylogs("session-2");
        assert_eq!(entries2.len(), 1);
        
        let entries3 = service.get_keylogs("session-3");
        assert_eq!(entries3.len(), 1);
    }
}
