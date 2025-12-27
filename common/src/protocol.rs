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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestProbePacket {
    pub test_seq: u64,
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

/// Message sent from server to client to report traceroute hop information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceHopMessage {
    /// Hop number (TTL value)
    pub hop: u8,
    
    /// IP address of the hop (if available from ICMP)
    pub ip_address: Option<String>,
    
    /// Round-trip time to this hop in milliseconds
    pub rtt_ms: f64,
    
    /// Human-readable message about this hop
    pub message: String,
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
    
    /// IP address of the router that sent the ICMP error
    pub router_ip: Option<String>,
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

    #[test]
    fn test_testprobe_packet_serialization() {
        let packet = TestProbePacket {
            test_seq: 123,
            timestamp_ms: 9876543210,
            direction: Direction::ServerToClient,
            send_options: Some(SendOptions {
                ttl: Some(5),
                df_bit: Some(true),
                tos: None,
                flow_label: None,
                track_for_ms: 5000,
            }),
        };

        let json = serde_json::to_string(&packet).unwrap();
        let deserialized: TestProbePacket = serde_json::from_str(&json).unwrap();

        assert_eq!(packet, deserialized);
        assert_eq!(deserialized.test_seq, 123);
        assert_eq!(deserialized.send_options.as_ref().unwrap().ttl, Some(5));
    }

    #[test]
    fn test_probe_and_testprobe_have_different_json() {
        let probe = ProbePacket {
            seq: 42,
            timestamp_ms: 1000,
            direction: Direction::ServerToClient,
            send_options: None,
        };
        
        let testprobe = TestProbePacket {
            test_seq: 42,
            timestamp_ms: 1000,
            direction: Direction::ServerToClient,
            send_options: None,
        };
        
        let probe_json = serde_json::to_string(&probe).unwrap();
        let testprobe_json = serde_json::to_string(&testprobe).unwrap();
        
        // Verify they serialize to different JSON
        assert_ne!(probe_json, testprobe_json, 
            "ProbePacket and TestProbePacket should serialize to different JSON structures");
        
        // Verify probe has "seq" field
        assert!(probe_json.contains("\"seq\":"), 
            "ProbePacket JSON should contain 'seq' field");
        
        // Verify testprobe has "test_seq" field
        assert!(testprobe_json.contains("\"test_seq\":"), 
            "TestProbePacket JSON should contain 'test_seq' field");
        
        // Verify testprobe JSON does NOT have "seq" field (it has "test_seq" instead)
        assert!(!testprobe_json.contains("\"seq\":"), 
            "TestProbePacket JSON should NOT contain 'seq' field, only 'test_seq'");
    }
}
