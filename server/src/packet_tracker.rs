/// Packet tracking for ICMP correlation
/// 
/// This module manages tracking of UDP packets for correlation with ICMP errors.
/// Packets are stored with their cleartext payloads and automatically expire
/// based on the track_for_ms value.

use std::collections::HashMap;
use std::net::{SocketAddr, IpAddr};
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
}

/// Key for matching UDP packets in ICMP errors
/// Uses destination address and UDP packet length for unique identification
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct UdpPacketKey {
    pub dest_addr: SocketAddr,
    pub udp_length: u16,  // UDP packet length (includes 8-byte UDP header + payload)
}

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
}

/// Callback type for handling unmatched ICMP errors
/// Passes the full destination socket address (IP + port) for session lookup and cleanup
pub type IcmpErrorCallback = Arc<dyn Fn(SocketAddr) + Send + Sync>;

/// Manages tracked packets and provides lookup for ICMP correlation
pub struct PacketTracker {
    /// Maps packet key to tracked packet info
    tracked_packets: Arc<RwLock<HashMap<UdpPacketKey, TrackedPacket>>>,
    
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
            event_queue: Arc::new(RwLock::new(Vec::new())),
            tracking_rx: Arc::new(tokio::sync::Mutex::new(rx)),
            icmp_error_callback: Arc::new(RwLock::new(None)),
        };
        
        // Start cleanup task for expired tracked packets
        let tracked_packets = tracker.tracked_packets.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
            loop {
                interval.tick().await;
                Self::cleanup_expired(&tracked_packets).await;
            }
        });
        
        // Start tracking receiver task
        let tracking_rx = tracker.tracking_rx.clone();
        let tracked_packets = tracker.tracked_packets.clone();
        tokio::spawn(async move {
            Self::tracking_receiver_task(tracking_rx, tracked_packets).await;
        });
        
        (tracker, tx)
    }
    
    /// Set the callback for handling unmatched ICMP errors
    pub async fn set_icmp_error_callback(&self, callback: IcmpErrorCallback) {
        let mut cb = self.icmp_error_callback.write().await;
        *cb = Some(callback);
    }
    
    /// Track a packet for ICMP correlation
    pub async fn track_packet(
        &self,
        cleartext: Vec<u8>,
        udp_packet: Vec<u8>,
        src_port: u16,
        dest_addr: SocketAddr,
        udp_length: u16,  // Expected UDP packet length (UDP header + payload)
        send_options: SendOptions,
    ) {
        if send_options.track_for_ms == 0 {
            println!("DEBUG: track_packet called but track_for_ms is 0, not tracking");
            return;
        }
        
        println!("DEBUG: track_packet called: src_port={}, dest={}, udp_length={}, track_for_ms={}, ttl={:?}", 
            src_port, dest_addr, udp_length, send_options.track_for_ms, send_options.ttl);
        
        let now = Instant::now();
        let expires_at = now + std::time::Duration::from_millis(send_options.track_for_ms as u64);
        
        // Create key from destination address and UDP length
        println!("DEBUG: Creating tracking key with dest_addr={}, udp_length={}", dest_addr, udp_length);
        
        let key = UdpPacketKey {
            dest_addr,
            udp_length,
        };
        
        let tracked = TrackedPacket {
            cleartext,
            udp_packet,
            sent_at: now,
            expires_at,
            send_options,
            dest_addr,
            src_port,
        };
        
        let mut packets = self.tracked_packets.write().await;
        packets.insert(key, tracked);
        
        let count = packets.len();
        println!("DEBUG: Packet tracked successfully, total tracked packets: {}", count);
        
        tracing::debug!(
            "Tracking packet: src_port={}, dest={}, expires_in={}ms",
            src_port,
            dest_addr,
            send_options.track_for_ms
        );
    }
    
    /// Try to match an ICMP error packet with a tracked UDP packet
    pub async fn match_icmp_error(
        &self,
        icmp_packet: Vec<u8>,
        embedded_udp_info: EmbeddedUdpInfo,
        router_ip: Option<String>,
    ) {
        println!("DEBUG: match_icmp_error called: src_port={}, dest={}, udp_length={}", 
            embedded_udp_info.src_port, embedded_udp_info.dest_addr, embedded_udp_info.udp_length);
        
        let mut packets = self.tracked_packets.write().await;
        println!("DEBUG: Current tracked packets count: {}", packets.len());
        
        // Match based on destination address and UDP length
        let key = UdpPacketKey {
            dest_addr: embedded_udp_info.dest_addr,
            udp_length: embedded_udp_info.udp_length,
        };
        
        let matched = packets.remove(&key);
        // Release lock early to avoid holding it during callback
        drop(packets);
        
        if let Some(tracked) = matched {
            println!("DEBUG: MATCH FOUND! dest={}, udp_length={}", 
                embedded_udp_info.dest_addr, embedded_udp_info.udp_length);
            
            let event = TrackedPacketEvent {
                icmp_packet,
                udp_packet: tracked.udp_packet,
                cleartext: tracked.cleartext,
                sent_at: tracked.sent_at,
                icmp_received_at: Instant::now(),
                send_options: tracked.send_options,
                router_ip,
            };
            
            let mut queue = self.event_queue.write().await;
            queue.push(event);
            
            println!("DEBUG: Event added to queue, queue size: {}", queue.len());
            
            tracing::info!(
                "ICMP error matched to tracked packet: dest={}, udp_length={}",
                embedded_udp_info.dest_addr,
                embedded_udp_info.udp_length
            );
        } else {
            println!("DEBUG: NO MATCH FOUND for dest={}, udp_length={}", 
                embedded_udp_info.dest_addr, embedded_udp_info.udp_length);
            
            // Pass unmatched ICMP error to callback for session state to handle
            let callback = self.icmp_error_callback.read().await;
            if let Some(ref cb) = *callback {
                cb(embedded_udp_info.dest_addr);
            }
        }
    }
    
    /// Get and clear all queued events
    pub async fn drain_events(&self) -> Vec<TrackedPacketEvent> {
        let mut queue = self.event_queue.write().await;
        std::mem::take(&mut *queue)
    }
    
    /// Get current number of tracked packets
    pub async fn tracked_count(&self) -> usize {
        self.tracked_packets.read().await.len()
    }
    
    /// Task that receives tracking data from UDP layer and stores it
    async fn tracking_receiver_task(
        tracking_rx: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<UdpPacketInfo>>>,
        tracked_packets: Arc<RwLock<HashMap<UdpPacketKey, TrackedPacket>>>,
    ) {
        let mut rx = tracking_rx.lock().await;
        
        while let Some(info) = rx.recv().await {
            if info.send_options.track_for_ms == 0 {
                continue;
            }
            
            println!("DEBUG: Received tracking data from UDP layer: dest={}, udp_length={}, ttl={:?}", 
                info.dest_addr, info.udp_length, info.send_options.ttl);
            
            let expires_at = info.sent_at + std::time::Duration::from_millis(info.send_options.track_for_ms as u64);
            
            let key = UdpPacketKey {
                dest_addr: info.dest_addr,
                udp_length: info.udp_length,
            };
            
            let tracked = TrackedPacket {
                cleartext: info.cleartext,
                udp_packet: Vec::new(), // Not available at this layer
                sent_at: info.sent_at,
                expires_at,
                send_options: info.send_options,
                dest_addr: info.dest_addr,
                src_port: 0, // Not available at this layer
            };
            
            let mut packets = tracked_packets.write().await;
            packets.insert(key, tracked);
            
            let count = packets.len();
            println!("DEBUG: Packet tracked successfully (from UDP layer), total tracked packets: {}", count);
            
            tracing::debug!(
                "Tracked packet from UDP layer: dest={}, udp_length={}, ttl={:?}",
                info.dest_addr,
                info.udp_length,
                info.send_options.ttl
            );
        }
    }
    
    /// Clean up expired tracking entries
    async fn cleanup_expired(tracked_packets: &Arc<RwLock<HashMap<UdpPacketKey, TrackedPacket>>>) {
        let now = Instant::now();
        let mut packets = tracked_packets.write().await;
        
        let before_count = packets.len();
        packets.retain(|_, tracked| tracked.expires_at > now);
        let removed = before_count - packets.len();
        
        if removed > 0 {
            tracing::debug!("Cleaned up {} expired tracked packets", removed);
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
        ).await;
        
        assert_eq!(tracker.tracked_count().await, 1);
        
        // Simulate ICMP error with matching UDP length
        let embedded_info = EmbeddedUdpInfo {
            src_port: 12345,
            dest_addr: dest,
            udp_length,
            payload_prefix: Vec::new(), // Empty payload (ICMP Time Exceeded)
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
        ).await;
        
        assert_eq!(tracker.tracked_count().await, 1);
        
        // Simulate ICMP error with DIFFERENT UDP length
        let embedded_info = EmbeddedUdpInfo {
            src_port: 12345,
            dest_addr: dest,
            udp_length: 200,  // Different length!
            payload_prefix: Vec::new(),
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
        let callback_invoked = Arc::new(tokio::sync::RwLock::new(Vec::<SocketAddr>::new()));
        let callback_invoked_clone = callback_invoked.clone();
        let callback = Arc::new(move |dest_addr: SocketAddr| {
            let invoked = callback_invoked_clone.clone();
            tokio::spawn(async move {
                invoked.write().await.push(dest_addr);
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
            };
            
            let fake_icmp = vec![0u8; 56];
            tracker.match_icmp_error(fake_icmp, embedded_info, None).await;
        }
        
        // Give the callback time to execute
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        // Callback should have been invoked 3 times
        let invocations = callback_invoked.read().await;
        assert_eq!(invocations.len(), 3, "Callback should be invoked for each unmatched error");
        assert_eq!(invocations[0], dest);
        assert_eq!(invocations[1], dest);
        assert_eq!(invocations[2], dest);
    }
    
    #[tokio::test]
    async fn test_matched_icmp_no_callback() {
        let (tracker, _tx) = PacketTracker::new();
        
        let callback_invoked = Arc::new(tokio::sync::RwLock::new(false));
        let callback_invoked_clone = callback_invoked.clone();
        let callback = Arc::new(move |_dest_addr: SocketAddr| {
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
        ).await;
        
        // Send a matching ICMP error
        let embedded_info = EmbeddedUdpInfo {
            src_port: 12345,
            dest_addr: dest,
            udp_length: 200,
            payload_prefix: Vec::new(),
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
}
