//! iperf3 protocol definitions.
//!
//! The iperf3 protocol uses a control connection (TCP) and one or more
//! data streams (TCP or UDP) for testing.
//!
//! Control messages are JSON objects with a 4-byte length prefix (big-endian).

use serde::{Deserialize, Serialize};

/// iperf3 protocol states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i8)]
pub enum State {
    /// Initial state, waiting for parameters
    ParamExchange = 9,
    /// Creating test streams
    CreateStreams = 10,
    /// Running the test
    TestStart = 1,
    /// Test is running
    TestRunning = 2,
    /// Test is ending
    TestEnd = 4,
    /// Exchanging results
    ExchangeResults = 13,
    /// Displaying results
    DisplayResults = 14,
    /// IPERF_DONE state
    IperfDone = 16,
    /// Server finished
    ServerTerminate = 15,
    /// Access denied
    AccessDenied = -1,
    /// Server error
    ServerError = -2,
}

impl State {
    pub fn from_byte(b: u8) -> Option<Self> {
        // Convert to signed byte for proper comparison
        let signed = b as i8;
        match signed {
            9 => Some(State::ParamExchange),
            10 => Some(State::CreateStreams),
            1 => Some(State::TestStart),
            2 => Some(State::TestRunning),
            4 => Some(State::TestEnd),
            13 => Some(State::ExchangeResults),
            14 => Some(State::DisplayResults),
            16 => Some(State::IperfDone),
            15 => Some(State::ServerTerminate),
            -1 => Some(State::AccessDenied),
            -2 => Some(State::ServerError),
            _ => None,
        }
    }

    pub fn to_byte(self) -> u8 {
        (self as i8) as u8
    }
}

/// Test parameters sent by the client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestParameters {
    /// Test protocol: "TCP" or "UDP"
    #[serde(default = "default_protocol")]
    pub protocol: String,

    /// Test duration in seconds
    #[serde(default = "default_duration")]
    pub time: u64,

    /// Number of parallel streams
    #[serde(default = "default_streams")]
    pub parallel: u32,

    /// Reverse mode: server sends to client
    #[serde(default)]
    pub reverse: bool,

    /// Bidirectional mode
    #[serde(default)]
    pub bidirectional: bool,

    /// UDP bandwidth target (bits/second)
    #[serde(default)]
    pub bandwidth: u64,

    /// Block size for writes
    #[serde(default = "default_blksize")]
    pub blksize: u32,

    /// Window size (socket buffer size)
    #[serde(default)]
    pub window: u32,

    /// MSS for TCP
    #[serde(default)]
    pub mss: u32,

    /// No delay (TCP_NODELAY)
    #[serde(default)]
    pub nodelay: bool,

    /// Number of bytes to transmit (0 = use time)
    #[serde(default)]
    pub bytes: u64,

    /// Number of blocks to transmit (0 = use time/bytes)
    #[serde(default)]
    pub blockcount: u64,

    /// Omit first N seconds from statistics
    #[serde(default)]
    pub omit: u32,

    /// Client version string
    #[serde(default)]
    pub client_version: String,

    /// Use UDP for test
    #[serde(default)]
    pub udp: bool,

    /// Interval for periodic reports (seconds)
    #[serde(default = "default_interval")]
    pub interval: f64,
}

fn default_protocol() -> String {
    "TCP".to_string()
}

fn default_duration() -> u64 {
    10
}

fn default_streams() -> u32 {
    1
}

fn default_blksize() -> u32 {
    128 * 1024 // 128 KB default for TCP
}

fn default_interval() -> f64 {
    1.0
}

impl Default for TestParameters {
    fn default() -> Self {
        Self {
            protocol: default_protocol(),
            time: default_duration(),
            parallel: default_streams(),
            reverse: false,
            bidirectional: false,
            bandwidth: 0,
            blksize: default_blksize(),
            window: 0,
            mss: 0,
            nodelay: false,
            bytes: 0,
            blockcount: 0,
            omit: 0,
            client_version: String::new(),
            udp: false,
            interval: default_interval(),
        }
    }
}

/// Results from a test stream
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StreamResult {
    /// Stream ID
    pub id: u32,

    /// Bytes transferred
    pub bytes: u64,

    /// Duration in seconds (fractional)
    pub seconds: f64,

    /// Bits per second
    pub bits_per_second: f64,

    /// Retransmits (TCP only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retransmits: Option<u64>,

    /// Jitter in milliseconds (UDP only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jitter_ms: Option<f64>,

    /// Lost packets (UDP only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lost_packets: Option<u64>,

    /// Total packets (UDP only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packets: Option<u64>,

    /// Lost percentage (UDP only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lost_percent: Option<f64>,
}

/// Interval result for periodic reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntervalResult {
    /// Streams in this interval
    pub streams: Vec<StreamResult>,

    /// Sum of all streams
    pub sum: StreamResult,
}

/// Server results sent at test end
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerResults {
    /// Start time info
    pub start: StartInfo,

    /// Intervals during the test
    pub intervals: Vec<IntervalResult>,

    /// End (final) results
    pub end: EndInfo,
}

/// Start information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartInfo {
    /// Connected info
    pub connected: Vec<ConnectedInfo>,

    /// Version string
    pub version: String,

    /// System info
    pub system_info: String,

    /// Test start info
    pub test_start: TestStartInfo,
}

/// Connection info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedInfo {
    pub socket: i32,
    pub local_host: String,
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
}

/// Test start parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestStartInfo {
    pub protocol: String,
    pub num_streams: u32,
    pub blksize: u32,
    pub omit: u32,
    pub duration: u64,
    pub bytes: u64,
    pub blocks: u64,
    pub reverse: bool,
}

/// End (final) results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndInfo {
    /// Per-stream results
    pub streams: Vec<StreamEndResult>,

    /// Sum of sending direction
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sum_sent: Option<StreamResult>,

    /// Sum of receiving direction
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sum_received: Option<StreamResult>,

    /// CPU utilization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_utilization_percent: Option<CpuUtilization>,
}

/// Per-stream end result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEndResult {
    /// Sender results
    pub sender: StreamResult,

    /// Receiver results
    pub receiver: StreamResult,
}

/// CPU utilization info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuUtilization {
    pub host_total: f64,
    pub host_user: f64,
    pub host_system: f64,
    pub remote_total: f64,
    pub remote_user: f64,
    pub remote_system: f64,
}

/// Cookie length for stream identification
pub const COOKIE_SIZE: usize = 37;

/// iperf3 magic number for UDP packets
pub const UDP_HEADER_SIZE: usize = 12;
