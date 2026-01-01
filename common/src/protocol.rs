use serde::{Deserialize, Serialize};
use crate::metrics::ClientMetrics;
use std::time::Instant;

/// IP address family to use for ICE candidates
/// This controls which network types are used during ICE candidate gathering
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum IpFamily {
    /// Use only IPv4 addresses (UDP4)
    #[serde(alias = "ipv4", alias = "4")]
    IPv4,
    /// Use only IPv6 addresses (UDP6)
    #[serde(alias = "ipv6", alias = "6")]
    IPv6,
    /// Use both IPv4 and IPv6 addresses (default)
    #[default]
    #[serde(alias = "any", alias = "all")]
    Both,
}

impl IpFamily {
    /// Parse from a string representation
    pub fn from_str_loose(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "ipv4" | "4" | "v4" => IpFamily::IPv4,
            "ipv6" | "6" | "v6" => IpFamily::IPv6,
            _ => IpFamily::Both,
        }
    }
}

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
    
    /// Connection ID for multi-path ECMP testing (UUID string)
    /// Defaults to empty string for backwards compatibility
    #[serde(default)]
    pub conn_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestProbePacket {
    pub test_seq: u64,
    pub timestamp_ms: u64,
    pub direction: Direction,
    
    /// Optional send options for this packet
    #[serde(skip_serializing_if = "Option::is_none")]
    pub send_options: Option<SendOptions>,
    
    /// Connection ID for multi-path ECMP testing (UUID string)
    /// Defaults to empty string for backwards compatibility
    #[serde(default)]
    pub conn_id: String,
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

/// Diagnostics information for a single client session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDiagnostics {
    pub session_id: String,
    pub parent_id: Option<String>,
    pub ip_version: Option<String>,
    pub mode: Option<String>,
    pub conn_id: String,
    pub connected_at_secs: u64,
    pub connection_state: String,
    pub ice_connection_state: String,
    pub ice_gathering_state: String,
    pub peer_address: Option<String>,
    pub peer_port: Option<u16>,
    pub candidate_pairs: Vec<CandidatePairInfo>,
    pub data_channels: DataChannelStatus,
    pub icmp_error_count: u32,
    pub last_icmp_error_secs_ago: Option<u64>,
}

/// ICE candidate pair information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidatePairInfo {
    pub local_candidate_type: String,
    pub local_address: String,
    pub remote_candidate_type: String,
    pub remote_address: String,
    pub state: String,
    pub nominated: bool,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

/// Data channel status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataChannelStatus {
    pub probe: Option<String>,
    pub bulk: Option<String>,
    pub control: Option<String>,
    pub testprobe: Option<String>,
}

/// Server diagnostics information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerDiagnostics {
    pub server_uptime_secs: u64,
    pub total_sessions: usize,
    pub connected_sessions: usize,
    pub disconnected_sessions: usize,
    pub failed_sessions: usize,
    pub sessions: Vec<SessionDiagnostics>,
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
    
    /// Connection ID for multi-path ECMP testing (UUID string)
    /// Defaults to empty string for backwards compatibility
    #[serde(default)]
    pub conn_id: String,
    
    /// Survey session ID (UUID) for cross-correlation
    #[serde(default)]
    pub survey_session_id: String,
    
    /// Source port from the original UDP packet
    #[serde(default)]
    pub original_src_port: u16,
    
    /// Destination address (IP:port) from the original UDP packet
    #[serde(default)]
    pub original_dest_addr: String,
}

/// Message sent from client to server to stop traceroute probes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopTracerouteMessage {
    /// Connection ID for multi-path ECMP testing (UUID string)
    /// Defaults to empty string for backwards compatibility
    #[serde(default)]
    pub conn_id: String,
    
    /// Survey session ID (UUID) for cross-correlation
    #[serde(default)]
    pub survey_session_id: String,
}

/// Message sent from client to server to start traceroute probes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartTracerouteMessage {
    /// Connection ID for multi-path ECMP testing (UUID string)
    /// Defaults to empty string for backwards compatibility
    #[serde(default)]
    pub conn_id: String,
    
    /// Survey session ID (UUID) for cross-correlation
    #[serde(default)]
    pub survey_session_id: String,
}

/// Message sent from client to server to start a survey session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartSurveySessionMessage {
    /// Unique survey session ID (UUID) for cross-correlation
    pub survey_session_id: String,
    
    /// Connection ID for multi-path ECMP testing (UUID string)
    #[serde(default)]
    pub conn_id: String,
}

/// Message sent from server to client when all channels are ready
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSideReadyMessage {
    /// Connection ID for multi-path ECMP testing (UUID string)
    #[serde(default)]
    pub conn_id: String,
    
    /// Survey session ID (UUID) for cross-correlation
    #[serde(default)]
    pub survey_session_id: String,
}

/// Message sent from client to server to start MTU traceroute probes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartMtuTracerouteMessage {
    /// Connection ID for multi-path ECMP testing (UUID string)
    #[serde(default)]
    pub conn_id: String,
    
    /// Survey session ID (UUID) for cross-correlation
    #[serde(default)]
    pub survey_session_id: String,
    
    /// Target packet size to probe (including headers)
    pub packet_size: u32,
}

