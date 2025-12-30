/// Packet tracking for ICMP correlation
/// 
/// This module manages tracking of UDP packets for correlation with ICMP errors.
/// Packets are stored with their cleartext payloads and automatically expire
/// based on the track_for_ms value.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{RwLock, mpsc};
use common::{SendOptions, TrackedPacketEvent};

/// Data sent from UDP layer to ICMP listener for packet tracking
#[derive(Debug, Clone)]
pub struct UdpPacketInfo {
    /// Destination address of the packet
    pub dest_addr: SocketAddr,
    
    /// Actual UDP packet length (UDP header + payload)
    pub udp_length: u16,
    
    /// Original cleartext data before encryption
    pub cleartext: Vec<u8>,
    
    /// Send options (TTL, TOS, etc.)
    pub send_options: SendOptions,
    
    /// When the packet was sent
    pub sent_at: Instant,
    
    /// Connection ID for per-session event routing
    pub conn_id: String,
    
    /// UDP checksum for matching with ICMP errors
    pub udp_checksum: u16,
}

/// Key for matching UDP packets in ICMP errors
/// Uses destination address and UDP packet length for unique identification
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct UdpPacketKey {
    pub dest_addr: SocketAddr,
    pub udp_length: u16,  // UDP packet length (includes 8-byte UDP header + payload)
}

/// Maximum payload prefix size to store for matching (64 bytes)
pub const MAX_PAYLOAD_PREFIX_SIZE: usize = 64;

/// Information about a tracked packet
#[derive(Debug, Clone)]
pub struct TrackedPacket {
    /// Original cleartext data
    pub cleartext: Vec<u8>,
    
    /// The UDP packet that was sent (IP header + UDP header + payload)
    pub udp_packet: Vec<u8>,
    
    /// When the packet was sent
    pub sent_at: Instant,
    
    /// When to expire this tracking entry
    pub expires_at: Instant,
    
    /// Send options used for this packet
    pub send_options: SendOptions,
    
    /// Destination address
    pub dest_addr: SocketAddr,
    
    /// Source port used
    pub src_port: u16,
    
    /// First 64 bytes of the UDP payload (for matching with ICMP errors)
    pub payload_prefix: Vec<u8>,
    
    /// Connection ID for per-session event routing
    pub conn_id: String,
    
    /// UDP checksum for matching with ICMP errors
    pub udp_checksum: u16,
}

/// Callback type for handling unmatched ICMP errors
/// Passes the full EmbeddedUdpInfo for session lookup, cleanup, and enhanced logging
pub type IcmpErrorCallback = Arc<dyn Fn(EmbeddedUdpInfo) + Send + Sync>;

/// Key for payload-based matching (uses first N bytes of UDP payload)
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct PayloadPrefixKey {
    /// First N bytes of the UDP payload (up to MAX_PAYLOAD_PREFIX_SIZE)
    pub payload_prefix: Vec<u8>,
}

/// Key for checksum-based matching (uses destination address + UDP checksum)
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct ChecksumKey {
    /// Destination address (IP + port)
    pub dest_addr: SocketAddr,
    /// UDP checksum
    pub udp_checksum: u16,
}

/// Manages tracked packets and provides lookup for ICMP correlation
pub struct PacketTracker {
    /// Maps packet key to tracked packet info (primary index using 5-tuple + length)
    tracked_packets: Arc<RwLock<HashMap<UdpPacketKey, TrackedPacket>>>,
    
    /// Secondary index: maps payload prefix to UdpPacketKey for faster payload-based lookup
    payload_index: Arc<RwLock<HashMap<PayloadPrefixKey, UdpPacketKey>>>,
    
    /// Tertiary index: maps (dest_addr, udp_checksum) to UdpPacketKey for checksum-based lookup
    checksum_index: Arc<RwLock<HashMap<ChecksumKey, UdpPacketKey>>>,
    
    /// Queue of matched ICMP events
    pub(crate) event_queue: Arc<RwLock<Vec<TrackedPacketEvent>>>,
    
    /// Receiver for packet tracking data from UDP layer
    tracking_rx: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<UdpPacketInfo>>>,
    
    /// Callback to handle unmatched ICMP errors (for session state to manage)
    icmp_error_callback: Arc<RwLock<Option<IcmpErrorCallback>>>,
}

