use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::Mutex;
use std::collections::{HashMap, VecDeque};
use std::time::Instant;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::data_channel::RTCDataChannel;
use common::ClientMetrics;
use crate::packet_tracker::PacketTracker;

#[derive(Clone)]
pub struct AppState {
    pub clients: Arc<RwLock<HashMap<String, Arc<ClientSession>>>>,
    pub packet_tracker: Arc<PacketTracker>,
}

pub struct ClientSession {
    pub id: String,
    pub parent_id: Option<String>,
    pub ip_version: Option<String>,
    pub peer_connection: Arc<RTCPeerConnection>,
    pub data_channels: Arc<RwLock<DataChannels>>,
    pub metrics: Arc<RwLock<ClientMetrics>>,
    pub measurement_state: Arc<RwLock<MeasurementState>>,
    pub connected_at: Instant,
    pub ice_candidates: Arc<Mutex<VecDeque<String>>>,
    pub peer_address: Arc<Mutex<Option<(String, u16)>>>, // (address, port)
}

pub struct DataChannels {
    pub probe: Option<Arc<RTCDataChannel>>,
    pub bulk: Option<Arc<RTCDataChannel>>,
    pub control: Option<Arc<RTCDataChannel>>,
}

pub struct MeasurementState {
    pub probe_seq: u64,
    pub bulk_bytes_sent: u64,
    pub received_probes: VecDeque<ReceivedProbe>,
    pub received_bulk_bytes: VecDeque<ReceivedBulk>,
    pub sent_bulk_packets: VecDeque<SentBulk>,
    pub sent_probes: VecDeque<SentProbe>,  // Track sent S2C probes
    pub echoed_probes: VecDeque<EchoedProbe>,  // Track echoed S2C probes
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
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            packet_tracker: Arc::new(PacketTracker::new()),
        }
    }
}

impl DataChannels {
    pub fn new() -> Self {
        Self {
            probe: None,
            bulk: None,
            control: None,
        }
    }
}

impl MeasurementState {
    pub fn new() -> Self {
        Self {
            probe_seq: 0,
            bulk_bytes_sent: 0,
            received_probes: VecDeque::new(),
            received_bulk_bytes: VecDeque::new(),
            sent_bulk_packets: VecDeque::new(),
            sent_probes: VecDeque::new(),
            echoed_probes: VecDeque::new(),
            last_received_seq: None,
        }
    }
}
