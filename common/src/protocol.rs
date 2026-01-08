use crate::metrics::ClientMetrics;
use serde::{Deserialize, Serialize};
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

// ============ Probe Stream Constants ============
// These constants ensure consistency between client and server implementations

/// Probe stream rate in packets per second
pub const PROBE_STREAM_PPS: u32 = 100;

/// Probe interval in milliseconds (derived from PPS)
pub const PROBE_INTERVAL_MS: u32 = 1000 / PROBE_STREAM_PPS;

/// Minimum sample count before applying outlier exclusion for baseline calculation
pub const BASELINE_MIN_SAMPLES: u64 = 10;

/// Multiplier for outlier exclusion (values > baseline * this are excluded)
pub const BASELINE_OUTLIER_MULTIPLIER: f64 = 3.0;

/// Minimum threshold for outlier detection in milliseconds.
/// This ensures outlier detection works correctly even when baseline is near zero
/// (e.g., with minimal clock skew or very low latency connections).
pub const BASELINE_MIN_THRESHOLD_MS: f64 = 100.0;

/// Duration to keep probes for stats calculation (milliseconds)
pub const PROBE_STATS_WINDOW_MS: u64 = 2000;

/// Duration to keep probes for feedback calculation (milliseconds)  
pub const PROBE_FEEDBACK_WINDOW_MS: u64 = 1000;

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

    /// SECURITY WARNING: Bypass DTLS encryption and send cleartext directly to UDP
    ///
    /// When enabled, this bypasses all encryption and authentication. Data will be
    /// transmitted in cleartext and visible to network observers. NO integrity
    /// checking or confidentiality protection will be applied.
    ///
    /// This is ONLY intended for diagnostic MTU discovery packets where DTLS framing
    /// would interfere with precise packet size control. DO NOT use for any
    /// sensitive data, user information, or authentication tokens.
    #[serde(default)]
    pub bypass_dtls: bool,

    /// Bypass SCTP fragmentation and send data as a single chunk
    ///
    /// When enabled, this bypasses SCTP's normal fragmentation behavior which splits
    /// large messages into chunks based on max_payload_size (typically ~1200 bytes).
    /// The entire message will be sent as a single SCTP chunk, allowing packet sizes
    /// up to the interface MTU.
    ///
    /// This is ONLY intended for MTU discovery tests where you need to send packets
    /// larger than the SCTP fragmentation threshold. Use with bypass_dtls for full
    /// control over UDP packet size.
    ///
    /// WARNING: Sending very large packets may exceed the path MTU and get dropped
    /// or fragmented by intermediate routers. Only use this for controlled testing.
    #[serde(default)]
    pub bypass_sctp_fragmentation: bool,
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

/// Message sent from server to client when traceroute probes are done
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracerouteCompletedMessage {
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

    /// Max TTL to try to traceroute with
    pub path_ttl: i32,

    /// For how long to wait with the timeouts
    pub collect_timeout_ms: usize,
}

/// Message sent from server client when MTU traceroute probes are done
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtuTracerouteCompletedMessage {
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

/// Message sent from client to server to start probe streams for baseline measurement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartProbeStreamsMessage {
    /// Connection ID for multi-path ECMP testing (UUID string)
    #[serde(default)]
    pub conn_id: String,

    /// Survey session ID (UUID) for cross-correlation
    #[serde(default)]
    pub survey_session_id: String,
}

/// Message sent from client to server to stop probe streams
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopProbeStreamsMessage {
    /// Connection ID for multi-path ECMP testing (UUID string)
    #[serde(default)]
    pub conn_id: String,

    /// Survey session ID (UUID) for cross-correlation
    #[serde(default)]
    pub survey_session_id: String,
}

/// Statistics for a single direction of probe stream
/// Contains 50th percentile, 99th percentile, and full range values
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DirectionStats {
    /// One-way delay deviation from baseline in milliseconds
    /// [0] = 50th percentile (median), [1] = 99th percentile, [2] = min, [3] = max
    pub delay_deviation_ms: [f64; 4],

    /// RTT in milliseconds (if available from echo)
    /// [0] = 50th percentile, [1] = 99th percentile, [2] = min, [3] = max
    pub rtt_ms: [f64; 4],

    /// Jitter in milliseconds
    /// [0] = 50th percentile, [1] = 99th percentile, [2] = min, [3] = max
    pub jitter_ms: [f64; 4],

    /// Loss rate as percentage
    pub loss_rate: f64,

    /// Reorder rate as percentage
    pub reorder_rate: f64,

    /// Number of probes used in this calculation
    pub probe_count: u32,

    /// Baseline average delay in milliseconds (incrementally calculated)
    pub baseline_delay_ms: f64,
}

