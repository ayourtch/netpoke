/// Packet tracking for ICMP correlation
/// 
/// This module manages tracking of UDP packets for correlation with ICMP errors.
/// Packets are stored with their cleartext payloads and automatically expire
/// based on the track_for_ms value.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use common::{SendOptions, TrackedPacketEvent};

/// Key for matching UDP packets in ICMP errors
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct UdpPacketKey {
    pub src_port: u16,
    pub dest_addr: SocketAddr,
    /// First N bytes of UDP payload for matching
    pub payload_prefix: Vec<u8>,
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

/// Manages tracked packets and provides lookup for ICMP correlation
pub struct PacketTracker {
    /// Maps packet key to tracked packet info
    tracked_packets: Arc<RwLock<HashMap<UdpPacketKey, TrackedPacket>>>,
    
    /// Queue of matched ICMP events
    pub(crate) event_queue: Arc<RwLock<Vec<TrackedPacketEvent>>>,
}

impl PacketTracker {
    pub fn new() -> Self {
        let tracker = Self {
            tracked_packets: Arc::new(RwLock::new(HashMap::new())),
            event_queue: Arc::new(RwLock::new(Vec::new())),
        };
        
        // Start cleanup task
        let tracked_packets = tracker.tracked_packets.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
            loop {
                interval.tick().await;
                Self::cleanup_expired(&tracked_packets).await;
            }
        });
        
        tracker
    }
    
    /// Track a packet for ICMP correlation
    pub async fn track_packet(
        &self,
        cleartext: Vec<u8>,
        udp_packet: Vec<u8>,
        src_port: u16,
        dest_addr: SocketAddr,
        send_options: SendOptions,
    ) {
        if send_options.track_for_ms == 0 {
            println!("DEBUG: track_packet called but track_for_ms is 0, not tracking");
            return;
        }
        
        println!("DEBUG: track_packet called: src_port={}, dest={}, track_for_ms={}, ttl={:?}", 
            src_port, dest_addr, send_options.track_for_ms, send_options.ttl);
        
        let now = Instant::now();
        let expires_at = now + std::time::Duration::from_millis(send_options.track_for_ms as u64);
        
        // Create key from first 8 bytes of UDP payload (after IP+UDP headers)
        let payload_prefix = udp_packet.get(28..36) // Assuming IPv4(20) + UDP(8) = 28 byte offset
            .unwrap_or(&udp_packet[28..])
            .to_vec();
        
        println!("DEBUG: Payload prefix extracted: len={}", payload_prefix.len());
        
        let key = UdpPacketKey {
            src_port,
            dest_addr,
            payload_prefix,
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
    ) {
        println!("DEBUG: match_icmp_error called: src_port={}, dest={}", 
            embedded_udp_info.src_port, embedded_udp_info.dest_addr);
        
        let key = UdpPacketKey {
            src_port: embedded_udp_info.src_port,
            dest_addr: embedded_udp_info.dest_addr,
            payload_prefix: embedded_udp_info.payload_prefix,
        };
        
        let mut packets = self.tracked_packets.write().await;
        println!("DEBUG: Current tracked packets count: {}", packets.len());
        
        if let Some(tracked) = packets.remove(&key) {
            println!("DEBUG: MATCH FOUND! Removing tracked packet and creating event");
            
            let event = TrackedPacketEvent {
                icmp_packet,
                udp_packet: tracked.udp_packet,
                cleartext: tracked.cleartext,
                sent_at: tracked.sent_at,
                icmp_received_at: Instant::now(),
                send_options: tracked.send_options,
            };
            
            let mut queue = self.event_queue.write().await;
            queue.push(event);
            
            println!("DEBUG: Event added to queue, queue size: {}", queue.len());
            
            tracing::info!(
                "ICMP error matched to tracked packet: src_port={}, dest={}",
                key.src_port,
                key.dest_addr
            );
        } else {
            println!("DEBUG: NO MATCH FOUND for tracked packet");
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
        Self::new()
    }
}

/// Information extracted from ICMP error about the embedded UDP packet
#[derive(Debug, Clone)]
pub struct EmbeddedUdpInfo {
    pub src_port: u16,
    pub dest_addr: SocketAddr,
    pub payload_prefix: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};
    
    #[tokio::test]
    async fn test_packet_tracker_basic() {
        let tracker = PacketTracker::new();
        
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
            options,
        ).await;
        
        assert_eq!(tracker.tracked_count().await, 1);
    }
    
    #[tokio::test]
    async fn test_packet_expiry() {
        let tracker = PacketTracker::new();
        
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
            options,
        ).await;
        
        assert_eq!(tracker.tracked_count().await, 1);
        
        // Wait for expiry
        tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
        
        // Should be cleaned up
        assert_eq!(tracker.tracked_count().await, 0);
    }
}