impl PacketTracker {
    pub fn new() -> (Self, mpsc::UnboundedSender<UdpPacketInfo>) {
        let (tx, rx) = mpsc::unbounded_channel();
        
        let tracker = Self {
            tracked_packets: Arc::new(RwLock::new(HashMap::new())),
            payload_index: Arc::new(RwLock::new(HashMap::new())),
            checksum_index: Arc::new(RwLock::new(HashMap::new())),
            event_queue: Arc::new(RwLock::new(Vec::new())),
            tracking_rx: Arc::new(tokio::sync::Mutex::new(rx)),
            icmp_error_callback: Arc::new(RwLock::new(None)),
        };
        
        // Start cleanup task for expired tracked packets
        let tracked_packets = tracker.tracked_packets.clone();
        let payload_index = tracker.payload_index.clone();
        let checksum_index = tracker.checksum_index.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
            loop {
                interval.tick().await;
                Self::cleanup_expired(&tracked_packets, &payload_index, &checksum_index).await;
            }
        });
        
        // Start tracking receiver task
        let tracking_rx = tracker.tracking_rx.clone();
        let tracked_packets = tracker.tracked_packets.clone();
        let payload_index = tracker.payload_index.clone();
        let checksum_index = tracker.checksum_index.clone();
        tokio::spawn(async move {
            Self::tracking_receiver_task(tracking_rx, tracked_packets, payload_index, checksum_index).await;
        });
        
        (tracker, tx)
    }
    
    /// Set the callback for handling unmatched ICMP errors
    pub async fn set_icmp_error_callback(&self, callback: IcmpErrorCallback) {
        let mut cb = self.icmp_error_callback.write().await;
        *cb = Some(callback);
    }
    
    /// Track a packet for ICMP correlation (test-only helper, no checksum)
    /// 
    /// In production, packets are tracked via the UDP layer FFI callback which
    /// calculates the checksum. This helper is for tests that don't need checksum
    /// matching (e.g., tests for payload or length-based matching).
    #[cfg(test)]
    pub async fn track_packet(
        &self,
        cleartext: Vec<u8>,
        udp_packet: Vec<u8>,
        src_port: u16,
        dest_addr: SocketAddr,
        udp_length: u16,
        send_options: SendOptions,
        conn_id: String,
    ) {
        // Pass 0 checksum - will not match via checksum, only via payload or length
        self.track_packet_with_checksum(cleartext, udp_packet, src_port, dest_addr, udp_length, send_options, conn_id, 0).await;
    }
    
    /// Track a packet for ICMP correlation with explicit checksum (test-only helper)
    /// 
    /// Use this for testing checksum-based matching specifically.
    /// In production, packets are tracked via the UDP layer FFI callback.
    #[cfg(test)]
    pub async fn track_packet_with_checksum(
        &self,
        cleartext: Vec<u8>,
        udp_packet: Vec<u8>,
        src_port: u16,
        dest_addr: SocketAddr,
        udp_length: u16,
        send_options: SendOptions,
        conn_id: String,
        udp_checksum: u16,
    ) {
        if send_options.track_for_ms == 0 {
            return;
        }
        
        let now = Instant::now();
        let expires_at = now + std::time::Duration::from_millis(send_options.track_for_ms as u64);
        
        let key = UdpPacketKey {
            dest_addr,
            udp_length,
        };
        
        // Extract first 64 bytes of UDP payload for payload-based matching
        let payload_prefix = if udp_packet.len() > 8 {
            let payload_start = 8;
            let payload_end = std::cmp::min(payload_start + MAX_PAYLOAD_PREFIX_SIZE, udp_packet.len());
            udp_packet[payload_start..payload_end].to_vec()
        } else {
            cleartext.iter().take(MAX_PAYLOAD_PREFIX_SIZE).cloned().collect()
        };
        
        let tracked = TrackedPacket {
            cleartext,
            udp_packet,
            sent_at: now,
            expires_at,
            send_options,
            dest_addr,
            src_port,
            payload_prefix: payload_prefix.clone(),
            conn_id,
            udp_checksum,
        };
        
        let mut packets = self.tracked_packets.write().await;
        packets.insert(key.clone(), tracked);
        
        if !payload_prefix.is_empty() {
            let payload_key = PayloadPrefixKey { payload_prefix };
            let mut payload_idx = self.payload_index.write().await;
            payload_idx.insert(payload_key, key.clone());
        }
        
        // Add to checksum index if checksum is non-zero
        if udp_checksum != 0 {
            let checksum_key = ChecksumKey { dest_addr, udp_checksum };
            let mut checksum_idx = self.checksum_index.write().await;
            checksum_idx.insert(checksum_key, key);
        }
    }
    
    /// Try to match an ICMP error packet with a tracked UDP packet
    /// Matching order: 1) checksum-based (most reliable), 2) payload-based, 3) destination + length fallback
    pub async fn match_icmp_error(
        &self,
        icmp_packet: Vec<u8>,
        embedded_udp_info: EmbeddedUdpInfo,
        router_ip: Option<String>,
    ) {
        tracing::debug!("match_icmp_error called: src_port={}, dest={}, udp_length={}, udp_checksum={:#06x}, payload_prefix_len={}", 
            embedded_udp_info.src_port, embedded_udp_info.dest_addr, embedded_udp_info.udp_length,
            embedded_udp_info.udp_checksum, embedded_udp_info.payload_prefix.len());
        
        let mut packets = self.tracked_packets.write().await;
        let mut payload_idx = self.payload_index.write().await;
        let mut checksum_idx = self.checksum_index.write().await;
        tracing::debug!("Current tracked packets count: {}", packets.len());
        
        let mut matched: Option<TrackedPacket> = None;
        let mut match_type = "none";
        
        // First, try checksum-based matching (most reliable since checksum includes all packet data)
        if embedded_udp_info.udp_checksum != 0 {
            let checksum_key = ChecksumKey {
                dest_addr: embedded_udp_info.dest_addr,
                udp_checksum: embedded_udp_info.udp_checksum,
            };
            
            if let Some(key) = checksum_idx.remove(&checksum_key) {
                if let Some(tracked) = packets.remove(&key) {
                    tracing::debug!("CHECKSUM MATCH FOUND! checksum={:#06x}, dest={}, udp_length={}, tracked: {:?}",
                        embedded_udp_info.udp_checksum, key.dest_addr, key.udp_length, &tracked);
                    
                    // Also remove from payload index if present
                    if !tracked.payload_prefix.is_empty() {
                        let payload_key = PayloadPrefixKey {
                            payload_prefix: tracked.payload_prefix.clone(),
                        };
                        payload_idx.remove(&payload_key);
                    }
                    
                    matched = Some(tracked);
                    match_type = "checksum";
                } else {
                    tracing::debug!("Checksum index pointed to non-existent packet key");
                }
            } else {
                tracing::debug!("No checksum match found");
            }
        }

        let mut do_payload_match = false;
        let mut do_fallback_match = false;
        
        // Second, try payload-based matching if we have payload data from the ICMP packet
        if matched.is_none() && do_payload_match && !embedded_udp_info.payload_prefix.is_empty() {
            let payload_key = PayloadPrefixKey {
                payload_prefix: embedded_udp_info.payload_prefix.clone(),
            };
            
            if let Some(key) = payload_idx.remove(&payload_key) {
                if let Some(tracked) = packets.remove(&key) {
                    tracing::debug!("PAYLOAD MATCH FOUND! payload_prefix_len={}, dest={}, udp_length={}",
                        embedded_udp_info.payload_prefix.len(), key.dest_addr, key.udp_length);
                    
                    // Also remove from checksum index if present
                    if tracked.udp_checksum != 0 {
                        let checksum_key = ChecksumKey {
                            dest_addr: tracked.dest_addr,
                            udp_checksum: tracked.udp_checksum,
                        };
                        checksum_idx.remove(&checksum_key);
                    }
                    
                    matched = Some(tracked);
                    match_type = "payload";
                } else {
                    tracing::debug!("Payload index pointed to non-existent packet key");
                }
            } else {
                tracing::debug!("No payload match found, trying fallback to destination + length");
            }
        }
        
        // Fallback: Match based on destination address and UDP length
        if matched.is_none() && do_fallback_match {
            let key = UdpPacketKey {
                dest_addr: embedded_udp_info.dest_addr,
                udp_length: embedded_udp_info.udp_length,
            };
            
            if let Some(tracked) = packets.remove(&key) {
                tracing::debug!("FALLBACK MATCH FOUND! dest={}, udp_length={}",
                    embedded_udp_info.dest_addr, embedded_udp_info.udp_length);
                
                // Also remove from payload index if present
                if !tracked.payload_prefix.is_empty() {
                    let payload_key = PayloadPrefixKey {
                        payload_prefix: tracked.payload_prefix.clone(),
                    };
                    payload_idx.remove(&payload_key);
                }
                
                // Also remove from checksum index if present
                if tracked.udp_checksum != 0 {
                    let checksum_key = ChecksumKey {
                        dest_addr: tracked.dest_addr,
                        udp_checksum: tracked.udp_checksum,
                    };
                    checksum_idx.remove(&checksum_key);
                }
                
                matched = Some(tracked);
                match_type = "fallback";
            }
        }
        
        // Release locks early to avoid holding them during callback
        drop(packets);
        drop(payload_idx);
        drop(checksum_idx);
        
        if let Some(tracked) = matched {
            tracing::debug!("MATCH FOUND via {}: dest={}, udp_length={}, conn_id={}", 
                match_type, embedded_udp_info.dest_addr, embedded_udp_info.udp_length, tracked.conn_id);
            
            let event = TrackedPacketEvent {
                icmp_packet,
                udp_packet: tracked.udp_packet,
                cleartext: tracked.cleartext,
                sent_at: tracked.sent_at,
                icmp_received_at: Instant::now(),
                send_options: tracked.send_options,
                router_ip,
                conn_id: tracked.conn_id,
                original_src_port: embedded_udp_info.src_port,
                original_dest_addr: embedded_udp_info.dest_addr.to_string(),
            };
            
            tracing::debug!("Event added to queue for conn_id={}, event: {:?}", event.conn_id, event);
            
            let mut queue = self.event_queue.write().await;
            queue.push(event);
            
            tracing::debug!("Queue size after push: {}", queue.len());
            
            tracing::debug!(
                "ICMP error matched to tracked packet: dest={}, udp_length={}",
                embedded_udp_info.dest_addr,
                embedded_udp_info.udp_length
            );
        } else {
            tracing::debug!("NO MATCH FOUND for dest={}, udp_length={}, udp_checksum={:#06x}, payload_prefix_len={}", 
                embedded_udp_info.dest_addr, embedded_udp_info.udp_length,
                embedded_udp_info.udp_checksum, embedded_udp_info.payload_prefix.len());
            
            // Pass unmatched ICMP error to callback for session state to handle
            let callback = self.icmp_error_callback.read().await;
            if let Some(ref cb) = *callback {
                cb(embedded_udp_info);
            }
        }
    }
    
    /// Get and clear all queued events
    #[deprecated(since = "0.1.0", note = "Use drain_events_for_conn_id() for per-session event filtering")]
    pub async fn drain_events(&self) -> Vec<TrackedPacketEvent> {
        let mut queue = self.event_queue.write().await;
        std::mem::take(&mut *queue)
    }
    
    /// Get and remove queued events for a specific connection ID
    /// Only returns events matching the given conn_id, leaving other events in the queue
    pub async fn drain_events_for_conn_id(&self, conn_id: &str) -> Vec<TrackedPacketEvent> {
        let mut queue = self.event_queue.write().await;
        
        // Partition events: matching conn_id vs. others
        let mut matching = Vec::new();
        let mut remaining = Vec::new();
        
        for event in queue.drain(..) {
            if event.conn_id == conn_id {
                matching.push(event);
            } else {
                remaining.push(event);
            }
        }
        
        // Put non-matching events back in the queue
        *queue = remaining;
        
        matching
    }
    
    /// Get current number of tracked packets
    pub async fn tracked_count(&self) -> usize {
        self.tracked_packets.read().await.len()
    }
    
    /// Task that receives tracking data from UDP layer and stores it
    async fn tracking_receiver_task(
        tracking_rx: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<UdpPacketInfo>>>,
        tracked_packets: Arc<RwLock<HashMap<UdpPacketKey, TrackedPacket>>>,
        payload_index: Arc<RwLock<HashMap<PayloadPrefixKey, UdpPacketKey>>>,
        checksum_index: Arc<RwLock<HashMap<ChecksumKey, UdpPacketKey>>>,
    ) {
        let mut rx = tracking_rx.lock().await;
        
        while let Some(info) = rx.recv().await {
            if info.send_options.track_for_ms == 0 {
                continue;
            }
            
            // Use conn_id directly from UdpPacketInfo (passed through from UdpSendOptions)
            let conn_id = info.conn_id.clone();
            
            tracing::debug!("Received tracking data from UDP layer: dest={}, udp_length={}, udp_checksum={:#06x}, ttl={:?}, conn_id={}", 
                info.dest_addr, info.udp_length, info.udp_checksum, info.send_options.ttl, conn_id);
            
            let expires_at = info.sent_at + std::time::Duration::from_millis(info.send_options.track_for_ms as u64);
            
            let key = UdpPacketKey {
                dest_addr: info.dest_addr,
                udp_length: info.udp_length,
            };
            
            // Extract first 64 bytes of cleartext for payload-based matching
            let payload_prefix: Vec<u8> = info.cleartext.iter()
                .take(MAX_PAYLOAD_PREFIX_SIZE)
                .cloned()
                .collect();
            
            tracing::debug!("Storing payload_prefix of {} bytes (from UDP layer)", payload_prefix.len());
            
            let tracked = TrackedPacket {
                cleartext: info.cleartext,
                udp_packet: Vec::new(), // Not available at this layer
                sent_at: info.sent_at,
                expires_at,
                send_options: info.send_options,
                dest_addr: info.dest_addr,
                src_port: 0, // Not available at this layer
                payload_prefix: payload_prefix.clone(),
                conn_id,
                udp_checksum: info.udp_checksum,
            };
            
            let mut packets = tracked_packets.write().await;
            packets.insert(key.clone(), tracked);
            
            // Add to payload index if we have a non-empty payload prefix
            if !payload_prefix.is_empty() {
                let payload_key = PayloadPrefixKey { payload_prefix };
                let mut payload_idx = payload_index.write().await;
                payload_idx.insert(payload_key, key.clone());
            }
            
            // Add to checksum index if we have a non-zero checksum
            if info.udp_checksum != 0 {
                let checksum_key = ChecksumKey {
                    dest_addr: info.dest_addr,
                    udp_checksum: info.udp_checksum,
                };
                let mut checksum_idx = checksum_index.write().await;
                checksum_idx.insert(checksum_key, key);
            }
            
            let count = packets.len();
            tracing::debug!("Packet tracked successfully (from UDP layer), total tracked packets: {}", count);
            
            tracing::debug!(
                "Tracked packet from UDP layer: dest={}, udp_length={}, udp_checksum={:#06x}, ttl={:?}",
                info.dest_addr,
                info.udp_length,
                info.udp_checksum,
                info.send_options.ttl
            );
        }
    }
    
    /// Clean up expired tracking entries
    async fn cleanup_expired(
        tracked_packets: &Arc<RwLock<HashMap<UdpPacketKey, TrackedPacket>>>,
        payload_index: &Arc<RwLock<HashMap<PayloadPrefixKey, UdpPacketKey>>>,
        checksum_index: &Arc<RwLock<HashMap<ChecksumKey, UdpPacketKey>>>,
    ) {
        let now = Instant::now();
        let mut packets = tracked_packets.write().await;
        let mut payload_idx = payload_index.write().await;
        let mut checksum_idx = checksum_index.write().await;
        
        // Collect expired entries and their payload prefixes and checksums
        let expired_keys: Vec<(UdpPacketKey, Vec<u8>, SocketAddr, u16)> = packets
            .iter()
            .filter(|(_, tracked)| tracked.expires_at <= now)
            .map(|(key, tracked)| (key.clone(), tracked.payload_prefix.clone(), tracked.dest_addr, tracked.udp_checksum))
            .collect();
        
        let removed_count = expired_keys.len();
        
        // Remove from all indexes
        for (key, payload_prefix, dest_addr, udp_checksum) in expired_keys {
            packets.remove(&key);
            if !payload_prefix.is_empty() {
                let payload_key = PayloadPrefixKey { payload_prefix };
                payload_idx.remove(&payload_key);
            }
            if udp_checksum != 0 {
                let checksum_key = ChecksumKey { dest_addr, udp_checksum };
                checksum_idx.remove(&checksum_key);
            }
        }
        
        if removed_count > 0 {
            tracing::debug!("Cleaned up {} expired tracked packets", removed_count);
        }
    }
}

