//! Authenticated address cache for iperf3 access control.
//!
//! This module maintains a cache of recently authenticated IP addresses
//! with timestamps and user information. The cache is updated from
//! HTTP/HTTPS authentication events and consulted by the iperf3 server.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Information about an authenticated address
#[derive(Clone, Debug)]
pub struct AuthenticatedAddress {
    /// The IP address
    pub ip: IpAddr,
    /// User identifier (handle/email)
    pub user_id: String,
    /// Display name if available
    pub display_name: Option<String>,
    /// When this address was last authenticated
    pub last_authenticated: Instant,
    /// Source of authentication (e.g., "oauth", "magic_key", "webrtc")
    pub auth_source: String,
}

/// Cache of recently authenticated addresses
/// 
/// This cache is designed to be:
/// - Updated from HTTP/HTTPS authentication requests
/// - Consulted synchronously by the iperf3 auth callback
/// - Thread-safe using std::sync::RwLock for sync access
pub struct AuthAddressCache {
    /// The cache: IP -> AuthenticatedAddress
    cache: RwLock<HashMap<IpAddr, AuthenticatedAddress>>,
    /// How long an address remains valid after last authentication
    timeout: Duration,
}

impl AuthAddressCache {
    /// Create a new cache with the specified timeout
    pub fn new(timeout_secs: u64) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            timeout: Duration::from_secs(timeout_secs),
        }
    }

    /// Record an authenticated address
    pub fn record_auth(&self, ip: IpAddr, user_id: String, display_name: Option<String>, auth_source: String) {
        let entry = AuthenticatedAddress {
            ip,
            user_id: user_id.clone(),
            display_name: display_name.clone(),
            last_authenticated: Instant::now(),
            auth_source: auth_source.clone(),
        };

        if let Ok(mut cache) = self.cache.write() {
            tracing::debug!(
                "Recording authenticated address: {} for user '{}' via {}",
                ip, user_id, auth_source
            );
            cache.insert(ip, entry);
        }
    }

    /// Refresh an existing authenticated address (update timestamp only)
    pub fn refresh_auth(&self, ip: IpAddr) -> bool {
        if let Ok(mut cache) = self.cache.write() {
            if let Some(entry) = cache.get_mut(&ip) {
                entry.last_authenticated = Instant::now();
                return true;
            }
        }
        false
    }

    /// Check if an IP is authenticated and return user info if so
    /// Also cleans up expired entries
    pub fn check_auth(&self, ip: IpAddr) -> Option<AuthenticatedAddress> {
        if let Ok(cache) = self.cache.read() {
            if let Some(entry) = cache.get(&ip) {
                if entry.last_authenticated.elapsed() < self.timeout {
                    return Some(entry.clone());
                }
            }
        }
        None
    }

    /// Check if an IP is authenticated (simple boolean check)
    pub fn is_authenticated(&self, ip: IpAddr) -> bool {
        self.check_auth(ip).is_some()
    }

    /// Remove expired entries from the cache
    pub fn cleanup_expired(&self) {
        if let Ok(mut cache) = self.cache.write() {
            let timeout = self.timeout;
            cache.retain(|_, entry| entry.last_authenticated.elapsed() < timeout);
        }
    }

    /// Get all currently valid authenticated addresses
    pub fn get_all_valid(&self) -> Vec<AuthenticatedAddress> {
        if let Ok(cache) = self.cache.read() {
            cache.values()
                .filter(|entry| entry.last_authenticated.elapsed() < self.timeout)
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get the configured timeout duration
    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}

/// Shared authenticated address cache type
pub type SharedAuthAddressCache = Arc<AuthAddressCache>;

/// Create a new shared authenticated address cache
pub fn create_auth_cache(timeout_secs: u64) -> SharedAuthAddressCache {
    Arc::new(AuthAddressCache::new(timeout_secs))
}