/// Per-second statistics report sent on control channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeStatsReport {
    /// Connection ID for multi-path ECMP testing (UUID string)
    #[serde(default)]
    pub conn_id: String,

    /// Survey session ID (UUID) for cross-correlation
    #[serde(default)]
    pub survey_session_id: String,

    /// Timestamp when this report was generated (ms since epoch)
    pub timestamp_ms: u64,

    /// Client-to-server direction statistics (measured by server)
    pub c2s_stats: DirectionStats,

    /// Server-to-client direction statistics (measured by client)
    pub s2c_stats: DirectionStats,
}

/// Compact feedback about received probes from the other direction
/// Included in each probe to allow the sender to calculate stats without waiting
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProbeFeedback {
    /// Highest sequence number received so far
    pub highest_seq: u64,

    /// Timestamp when highest seq was received (ms since epoch)
    pub highest_seq_received_at_ms: u64,

    /// Count of probes received in the last second
    pub recent_count: u32,

    /// Count of out-of-order probes in the last second
    pub recent_reorders: u32,
}

/// Probe packet for bidirectional measurement streams
/// Sent at 100pps on the "probe" channel (unreliable, unordered)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasurementProbePacket {
    /// Sequence number (monotonically increasing per direction)
    pub seq: u64,

    /// Timestamp when this probe was sent (ms since epoch)
    pub sent_at_ms: u64,

    /// Direction of this probe
    pub direction: Direction,

    /// Connection ID for multi-path ECMP testing (UUID string)
    #[serde(default)]
    pub conn_id: String,

    /// Feedback about probes received from the other direction
    #[serde(default)]
    pub feedback: ProbeFeedback,
}

/// Enum wrapping all control message types for proper serialization/deserialization
/// This ensures each message type has a distinct JSON representation with a "type" tag
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlMessage {
    StartTraceroute(StartTracerouteMessage),
    StopTraceroute(StopTracerouteMessage),
    TracerouteCompleted(TracerouteCompletedMessage),
    StartSurveySession(StartSurveySessionMessage),
    ServerSideReady(ServerSideReadyMessage),
    StartMtuTraceroute(StartMtuTracerouteMessage),
    MtuTracerouteCompleted(MtuTracerouteCompletedMessage),
    TraceHop(TraceHopMessage),
    MtuHop(MtuHopMessage),
    GetMeasuringTime(GetMeasuringTimeMessage),
    MeasuringTimeResponse(MeasuringTimeResponseMessage),
    StartServerTraffic(StartServerTrafficMessage),
    StopServerTraffic(StopServerTrafficMessage),
    TestProbeMessageEcho(TestProbePacket),
    // Probe stream messages for baseline measurement
    StartProbeStreams(StartProbeStreamsMessage),
    StopProbeStreams(StopProbeStreamsMessage),
    ProbeStats(ProbeStatsReport),
}

/// Event generated when an ICMP error matches a tracked packet
#[derive(Debug, Clone)]
pub struct TrackedPacketEvent {
    /// Raw ICMP error packet
    pub icmp_packet: Vec<u8>,

    /// Original UDP packet that was sent
    pub udp_packet: Vec<u8>,

    /// Original IP packet length
    pub tracked_ip_length: usize,

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
            clients: vec![ClientInfo {
                id: "client-1".to_string(),
                parent_id: Some("parent-1".to_string()),
                ip_version: Some("ipv4".to_string()),
                connected_at: 1234567890,
                metrics: ClientMetrics::default(),
                peer_address: Some("192.168.1.100".to_string()),
                peer_port: Some(54321),
                current_seq: 42,
            }],
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
                bypass_dtls: false,
                bypass_sctp_fragmentation: false,
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
        assert_ne!(
            probe_json, testprobe_json,
            "ProbePacket and TestProbePacket should serialize to different JSON structures"
        );

        // Verify probe has "seq" field
        assert!(
            probe_json.contains("\"seq\":"),
            "ProbePacket JSON should contain 'seq' field"
        );

        // Verify testprobe has "test_seq" field
        assert!(
            testprobe_json.contains("\"test_seq\":"),
            "TestProbePacket JSON should contain 'test_seq' field"
        );

        // Verify testprobe JSON does NOT have "seq" field (it has "test_seq" instead)
        assert!(
            !testprobe_json.contains("\"seq\":"),
            "TestProbePacket JSON should NOT contain 'seq' field, only 'test_seq'"
        );
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
        assert_eq!(
            serde_json::from_str::<IpFamily>("\"ipv4\"").unwrap(),
            IpFamily::IPv4
        );
        assert_eq!(
            serde_json::from_str::<IpFamily>("\"ipv6\"").unwrap(),
            IpFamily::IPv6
        );
        assert_eq!(
            serde_json::from_str::<IpFamily>("\"both\"").unwrap(),
            IpFamily::Both
        );

