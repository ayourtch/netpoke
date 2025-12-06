use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::broadcast;
use std::collections::{HashMap, VecDeque};
use std::time::Instant;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::data_channel::RTCDataChannel;
use common::ClientMetrics;

#[derive(Clone)]
pub struct AppState {
    pub clients: Arc<RwLock<HashMap<String, Arc<ClientSession>>>>,
}

pub struct ClientSession {
    pub id: String,
    pub peer_connection: Arc<RTCPeerConnection>,
    pub data_channels: Arc<RwLock<DataChannels>>,
    pub metrics: Arc<RwLock<ClientMetrics>>,
    pub measurement_state: Arc<RwLock<MeasurementState>>,
    pub connected_at: Instant,
    pub ice_candidate_tx: broadcast::Sender<String>,
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

impl AppState {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
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
            last_received_seq: None,
        }
    }
}
