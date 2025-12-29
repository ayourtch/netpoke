use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::Mutex;
use std::collections::{HashMap, VecDeque};
use std::time::Instant;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::data_channel::RTCDataChannel;
use common::ClientMetrics;
use crate::packet_tracker::{PacketTracker, UdpPacketInfo};
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct AppState {
    pub clients: Arc<RwLock<HashMap<String, Arc<ClientSession>>>>,
    pub packet_tracker: Arc<PacketTracker>,
    pub tracking_sender: mpsc::UnboundedSender<UdpPacketInfo>,
    pub server_start_time: Instant,
    /// Channel for sending peer connections that need to be closed
    /// This is used when signaling fails to prevent resource leaks
    pub peer_cleanup_sender: mpsc::UnboundedSender<Arc<RTCPeerConnection>>,
}

pub struct ClientSession {
    pub id: String,
    pub parent_id: Option<String>,
    pub ip_version: Option<String>,
    pub mode: Option<String>,  // "measurement" or "traceroute"
    /// Connection ID (UUID) for multi-path ECMP testing
    pub conn_id: String,
    pub peer_connection: Arc<RTCPeerConnection>,
    pub data_channels: Arc<RwLock<DataChannels>>,
    pub metrics: Arc<RwLock<ClientMetrics>>,
    pub measurement_state: Arc<RwLock<MeasurementState>>,
    pub connected_at: Instant,
    pub ice_candidates: Arc<Mutex<VecDeque<String>>>,
    pub peer_address: Arc<Mutex<Option<(String, u16)>>>, // (address, port)
    pub packet_tracker: Arc<PacketTracker>, // For ICMP correlation
    // ICMP error tracking for session cleanup
    pub icmp_error_count: Arc<Mutex<u32>>,
    pub last_icmp_error: Arc<Mutex<Option<Instant>>>,
}

pub struct DataChannels {
    pub probe: Option<Arc<RTCDataChannel>>,
    pub bulk: Option<Arc<RTCDataChannel>>,
    pub control: Option<Arc<RTCDataChannel>>,
    pub testprobe: Option<Arc<RTCDataChannel>>,
}

pub struct MeasurementState {
    pub probe_seq: u64,
    pub testprobe_seq: u64,  // Separate sequence space for traceroute test probes
    pub current_ttl: u8,  // Current TTL for traceroute
    pub stop_traceroute: bool,  // Flag to stop traceroute sender
    pub traceroute_started_at: Option<Instant>,  // When traceroute started (for timeout)
    pub bulk_bytes_sent: u64,
    pub received_probes: VecDeque<ReceivedProbe>,
    pub received_bulk_bytes: VecDeque<ReceivedBulk>,
    pub sent_bulk_packets: VecDeque<SentBulk>,
    pub sent_probes: VecDeque<SentProbe>,  // Track sent S2C probes
    pub sent_probes_map: HashMap<u64, SentProbe>,  // Fast lookup by seq for sent probes
    pub echoed_probes: VecDeque<EchoedProbe>,  // Track echoed S2C probes
    pub sent_testprobes: VecDeque<SentProbe>,  // Track sent test probes for traceroute
    pub sent_testprobes_map: HashMap<u64, SentProbe>,  // Fast lookup by seq for test probes
    pub echoed_testprobes: VecDeque<EchoedProbe>,  // Track echoed test probes
    pub last_received_seq: Option<u64>,
}

#[derive(Clone)]
pub struct ReceivedProbe {
    pub seq: u64,
    pub sent_at_ms: u64,
    pub received_at_ms: u64,
}

#[derive(Clone)]
pub struct ReceivedBulk {
    pub bytes: u64,
    pub received_at_ms: u64,
}

#[derive(Clone)]
pub struct SentBulk {
    pub bytes: u64,
    pub sent_at_ms: u64,
}

#[derive(Clone)]
pub struct SentProbe {
    pub seq: u64,
    pub sent_at_ms: u64,
}

#[derive(Clone)]
pub struct EchoedProbe {
    pub seq: u64,
    pub sent_at_ms: u64,
    pub echoed_at_ms: u64,  // When client received it and echoed back
}

impl AppState {
    /// Creates a new AppState and returns both it and the receiver for peer connection cleanup
    pub fn new() -> (Self, mpsc::UnboundedReceiver<Arc<RTCPeerConnection>>) {
        let (tracker, tx) = PacketTracker::new();
        let (cleanup_tx, cleanup_rx) = mpsc::unbounded_channel();
        let state = Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            packet_tracker: Arc::new(tracker),
            tracking_sender: tx,
            server_start_time: Instant::now(),
            peer_cleanup_sender: cleanup_tx,
        };
        (state, cleanup_rx)
    }
}

impl DataChannels {
    pub fn new() -> Self {
        Self {
            probe: None,
            bulk: None,
            control: None,
            testprobe: None,
        }
    }
}

impl MeasurementState {
    pub fn new() -> Self {
        Self {
            probe_seq: 0,
            testprobe_seq: 0,
            current_ttl: 1,  // Start at TTL 1
            stop_traceroute: false,  // Initialize to false
            traceroute_started_at: None,  // Not started yet
            bulk_bytes_sent: 0,
            received_probes: VecDeque::new(),
            received_bulk_bytes: VecDeque::new(),
            sent_bulk_packets: VecDeque::new(),
            sent_probes: VecDeque::new(),
            sent_probes_map: HashMap::new(),
            echoed_probes: VecDeque::new(),
            sent_testprobes: VecDeque::new(),
            sent_testprobes_map: HashMap::new(),
            echoed_testprobes: VecDeque::new(),
            last_received_seq: None,
        }
    }
}