        // Test aliases
        assert_eq!(
            serde_json::from_str::<IpFamily>("\"4\"").unwrap(),
            IpFamily::IPv4
        );
        assert_eq!(
            serde_json::from_str::<IpFamily>("\"6\"").unwrap(),
            IpFamily::IPv6
        );
        assert_eq!(
            serde_json::from_str::<IpFamily>("\"any\"").unwrap(),
            IpFamily::Both
        );
        assert_eq!(
            serde_json::from_str::<IpFamily>("\"all\"").unwrap(),
            IpFamily::Both
        );
    }

    #[test]
    fn test_control_message_serialization_uniqueness() {
        // Create messages with identical field values
        let start_traceroute = ControlMessage::StartTraceroute(StartTracerouteMessage {
            conn_id: "test-conn".to_string(),
            survey_session_id: "test-survey".to_string(),
        });

        let stop_traceroute = ControlMessage::StopTraceroute(StopTracerouteMessage {
            conn_id: "test-conn".to_string(),
            survey_session_id: "test-survey".to_string(),
        });

        let start_survey = ControlMessage::StartSurveySession(StartSurveySessionMessage {
            survey_session_id: "test-survey".to_string(),
            conn_id: "test-conn".to_string(),
        });

        // Serialize to JSON
        let start_traceroute_json = serde_json::to_string(&start_traceroute).unwrap();
        let stop_traceroute_json = serde_json::to_string(&stop_traceroute).unwrap();
        let start_survey_json = serde_json::to_string(&start_survey).unwrap();

        // Verify they serialize to DIFFERENT JSON (due to "type" field)
        assert_ne!(
            start_traceroute_json, stop_traceroute_json,
            "StartTraceroute and StopTraceroute should serialize differently"
        );
        assert_ne!(
            start_traceroute_json, start_survey_json,
            "StartTraceroute and StartSurveySession should serialize differently"
        );
        assert_ne!(
            stop_traceroute_json, start_survey_json,
            "StopTraceroute and StartSurveySession should serialize differently"
        );

        // Verify each has the correct "type" tag
        assert!(
            start_traceroute_json.contains("\"type\":\"start_traceroute\""),
            "StartTraceroute should have type tag: {}",
            start_traceroute_json
        );
        assert!(
            stop_traceroute_json.contains("\"type\":\"stop_traceroute\""),
            "StopTraceroute should have type tag: {}",
            stop_traceroute_json
        );
        assert!(
            start_survey_json.contains("\"type\":\"start_survey_session\""),
            "StartSurveySession should have type tag: {}",
            start_survey_json
        );
    }

    #[test]
    fn test_control_message_deserialization() {
        // Test that we can deserialize each message type correctly
        let start_traceroute_json = r#"{"type":"start_traceroute","conn_id":"test-conn","survey_session_id":"test-survey"}"#;
        let stop_traceroute_json =
            r#"{"type":"stop_traceroute","conn_id":"test-conn","survey_session_id":"test-survey"}"#;
        let start_survey_json = r#"{"type":"start_survey_session","survey_session_id":"test-survey","conn_id":"test-conn"}"#;

        let start_traceroute: ControlMessage = serde_json::from_str(start_traceroute_json).unwrap();
        let stop_traceroute: ControlMessage = serde_json::from_str(stop_traceroute_json).unwrap();
        let start_survey: ControlMessage = serde_json::from_str(start_survey_json).unwrap();

        // Verify correct variants were deserialized
        match start_traceroute {
            ControlMessage::StartTraceroute(msg) => {
                assert_eq!(msg.conn_id, "test-conn");
                assert_eq!(msg.survey_session_id, "test-survey");
            }
            _ => panic!("Expected StartTraceroute variant"),
        }

        match stop_traceroute {
            ControlMessage::StopTraceroute(msg) => {
                assert_eq!(msg.conn_id, "test-conn");
                assert_eq!(msg.survey_session_id, "test-survey");
            }
            _ => panic!("Expected StopTraceroute variant"),
        }

        match start_survey {
            ControlMessage::StartSurveySession(msg) => {
                assert_eq!(msg.survey_session_id, "test-survey");
                assert_eq!(msg.conn_id, "test-conn");
            }
            _ => panic!("Expected StartSurveySession variant"),
        }
    }

    #[test]
    fn test_control_message_roundtrip() {
        // Test all control message variants for roundtrip serialization
        let messages = vec![
            ControlMessage::StartTraceroute(StartTracerouteMessage {
                conn_id: "conn1".to_string(),
                survey_session_id: "survey1".to_string(),
            }),
            ControlMessage::StopTraceroute(StopTracerouteMessage {
                conn_id: "conn2".to_string(),
                survey_session_id: "survey2".to_string(),
            }),
            ControlMessage::StartSurveySession(StartSurveySessionMessage {
                survey_session_id: "survey3".to_string(),
                conn_id: "conn3".to_string(),
            }),
            ControlMessage::StartMtuTraceroute(StartMtuTracerouteMessage {
                conn_id: "conn4".to_string(),
                survey_session_id: "survey4".to_string(),
                packet_size: 1500,
                path_ttl: 15,
                collect_timeout_ms: 5000,
            }),
            ControlMessage::GetMeasuringTime(GetMeasuringTimeMessage {
                conn_id: "conn5".to_string(),
                survey_session_id: "survey5".to_string(),
            }),
            ControlMessage::StartServerTraffic(StartServerTrafficMessage {
                conn_id: "conn6".to_string(),
                survey_session_id: "survey6".to_string(),
            }),
            ControlMessage::StopServerTraffic(StopServerTrafficMessage {
                conn_id: "conn7".to_string(),
                survey_session_id: "survey7".to_string(),
            }),
        ];

        for msg in messages {
            let json = serde_json::to_string(&msg).unwrap();
            let deserialized: ControlMessage = serde_json::from_str(&json).unwrap();

            // Re-serialize and compare JSON (to ensure roundtrip is stable)
            let json2 = serde_json::to_string(&deserialized).unwrap();
            assert_eq!(json, json2, "Roundtrip serialization should be stable");
        }
    }

    /// Test that signed delay calculation handles clock skew correctly
    /// This verifies the approach used to fix clock synchronization issues in measurements
    #[test]
    fn test_signed_delay_calculation() {
        // Simulate a scenario where sender clock is ahead of receiver (negative delay)
        let sender_timestamp: u64 = 1000000; // sender time
        let receiver_timestamp: u64 = 999990; // receiver time (10ms behind sender)

        // Old approach with saturating_sub would give 0
        let old_delay = receiver_timestamp.saturating_sub(sender_timestamp);
        assert_eq!(
            old_delay, 0,
            "saturating_sub should give 0 for negative delay"
        );

        // New approach with signed arithmetic gives -10
        let new_delay = (receiver_timestamp as i64 - sender_timestamp as i64) as f64;
        assert_eq!(new_delay, -10.0, "signed arithmetic should give -10ms");

        // Simulate scenario where sender clock is behind receiver (positive delay)
        let sender_timestamp2: u64 = 999990;
        let receiver_timestamp2: u64 = 1000000;

        let new_delay2 = (receiver_timestamp2 as i64 - sender_timestamp2 as i64) as f64;
        assert_eq!(new_delay2, 10.0, "signed arithmetic should give +10ms");
    }

    /// Test that baseline calculation with clock skew produces meaningful deviations
    /// This demonstrates that even with clock offset, delay deviations are still accurate
    #[test]
    fn test_baseline_with_clock_skew() {
        // Simulate probes with sender clock ahead by 100ms (all delays appear as -100ms)
        // But we have some jitter of ±5ms around that baseline
        let clock_offset: i64 = -100; // sender ahead by 100ms
        let base_time: u64 = 1000000;

        let probes: Vec<(u64, u64)> = vec![
            (
                base_time + 100,
                (base_time as i64 + clock_offset + 102) as u64,
            ), // +2ms jitter
            (
                base_time + 200,
                (base_time as i64 + 100 + clock_offset + 99) as u64,
            ), // -1ms jitter
            (
                base_time + 300,
                (base_time as i64 + 200 + clock_offset + 103) as u64,
            ), // +3ms jitter
            (
                base_time + 400,
                (base_time as i64 + 300 + clock_offset + 97) as u64,
            ), // -3ms jitter
            (
                base_time + 500,
                (base_time as i64 + 400 + clock_offset + 100) as u64,
            ), // 0ms jitter
        ];

        // Calculate delays using signed arithmetic
        let delays: Vec<f64> = probes
            .iter()
            .map(|(sent, received)| (*received as i64 - *sent as i64) as f64)
            .collect();

        // All delays should be around -100 (the clock offset)
        for delay in &delays {
            assert!(
                *delay < -90.0 && *delay > -110.0,
                "delay {} should be around -100ms",
                delay
            );
        }

        // Calculate baseline
        let baseline: f64 = delays.iter().sum::<f64>() / delays.len() as f64;
        assert!(
            (baseline + 99.8).abs() < 1.0,
            "baseline should be around -100ms, got {}",
            baseline
        );

        // Calculate deviations from baseline - these should be small (the actual jitter)
        let deviations: Vec<f64> = delays.iter().map(|d| d - baseline).collect();

        // All deviations should be small (±5ms)
        for dev in &deviations {
            assert!(
                dev.abs() < 6.0,
                "deviation {} should be within ±5ms of baseline",
                dev
            );
        }
    }
}
