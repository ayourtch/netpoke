use crate::dtls_keylog::DtlsKeylogService;
use crate::metrics_recorder::MetricsRecorder;
use crate::packet_capture::PacketCaptureService;
use crate::packet_tracker::{PacketTracker, UdpPacketInfo};
use crate::session_manager::SessionManager;
use common::ClientMetrics;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use webrtc::data_channel::RTCDataChannel;
use webrtc::peer_connection::RTCPeerConnection;

#[derive(Clone)]
pub struct AppState {
    pub clients: Arc<InstrumentedRwLock<HashMap<String, Arc<ClientSession>>>>,
    pub packet_tracker: Arc<PacketTracker>,
    pub tracking_sender: mpsc::UnboundedSender<UdpPacketInfo>,
    pub server_start_time: Instant,
    /// Channel for sending peer connections that need to be closed
    /// This is used when signaling fails to prevent resource leaks
    pub peer_cleanup_sender: mpsc::UnboundedSender<Arc<RTCPeerConnection>>,
    /// Packet capture service for survey-specific pcap downloads
    pub capture_service: Option<Arc<PacketCaptureService>>,
    /// DTLS keylog service for storing encryption keys by survey session
    pub keylog_service: Option<Arc<DtlsKeylogService>>,
    /// Session manager for survey session lifecycle (database persistence)
    pub session_manager: Option<Arc<SessionManager>>,
    /// Metrics recorder for persisting probe statistics to database
    pub metrics_recorder: Option<Arc<MetricsRecorder>>,
}

#[derive(Debug)]
pub struct InstrumentedRwLock<T> {
    inner: tokio::sync::RwLock<T>,
    name: &'static str,
    last_read_loc: Arc<std::sync::Mutex<Option<&'static str>>>,
    last_write_loc: Arc<std::sync::Mutex<Option<&'static str>>>,
}