impl Default for PacketTracker {
    fn default() -> Self {
        let (tracker, _tx) = Self::new();
        tracker
    }
}

/// Information extracted from ICMP error about the embedded UDP packet
#[derive(Debug, Clone)]
pub struct EmbeddedUdpInfo {
    pub src_port: u16,
    pub dest_addr: SocketAddr,
    pub udp_length: u16,  // UDP packet length from UDP header
    pub payload_prefix: Vec<u8>,
    pub udp_checksum: u16, // UDP checksum for matching
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};
    
    #[tokio::test]
    async fn test_packet_tracker_basic() {
        let (tracker, _tx) = PacketTracker::new();
        
        let options = SendOptions {
            ttl: Some(64),
            df_bit: Some(true),
            tos: None,
            flow_label: None,
            track_for_ms: 5000,
        };
        
        let dest = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        
        // Track a packet
        tracker.track_packet(
            vec![1, 2, 3, 4],  // cleartext
            vec![0; 50],        // udp packet
            12345,              // src port
            dest,
            100,                // udp_length
            options,
            String::new(),      // conn_id
        ).await;
        
        assert_eq!(tracker.tracked_count().await, 1);
    }
    
    #[tokio::test]
    async fn test_icmp_matching_with_udp_length() {
        let (tracker, _tx) = PacketTracker::new();
        
        let options = SendOptions {
            ttl: Some(1),
            df_bit: Some(true),
            tos: None,
            flow_label: None,
            track_for_ms: 5000,
        };
        
        let dest = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        let udp_length = 150;
        
        // Track a packet with specific UDP length
        tracker.track_packet(
            vec![1, 2, 3, 4],
            vec![0; 50],
            12345,
            dest,
            udp_length,
            options,
            String::new(),
        ).await;
        
        assert_eq!(tracker.tracked_count().await, 1);
        
        // Simulate ICMP error with matching UDP length
        let embedded_info = EmbeddedUdpInfo {
            src_port: 12345,
            dest_addr: dest,
            udp_length,
            payload_prefix: Vec::new(), // Empty payload (ICMP Time Exceeded)
            udp_checksum: 0,
        };
        
        let fake_icmp = vec![0u8; 56]; // Fake ICMP packet
        
        tracker.match_icmp_error(fake_icmp, embedded_info, Some("192.168.1.254".to_string())).await;
        
        // Packet should have been matched and removed
        assert_eq!(tracker.tracked_count().await, 0);
        
        // Should have one event in the queue
        let events = tracker.drain_events().await;
        assert_eq!(events.len(), 1);
    }
    
    #[tokio::test]
    async fn test_icmp_no_match_different_length() {
        let (tracker, _tx) = PacketTracker::new();
        
        let options = SendOptions {
            ttl: Some(1),
            df_bit: Some(true),
            tos: None,
            flow_label: None,
            track_for_ms: 5000,
        };
        
        let dest = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        
        // Track a packet with UDP length 150
        tracker.track_packet(
            vec![1, 2, 3, 4],
            vec![0; 50],
            12345,
            dest,
            150,
            options,
            String::new(),
        ).await;
        
        assert_eq!(tracker.tracked_count().await, 1);
        
        // Simulate ICMP error with DIFFERENT UDP length
        let embedded_info = EmbeddedUdpInfo {
            src_port: 12345,
            dest_addr: dest,
            udp_length: 200,  // Different length!
            payload_prefix: Vec::new(),
            udp_checksum: 0,
        };
        
        let fake_icmp = vec![0u8; 56];
        
        tracker.match_icmp_error(fake_icmp, embedded_info, None).await;
        
        // Packet should NOT have been matched (different UDP length)
        assert_eq!(tracker.tracked_count().await, 1);
        
        // Should have no events in the queue
        let events = tracker.drain_events().await;
        assert_eq!(events.len(), 0);
    }
    
    #[tokio::test]
    async fn test_packet_expiry() {
        let (tracker, _tx) = PacketTracker::new();
        
        let options = SendOptions {
            ttl: Some(64),
            df_bit: Some(true),
            tos: None,
            flow_label: None,
            track_for_ms: 100, // Very short expiry
        };
        
        let dest = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        
        tracker.track_packet(
            vec![1, 2, 3, 4],
            vec![0; 50],
            12345,
            dest,
            100,  // udp_length
            options,
            String::new(),
        ).await;
        
        assert_eq!(tracker.tracked_count().await, 1);
        
        // Wait for expiry + cleanup interval (cleanup runs every 1 second)
        // The packet expires after 100ms, but cleanup only runs every 1000ms
        tokio::time::sleep(tokio::time::Duration::from_millis(1100)).await;
        
        // Should be cleaned up
        assert_eq!(tracker.tracked_count().await, 0);
    }
    
    #[tokio::test]
    async fn test_unmatched_icmp_error_callback() {
        let (tracker, _tx) = PacketTracker::new();
        
        // Setup a callback to track if it was invoked
        let callback_invoked = Arc::new(tokio::sync::RwLock::new(Vec::<EmbeddedUdpInfo>::new()));
        let callback_invoked_clone = callback_invoked.clone();
        let callback = Arc::new(move |embedded_info: EmbeddedUdpInfo| {
            let invoked = callback_invoked_clone.clone();
            tokio::spawn(async move {
                invoked.write().await.push(embedded_info);
            });
        });
        
        tracker.set_icmp_error_callback(callback).await;
        
        let dest = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        
        // Simulate 3 unmatched ICMP errors
        for i in 0..3u16 {
            let embedded_info = EmbeddedUdpInfo {
                src_port: 12345,
                dest_addr: dest,
                udp_length: 100 + i, // Different UDP lengths so no packet is tracked
                payload_prefix: Vec::new(),
                udp_checksum: 0xABCD + i,
            };
            
            let fake_icmp = vec![0u8; 56];
            tracker.match_icmp_error(fake_icmp, embedded_info, None).await;
        }
        
        // Give the callback time to execute
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        // Callback should have been invoked 3 times
        let invocations = callback_invoked.read().await;
        assert_eq!(invocations.len(), 3, "Callback should be invoked for each unmatched error");
        assert_eq!(invocations[0].dest_addr, dest);
        assert_eq!(invocations[0].udp_length, 100);
        assert_eq!(invocations[0].udp_checksum, 0xABCD);
        assert_eq!(invocations[1].dest_addr, dest);
        assert_eq!(invocations[1].udp_length, 101);
        assert_eq!(invocations[1].udp_checksum, 0xABCE);
        assert_eq!(invocations[2].dest_addr, dest);
        assert_eq!(invocations[2].udp_length, 102);
        assert_eq!(invocations[2].udp_checksum, 0xABCF);
    }
    
    #[tokio::test]
    async fn test_matched_icmp_no_callback() {
        let (tracker, _tx) = PacketTracker::new();
        
        let callback_invoked = Arc::new(tokio::sync::RwLock::new(false));
        let callback_invoked_clone = callback_invoked.clone();
        let callback = Arc::new(move |_embedded_info: EmbeddedUdpInfo| {
            let invoked = callback_invoked_clone.clone();
            tokio::spawn(async move {
                *invoked.write().await = true;
            });
        });
        
        tracker.set_icmp_error_callback(callback).await;
        
        let dest = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        let options = SendOptions {
            ttl: Some(1),
            df_bit: Some(true),
            tos: None,
            flow_label: None,
            track_for_ms: 5000,
        };
        
        // Track a packet
        tracker.track_packet(
            vec![1, 2, 3, 4],
            vec![0; 50],
            12345,
            dest,
            200,
            options,
            String::new(),
        ).await;
        
        // Send a matching ICMP error
        let embedded_info = EmbeddedUdpInfo {
            src_port: 12345,
            dest_addr: dest,
            udp_length: 200,
            payload_prefix: Vec::new(),
            udp_checksum: 0,
        };
        
        let fake_icmp = vec![0u8; 56];
        tracker.match_icmp_error(fake_icmp, embedded_info, None).await;
        
        // Should have matched - callback should NOT be invoked
        assert_eq!(tracker.tracked_count().await, 0);
        assert_eq!(tracker.drain_events().await.len(), 1);
        
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        // Callback should not have been invoked for matched error
        assert!(!*callback_invoked.read().await, "Callback should not be invoked for matched errors");
    }
    
    #[tokio::test]
    async fn test_payload_based_matching() {
        let (tracker, _tx) = PacketTracker::new();
        
        let options = SendOptions {
            ttl: Some(1),
            df_bit: Some(true),
            tos: None,
            flow_label: None,
            track_for_ms: 5000,
        };
        
        let dest = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        
        // Create a fake UDP packet with a recognizable payload
        // UDP packet = 8 bytes header + payload
        let payload = vec![0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE, 0x12, 0x34, 0x56, 0x78];
        let mut udp_packet = vec![0u8; 8]; // 8-byte UDP header
        udp_packet.extend_from_slice(&payload);
        
        // Track a packet
        tracker.track_packet(
            payload.clone(),  // cleartext
            udp_packet,       // udp packet  
            12345,            // src port
            dest,
            (8 + payload.len()) as u16, // udp_length
            options,
            String::new(),
        ).await;
        
        assert_eq!(tracker.tracked_count().await, 1);
        
        // Simulate ICMP error with matching payload but DIFFERENT UDP length
        // This tests that payload matching takes priority
        let embedded_info = EmbeddedUdpInfo {
            src_port: 12345,
            dest_addr: dest,
            udp_length: 999, // Wrong UDP length
            payload_prefix: payload.clone(), // But matching payload!
            udp_checksum: 0,
        };
        
        let fake_icmp = vec![0u8; 56];
        tracker.match_icmp_error(fake_icmp, embedded_info, Some("10.0.0.1".to_string())).await;
        
        // Packet should have been matched via payload
        assert_eq!(tracker.tracked_count().await, 0);
        
        let events = tracker.drain_events().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].router_ip, Some("10.0.0.1".to_string()));
    }
    
    #[tokio::test]
    async fn test_fallback_to_length_matching() {
        let (tracker, _tx) = PacketTracker::new();
        
        let options = SendOptions {
            ttl: Some(1),
            df_bit: Some(true),
            tos: None,
            flow_label: None,
            track_for_ms: 5000,
        };
        
        let dest = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        let udp_length = 150;
        
        // Track a packet
        tracker.track_packet(
            vec![1, 2, 3, 4],  // cleartext
            vec![0; 50],       // udp packet
            12345,             // src port
            dest,
            udp_length,
            options,
            String::new(),
        ).await;
        
        assert_eq!(tracker.tracked_count().await, 1);
        
        // Simulate ICMP error with NO payload (like some ICMP Time Exceeded)
        // but matching UDP length - should fallback to length matching
        let embedded_info = EmbeddedUdpInfo {
            src_port: 12345,
            dest_addr: dest,
            udp_length,
            payload_prefix: Vec::new(), // No payload available
            udp_checksum: 0,
        };
        
        let fake_icmp = vec![0u8; 56];
        tracker.match_icmp_error(fake_icmp, embedded_info, None).await;
        
        // Packet should have been matched via fallback (length)
        assert_eq!(tracker.tracked_count().await, 0);
        assert_eq!(tracker.drain_events().await.len(), 1);
    }
    
    #[tokio::test]
    async fn test_conn_id_extraction_and_filtering() {
        let (tracker, _tx) = PacketTracker::new();
        
        let options = SendOptions {
            ttl: Some(1),
            df_bit: Some(true),
            tos: None,
            flow_label: None,
            track_for_ms: 5000,
        };
        
        let dest1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        let dest2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2)), 8080);
        
        // Create cleartext with conn_id embedded (simulating TestProbePacket JSON)
        let cleartext1 = br#"{"test_seq":1,"timestamp_ms":1234,"direction":"ServerToClient","conn_id":"session-a-uuid"}"#.to_vec();
        let cleartext2 = br#"{"test_seq":2,"timestamp_ms":1234,"direction":"ServerToClient","conn_id":"session-b-uuid"}"#.to_vec();
        
        // Track packets for two different sessions
        tracker.track_packet(
            cleartext1.clone(),
            vec![0; 8], // minimal UDP packet
            12345,
            dest1,
            100,
            options,
            "session-a-uuid".to_string(),
        ).await;
        
        tracker.track_packet(
            cleartext2.clone(),
            vec![0; 8],
            12345,
            dest2,
            200,
            options,
            "session-b-uuid".to_string(),
        ).await;
        
        assert_eq!(tracker.tracked_count().await, 2);
        
        // Simulate ICMP errors for both packets
        let embedded_info1 = EmbeddedUdpInfo {
            src_port: 12345,
            dest_addr: dest1,
            udp_length: 100,
            payload_prefix: Vec::new(),
            udp_checksum: 0,
        };
        let embedded_info2 = EmbeddedUdpInfo {
            src_port: 12345,
            dest_addr: dest2,
            udp_length: 200,
            payload_prefix: Vec::new(),
            udp_checksum: 0,
        };
        
        tracker.match_icmp_error(vec![0u8; 56], embedded_info1, Some("10.0.0.1".to_string())).await;
        tracker.match_icmp_error(vec![0u8; 56], embedded_info2, Some("10.0.0.2".to_string())).await;
        
        // Both packets should have been matched
        assert_eq!(tracker.tracked_count().await, 0);
        
        // Now test per-session draining
        // Session A should only get its event
        let session_a_events = tracker.drain_events_for_conn_id("session-a-uuid").await;
        assert_eq!(session_a_events.len(), 1);
        assert_eq!(session_a_events[0].conn_id, "session-a-uuid");
        assert_eq!(session_a_events[0].router_ip, Some("10.0.0.1".to_string()));
        
        // Session B should only get its event
        let session_b_events = tracker.drain_events_for_conn_id("session-b-uuid").await;
        assert_eq!(session_b_events.len(), 1);
        assert_eq!(session_b_events[0].conn_id, "session-b-uuid");
        assert_eq!(session_b_events[0].router_ip, Some("10.0.0.2".to_string()));
        
        // Queue should be empty now
        assert_eq!(tracker.drain_events().await.len(), 0);
    }
    
    #[tokio::test]
    async fn test_checksum_based_matching() {
        let (tracker, _tx) = PacketTracker::new();
        
        let options = SendOptions {
            ttl: Some(1),
            df_bit: Some(true),
            tos: None,
            flow_label: None,
            track_for_ms: 5000,
        };
        
        let dest = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        let udp_checksum = 0xABCD; // Specific checksum value
        
        // Track a packet with a specific checksum
        tracker.track_packet_with_checksum(
            vec![1, 2, 3, 4],  // cleartext
            vec![0; 50],       // udp packet
            12345,             // src port
            dest,
            150,               // udp_length
            options,
            "test-session".to_string(),
            udp_checksum,
        ).await;
        
        assert_eq!(tracker.tracked_count().await, 1);
        
        // Simulate ICMP error with WRONG UDP length but MATCHING checksum
        // This tests that checksum matching takes priority over length matching
        let embedded_info = EmbeddedUdpInfo {
            src_port: 12345,
            dest_addr: dest,
            udp_length: 9999,     // Wrong length!
            payload_prefix: Vec::new(), // No payload
            udp_checksum,         // But matching checksum!
        };
        
        let fake_icmp = vec![0u8; 56];
        tracker.match_icmp_error(fake_icmp, embedded_info, Some("router.example.com".to_string())).await;
        
        // Packet should have been matched via checksum
        assert_eq!(tracker.tracked_count().await, 0);
        
        let events = tracker.drain_events().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].conn_id, "test-session");
        assert_eq!(events[0].router_ip, Some("router.example.com".to_string()));
    }
    
    #[tokio::test]
    async fn test_checksum_no_match_wrong_checksum() {
        let (tracker, _tx) = PacketTracker::new();
        
        let options = SendOptions {
            ttl: Some(1),
            df_bit: Some(true),
            tos: None,
            flow_label: None,
            track_for_ms: 5000,
        };
        
        let dest = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        
        // Track a packet with checksum 0xABCD
        tracker.track_packet_with_checksum(
            vec![1, 2, 3, 4],
            vec![0; 50],
            12345,
            dest,
            150,
            options,
            String::new(),
            0xABCD,
        ).await;
        
        assert_eq!(tracker.tracked_count().await, 1);
        
        // Simulate ICMP error with DIFFERENT checksum and DIFFERENT length
        let embedded_info = EmbeddedUdpInfo {
            src_port: 12345,
            dest_addr: dest,
            udp_length: 9999,     // Wrong length
            payload_prefix: Vec::new(),
            udp_checksum: 0x1234, // Wrong checksum!
        };
        
        let fake_icmp = vec![0u8; 56];
        tracker.match_icmp_error(fake_icmp, embedded_info, None).await;
        
        // Packet should NOT have been matched (wrong checksum and wrong length)
        assert_eq!(tracker.tracked_count().await, 1);
        
        // Should have no events in the queue
        let events = tracker.drain_events().await;
        assert_eq!(events.len(), 0);
    }
    
    #[tokio::test]
    async fn test_checksum_takes_priority_over_payload() {
        let (tracker, _tx) = PacketTracker::new();
        
        let options = SendOptions {
            ttl: Some(1),
            df_bit: Some(true),
            tos: None,
            flow_label: None,
            track_for_ms: 5000,
        };
        
        let dest = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        let payload = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let udp_checksum = 0xCAFE;
        
        // Create a fake UDP packet with payload
        let mut udp_packet = vec![0u8; 8];
        udp_packet.extend_from_slice(&payload);
        
        // Track a packet with both payload and checksum
        tracker.track_packet_with_checksum(
            payload.clone(),
            udp_packet,
            12345,
            dest,
            (8 + payload.len()) as u16,
            options,
            "checksum-priority".to_string(),
            udp_checksum,
        ).await;
        
        assert_eq!(tracker.tracked_count().await, 1);
        
        // Simulate ICMP error with matching checksum but no payload
        // This tests that checksum matching is tried first
        let embedded_info = EmbeddedUdpInfo {
            src_port: 12345,
            dest_addr: dest,
            udp_length: 9999,     // Wrong length
            payload_prefix: Vec::new(), // No payload (checksum should still match)
            udp_checksum,         // Matching checksum
        };
        
        let fake_icmp = vec![0u8; 56];
        tracker.match_icmp_error(fake_icmp, embedded_info, Some("10.0.0.1".to_string())).await;
        
        // Packet should have been matched via checksum (not payload since payload is empty)
        assert_eq!(tracker.tracked_count().await, 0);
        
        let events = tracker.drain_events().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].conn_id, "checksum-priority");
    }
}
