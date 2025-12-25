use serde::{Deserialize, Serialize};
use crate::metrics::ClientMetrics;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Direction {
    ClientToServer,
    ServerToClient,
}

/// UDP socket options for packet transmission
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub struct SendOptions {
    /// Time To Live (IPv4) or Hop Limit (IPv6)
    pub ttl: Option<u8>,
    
    /// Don't Fragment bit (IPv4 only)
    pub df_bit: Option<bool>,
    
    /// Type of Service (IPv4) or Traffic Class (IPv6)
    pub tos: Option<u8>,
    
    /// Flow Label (IPv6 only)
    pub flow_label: Option<u32>,
    
    /// Track this packet for ICMP correlation (milliseconds, 0 = no tracking)
    pub track_for_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProbePacket {
    pub seq: u64,
    pub timestamp_ms: u64,
    pub direction: Direction,
    
    /// Optional send options for this packet
    #[serde(skip_serializing_if = "Option::is_none")]
    pub send_options: Option<SendOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkPacket {
    pub data: Vec<u8>,
    
    /// Optional send options for this packet
    #[serde(skip_serializing_if = "Option::is_none")]
    pub send_options: Option<SendOptions>,
}

impl BulkPacket {
    pub fn new(size: usize) -> Self {
        Self {
            data: vec![0u8; size],
            send_options: None,
        }
    }
    
    pub fn with_options(size: usize, options: SendOptions) -> Self {
        Self {
            data: vec![0u8; size],
            send_options: Some(options),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ClientInfo {
    pub id: String,
    pub parent_id: Option<String>,
    pub ip_version: Option<String>,
    pub connected_at: u64,
    pub metrics: ClientMetrics,
    pub peer_address: Option<String>,
    pub peer_port: Option<u16>,
    pub current_seq: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardMessage {
    pub clients: Vec<ClientInfo>,
}

/// Event generated when an ICMP error matches a tracked packet
#[derive(Debug, Clone)]
pub struct TrackedPacketEvent {
    /// Raw ICMP error packet
    pub icmp_packet: Vec<u8>,
    
    /// Original UDP packet that was sent
    pub udp_packet: Vec<u8>,
    
    /// Original cleartext data that was sent
    pub cleartext: Vec<u8>,
    
    /// When the UDP packet was sent
    pub sent_at: Instant,
    
    /// When the ICMP error was received
    pub icmp_received_at: Instant,
    
    /// Send options that were used
    pub send_options: SendOptions,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_probe_packet_serialization() {
        let packet = ProbePacket {
            seq: 42,
            timestamp_ms: 1234567890,
            direction: Direction::ClientToServer,
            send_options: None,
        };

        let json = serde_json::to_string(&packet).unwrap();
        let deserialized: ProbePacket = serde_json::from_str(&json).unwrap();

        assert_eq!(packet, deserialized);
    }

    #[test]
    fn test_bulk_packet_creation() {
        let packet = BulkPacket::new(1024);
        assert_eq!(packet.data.len(), 1024);
    }

    #[test]
    fn test_dashboard_message_serialization() {
        let msg = DashboardMessage {
            clients: vec![
                ClientInfo {
                    id: "client-1".to_string(),
                    parent_id: Some("parent-1".to_string()),
                    ip_version: Some("ipv4".to_string()),
                    connected_at: 1234567890,
                    metrics: ClientMetrics::default(),
                    peer_address: Some("192.168.1.100".to_string()),
                    peer_port: Some(54321),
                    current_seq: 42,
                }
            ],
        };

        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: DashboardMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(msg.clients.len(), deserialized.clients.len());
        assert_eq!(deserialized.clients[0].id, "client-1");
        assert_eq!(deserialized.clients[0].current_seq, 42);
    }
}