impl<T> InstrumentedRwLock<T> {
    pub fn new(name: &'static str, d: T) -> Self {
        Self {
            name,
            inner: RwLock::new(d),
            last_read_loc: Arc::new(std::sync::Mutex::new(None)),
            last_write_loc: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    pub async fn read(&self, label: &'static str) -> tokio::sync::RwLockReadGuard<'_, T> {
        // tracing::debug!("{}: waiting for read lock", self.name);
        let guard = self.inner.read().await;
        *self.last_read_loc.lock().unwrap() = Some(label);
        // tracing::debug!("{}: acquired read lock", self.name);
        guard
    }

    pub async fn write(&self, label: &'static str) -> tokio::sync::RwLockWriteGuard<'_, T> {
        // tracing::debug!("{}: waiting for write lock", self.name);
        let guard = self.inner.write().await;
        *self.last_write_loc.lock().unwrap() = Some(label);
        // tracing::debug!("{}: acquired write lock", self.name);
        guard
    }

    pub fn dump_locations(&self) {
        tracing::error!("=== Locations with locks: {} ===", self.name);
        if let Some(loc) = *self.last_read_loc.lock().unwrap() {
            tracing::error!("Last read: {}", loc);
        }
        if let Some(loc) = *self.last_write_loc.lock().unwrap() {
            tracing::error!("Last write: {}", loc);
        }
    }
}

pub struct ClientSession {
    pub id: String,
    pub parent_id: Option<String>,
    pub ip_version: Option<String>,
    pub mode: Option<String>, // "measurement" or "traceroute"
    /// Connection ID (UUID) for multi-path ECMP testing
    pub conn_id: String,
    /// Survey session ID (UUID) for cross-correlation across multiple connections
    pub survey_session_id: Arc<RwLock<String>>,
    pub peer_connection: Arc<RTCPeerConnection>,
    pub data_channels: Arc<RwLock<DataChannels>>,
    pub metrics: Arc<RwLock<ClientMetrics>>,
    pub measurement_state: Arc<RwLock<MeasurementState>>,
    pub connected_at: Instant,
    pub ice_candidates: Arc<Mutex<VecDeque<String>>>,
    pub peer_address: Arc<Mutex<Option<(String, u16)>>>, // (address, port)
    pub packet_tracker: Arc<PacketTracker>,              // For ICMP correlation
    // ICMP error tracking for session cleanup
    pub icmp_error_count: Arc<Mutex<u32>>,
    pub last_icmp_error: Arc<Mutex<Option<Instant>>>,
    /// Packet capture service for survey-specific pcap registration
    pub capture_service: Option<Arc<PacketCaptureService>>,
    /// DTLS keylog service for storing encryption keys by survey session
    pub keylog_service: Option<Arc<DtlsKeylogService>>,
    /// Session manager for survey session lifecycle (database persistence)
    pub session_manager: Option<Arc<SessionManager>>,
    /// Metrics recorder for persisting probe statistics to database
    pub metrics_recorder: Option<Arc<MetricsRecorder>>,
    /// Magic key for the current survey session (for database recording)
    pub magic_key: Arc<RwLock<Option<String>>>,
}

pub struct DataChannels {
    pub probe: Option<Arc<RTCDataChannel>>,
    pub bulk: Option<Arc<RTCDataChannel>>,
    pub control: Option<Arc<RTCDataChannel>>,
    pub testprobe: Option<Arc<RTCDataChannel>>,
}

pub struct MeasurementState {
    pub probe_seq: u64,
    pub testprobe_seq: u64, // Separate sequence space for traceroute test probes
    pub current_ttl: u8,    // Current TTL for traceroute
    pub path_ttl: Option<u8>, // TTL in the echoed probe packet
    pub stop_traceroute: bool, // Flag to stop traceroute sender
    pub traceroute_started_at: Option<Instant>, // When traceroute started (for timeout)
    pub traffic_active: bool, // Flag to indicate when traffic sending is active
    pub bulk_bytes_sent: u64,
    pub received_probes: VecDeque<ReceivedProbe>,
    pub received_bulk_bytes: VecDeque<ReceivedBulk>,
    pub sent_bulk_packets: VecDeque<SentBulk>,
    pub sent_probes: VecDeque<SentProbe>, // Track sent S2C probes
    pub sent_probes_map: HashMap<u64, SentProbe>, // Fast lookup by seq for sent probes
    pub echoed_probes: VecDeque<EchoedProbe>, // Track echoed S2C probes
    pub sent_testprobes: VecDeque<SentProbe>, // Track sent test probes for traceroute
    pub sent_testprobes_map: HashMap<u64, SentProbe>, // Fast lookup by seq for test probes
    pub echoed_testprobes: VecDeque<EchoedProbe>, // Track echoed test probes
    pub last_received_seq: Option<u64>,
    // Probe stream measurement fields
    pub probe_streams_active: bool, // Flag to indicate probe streams are active
    pub measurement_probe_seq: u64, // Sequence for measurement probes
    pub received_measurement_probes: VecDeque<ReceivedMeasurementProbe>, // Received measurement probes
    pub probe_stats: VecDeque<common::DirectionStats>, // Per-second calculated stats
    pub client_reported_s2c_stats: Option<common::DirectionStats>, // Stats reported by client
    pub baseline_delay_sum: f64,                       // Sum of delays for baseline calculation
    pub baseline_delay_count: u64,                     // Count for baseline calculation
    pub last_feedback: common::ProbeFeedback, // Last feedback to include in outgoing probes
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
    pub echoed_at_ms: u64, // When client received it and echoed back
}

/// Received measurement probe for probe stream measurements
#[derive(Clone)]
pub struct ReceivedMeasurementProbe {
    pub seq: u64,
    pub sent_at_ms: u64,
    pub received_at_ms: u64,
    pub feedback: common::ProbeFeedback,
}

impl AppState {
    /// Creates a new AppState and returns both it and the receiver for peer connection cleanup
    pub fn new() -> (Self, mpsc::UnboundedReceiver<Arc<RTCPeerConnection>>) {
        let (tracker, tx) = PacketTracker::new();
        let (cleanup_tx, cleanup_rx) = mpsc::unbounded_channel();
        let state = Self {
            // clients: Arc::new(RwLock::new(HashMap::new())),
            clients: Arc::new(InstrumentedRwLock::new("clients", HashMap::new())),
            packet_tracker: Arc::new(tracker),
            tracking_sender: tx,
            server_start_time: Instant::now(),
            peer_cleanup_sender: cleanup_tx,
            capture_service: None,   // Will be set after initialization
            keylog_service: None,    // Will be set after initialization
            session_manager: None,   // Will be set after initialization
            metrics_recorder: None,  // Will be set after initialization
        };
        (state, cleanup_rx)
    }

    /// Set the capture service for survey-specific pcap downloads
    pub fn set_capture_service(&mut self, capture_service: Arc<PacketCaptureService>) {
        self.capture_service = Some(capture_service);
    }

    /// Set the keylog service for DTLS key storage
    pub fn set_keylog_service(&mut self, keylog_service: Arc<DtlsKeylogService>) {
        self.keylog_service = Some(keylog_service);
    }

    /// Set the session manager for survey session lifecycle
    pub fn set_session_manager(&mut self, session_manager: Arc<SessionManager>) {
        self.session_manager = Some(session_manager);
    }

    /// Set the metrics recorder for probe statistics persistence
    pub fn set_metrics_recorder(&mut self, metrics_recorder: Arc<MetricsRecorder>) {
        self.metrics_recorder = Some(metrics_recorder);
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

    pub fn all_ready(&self) -> bool {
        self.probe.is_some()
            && self.bulk.is_some()
            && self.control.is_some()
            && self.testprobe.is_some()
    }
}

impl MeasurementState {
    pub fn new() -> Self {
        Self {
            probe_seq: 0,
            testprobe_seq: 0,
            current_ttl: 1, // Start at TTL 1
            path_ttl: None,
            stop_traceroute: false,      // Initialize to false
            traceroute_started_at: None, // Not started yet
            traffic_active: false,       // Traffic not active until StartServerTraffic
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
            // Probe stream fields
            probe_streams_active: false,
            measurement_probe_seq: 0,
            received_measurement_probes: VecDeque::new(),
            probe_stats: VecDeque::new(),
            client_reported_s2c_stats: None,
            baseline_delay_sum: 0.0,
            baseline_delay_count: 0,
            last_feedback: common::ProbeFeedback::default(),
        }
    }
}