/// MTU hop message sent from server to client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtuHopMessage {
    /// Hop number (TTL value)
    pub hop: u8,
    
    /// IP address of the hop (if available from ICMP)
    pub ip_address: Option<String>,
    
    /// Round-trip time to this hop in milliseconds
    pub rtt_ms: f64,
    
    /// MTU value from ICMP "Fragmentation Needed" message (if available)
    pub mtu: Option<u16>,
    
    /// Human-readable message about this hop
    pub message: String,
    
    /// Connection ID for multi-path ECMP testing (UUID string)
    #[serde(default)]
    pub conn_id: String,
    
    /// Survey session ID (UUID) for cross-correlation
    #[serde(default)]
    pub survey_session_id: String,
    
    /// Packet size that was used for this probe
    pub packet_size: u32,
}

/// Message sent from client to server to request measuring time limit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetMeasuringTimeMessage {
    /// Connection ID for multi-path ECMP testing (UUID string)
    #[serde(default)]
    pub conn_id: String,
    
    /// Survey session ID (UUID) for cross-correlation
    #[serde(default)]
    pub survey_session_id: String,
}

/// Response from server with measuring time limit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasuringTimeResponseMessage {
    /// Connection ID for multi-path ECMP testing (UUID string)
    #[serde(default)]
    pub conn_id: String,
    
    /// Survey session ID (UUID) for cross-correlation
    #[serde(default)]
    pub survey_session_id: String,
    
    /// Maximum duration in milliseconds for the performance measurement session
    pub max_duration_ms: u64,
}

/// Message sent from client to server to start server-side traffic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartServerTrafficMessage {
    /// Connection ID for multi-path ECMP testing (UUID string)
    #[serde(default)]
    pub conn_id: String,
    
    /// Survey session ID (UUID) for cross-correlation
    #[serde(default)]
    pub survey_session_id: String,
}

/// Message sent from client to server to stop server-side traffic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopServerTrafficMessage {
    /// Connection ID for multi-path ECMP testing (UUID string)
    #[serde(default)]
    pub conn_id: String,
    
    /// Survey session ID (UUID) for cross-correlation
    #[serde(default)]
    pub survey_session_id: String,
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
    
    /// Connection ID extracted from the packet (for per-session event routing)
    pub conn_id: String,
    
    /// Source port from the original UDP packet (extracted from ICMP embedded data)
    pub original_src_port: u16,
    
    /// Destination address (IP:port) from the original UDP packet
    pub original_dest_addr: String,
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
            conn_id: String::new(),
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
            conn_id: String::new(),
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
            conn_id: String::new(),
        };
        
        let testprobe = TestProbePacket {
            test_seq: 42,
            timestamp_ms: 1000,
            direction: Direction::ServerToClient,
            send_options: None,
            conn_id: String::new(),
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

    #[test]
    fn test_ip_family_default() {
        let family: IpFamily = Default::default();
        assert_eq!(family, IpFamily::Both);
    }

    #[test]
    fn test_ip_family_from_str_loose() {
        assert_eq!(IpFamily::from_str_loose("ipv4"), IpFamily::IPv4);
        assert_eq!(IpFamily::from_str_loose("IPV4"), IpFamily::IPv4);
        assert_eq!(IpFamily::from_str_loose("4"), IpFamily::IPv4);
        assert_eq!(IpFamily::from_str_loose("v4"), IpFamily::IPv4);
        
        assert_eq!(IpFamily::from_str_loose("ipv6"), IpFamily::IPv6);
        assert_eq!(IpFamily::from_str_loose("IPV6"), IpFamily::IPv6);
        assert_eq!(IpFamily::from_str_loose("6"), IpFamily::IPv6);
        assert_eq!(IpFamily::from_str_loose("v6"), IpFamily::IPv6);
        
        assert_eq!(IpFamily::from_str_loose("both"), IpFamily::Both);
        assert_eq!(IpFamily::from_str_loose("any"), IpFamily::Both);
        assert_eq!(IpFamily::from_str_loose("unknown"), IpFamily::Both);
    }

    #[test]
    fn test_ip_family_serialization() {
        let ipv4_json = serde_json::to_string(&IpFamily::IPv4).unwrap();
        let ipv6_json = serde_json::to_string(&IpFamily::IPv6).unwrap();
        let both_json = serde_json::to_string(&IpFamily::Both).unwrap();
        
        assert_eq!(ipv4_json, "\"ipv4\"");
        assert_eq!(ipv6_json, "\"ipv6\"");
        assert_eq!(both_json, "\"both\"");
        
        // Test deserialization
        assert_eq!(serde_json::from_str::<IpFamily>("\"ipv4\"").unwrap(), IpFamily::IPv4);
        assert_eq!(serde_json::from_str::<IpFamily>("\"ipv6\"").unwrap(), IpFamily::IPv6);
        assert_eq!(serde_json::from_str::<IpFamily>("\"both\"").unwrap(), IpFamily::Both);
        
        // Test aliases
        assert_eq!(serde_json::from_str::<IpFamily>("\"4\"").unwrap(), IpFamily::IPv4);
        assert_eq!(serde_json::from_str::<IpFamily>("\"6\"").unwrap(), IpFamily::IPv6);
        assert_eq!(serde_json::from_str::<IpFamily>("\"any\"").unwrap(), IpFamily::Both);
        assert_eq!(serde_json::from_str::<IpFamily>("\"all\"").unwrap(), IpFamily::Both);
    }
}
