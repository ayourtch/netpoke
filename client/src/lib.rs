mod measurements;
mod recorder;
mod signaling;
mod webrtc;
use crate::measurements::current_time_ms;

use gloo_timers::callback::Interval;
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Mutex;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{window, Document, RtcPeerConnection};

// Global SENSOR_MANAGER for recorder subsystem
static SENSOR_MANAGER: Lazy<Mutex<Option<recorder::sensors::SensorManager>>> =
    Lazy::new(|| Mutex::new(None));

// Path analysis timeout in milliseconds (30 seconds) - deprecated, now using phased approach
const PATH_ANALYSIS_TIMEOUT_MS: u32 = 30000;

// Delay in milliseconds before starting chart data collection after Phase 1 finishes (10 seconds)
const CHART_COLLECTION_DELAY_MS: u64 = 10000;

// Mode constants for measurement type
const MODE_TRACEROUTE: &str = "traceroute";
const MODE_MEASUREMENT: &str = "measurement";

// Placeholder text for WebRTC-managed addresses (actual addresses are abstracted by the browser)
const WEBRTC_MANAGED_ADDRESS: &str = "WebRTC managed";

// Default WebRTC connection delay in milliseconds
const DEFAULT_WEBRTC_CONNECTION_DELAY_MS: u32 = 50;

const TRACE_POLL_CHECK_MS: u32 = 100;

// New flow constants
// Delay between StartTraceroute messages to different connections (configurable)
const DEFAULT_TRACEROUTE_STAGGER_DELAY_MS: u32 = 1000;
// Minimum wait time between traceroute rounds for the same connection (3000ms)
const TRACEROUTE_ROUND_MIN_WAIT_MS: u32 = 3000;

// Minimum wait time between MTU traceroute rounds
const MTU_TRACEROUTE_ROUND_MIN_WAIT_MS: u32 = 500;
// Number of traceroute rounds
const TRACEROUTE_ROUNDS: u32 = 3;
// Number of MTU traceroute rounds
const MTU_TRACEROUTE_ROUNDS: u32 = 9;
// MTU sizes to test (in bytes)
const MTU_SIZES: [u32; 9] = [576, 1280, 1350, 1400, 1450, 1472, 1490, 1500, 1500];
// Timeout in milliseconds for control channels to be ready
const CONTROL_CHANNEL_READY_TIMEOUT_MS: u32 = 2000;

// Global state for tracking active connections and testing status
thread_local! {
    static WAKE_LOCK: RefCell<Option<JsValue>> = RefCell::new(None);
    static ACTIVE_PEERS: RefCell<Vec<RtcPeerConnection>> = RefCell::new(Vec::new());
    static ACTIVE_INTERVALS: RefCell<Vec<Interval>> = RefCell::new(Vec::new());
    static IS_TESTING_ACTIVE: RefCell<bool> = RefCell::new(false);
    // Flag to signal that testing should be aborted
    static ABORT_TESTING: RefCell<bool> = RefCell::new(false);
    // Timestamp (in ms) when chart data collection should begin (after delay period)
    static CHART_COLLECTION_START_MS: RefCell<Option<u64>> = RefCell::new(None);
    // Current survey session ID (UUID)
    static SURVEY_SESSION_ID: RefCell<String> = RefCell::new(String::new());
    // Issue 005: Track test start time for metadata
    static TEST_START_TIME: RefCell<Option<f64>> = RefCell::new(None);
}

/// Client configuration fetched from the server
#[derive(Debug, Clone, Deserialize)]
struct ClientConfig {
    /// Delay in milliseconds between WebRTC connection establishment attempts
    webrtc_connection_delay_ms: u32,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            webrtc_connection_delay_ms: DEFAULT_WEBRTC_CONNECTION_DELAY_MS,
        }
    }
}

/// Fetch client configuration from the server
async fn fetch_client_config() -> ClientConfig {
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{Request, RequestInit, RequestMode, Response};

    let window = match window() {
        Some(w) => w,
        None => {
            log::warn!("No window available, using default client config");
            return ClientConfig::default();
        }
    };

    let origin = match window.location().origin() {
        Ok(o) => o,
        Err(_) => {
            log::warn!("Failed to get origin, using default client config");
            return ClientConfig::default();
        }
    };

    let url = format!("{}/api/config/client", origin);

    let opts = RequestInit::new();
    opts.set_method("GET");
    opts.set_mode(RequestMode::Cors);

    let request = match Request::new_with_str_and_init(&url, &opts) {
        Ok(r) => r,
        Err(e) => {
            log::warn!(
                "Failed to create request: {:?}, using default client config",
                e
            );
            return ClientConfig::default();
        }
    };

    let resp_value = match JsFuture::from(window.fetch_with_request(&request)).await {
        Ok(r) => r,
        Err(e) => {
            log::warn!("Failed to fetch client config: {:?}, using default", e);
            return ClientConfig::default();
        }
    };

    let resp: Response = match resp_value.dyn_into() {
        Ok(r) => r,
        Err(_) => {
            log::warn!("Invalid response type, using default client config");
            return ClientConfig::default();
        }
    };

    if !resp.ok() {
        log::warn!("Server returned error status, using default client config");
        return ClientConfig::default();
    }

    let json = match resp.json() {
        Ok(j) => j,
        Err(e) => {
            log::warn!(
                "Failed to get JSON from response: {:?}, using default client config",
                e
            );
            return ClientConfig::default();
        }
    };

    let json_value = match JsFuture::from(json).await {
        Ok(v) => v,
        Err(e) => {
            log::warn!("Failed to parse JSON: {:?}, using default client config", e);
            return ClientConfig::default();
        }
    };

    match serde_wasm_bindgen::from_value(json_value) {
        Ok(config) => {
            log::info!("Fetched client config: {:?}", config);
            config
        }
        Err(e) => {
            log::warn!(
                "Failed to deserialize client config: {:?}, using default",
                e
            );
            ClientConfig::default()
        }
    }
}

/// Sleep for the specified number of milliseconds (shared utility function)
pub(crate) async fn sleep_ms(ms: u32) {
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let window = web_sys::window().expect("no global window available");
        window
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms as i32)
            .expect("Failed to set timeout");
    });
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .expect("Failed to sleep");
}

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::default());

    log::info!("WASM client initialized");

    // Set up visibility change handler to stop testing when page loses focus
    setup_visibility_change_handler();
}

/// Set up a visibility change handler to stop testing when the browser window loses focus
fn setup_visibility_change_handler() {
    use wasm_bindgen::closure::Closure;

    let win = match web_sys::window() {
        Some(w) => w,
        None => return,
    };

    let doc = match win.document() {
        Some(d) => d,
        None => return,
    };

    let handler = Closure::wrap(Box::new(move || {
        // Check if page is hidden
        let win_inner = match web_sys::window() {
            Some(w) => w,
            None => return,
        };

        let doc_inner = match win_inner.document() {
            Some(d) => d,
            None => return,
        };

        // Get document.hidden property
        let hidden = js_sys::Reflect::get(&doc_inner, &JsValue::from_str("hidden"))
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if hidden {
            log::info!("Browser window lost focus - stopping testing");

            // Set abort flag to stop ongoing phases
            set_abort_testing(true);

            // Stop all active testing
            if is_testing_active() {
                stop_testing();
            }
        }
    }) as Box<dyn FnMut()>);

    // Add visibilitychange event listener
    let _ =
        doc.add_event_listener_with_callback("visibilitychange", handler.as_ref().unchecked_ref());

    // Prevent the closure from being garbage collected
    handler.forget();

    log::info!("Visibility change handler set up");
}

/// Request a wake lock to prevent the device from sleeping
async fn request_wake_lock() -> Result<(), JsValue> {
    let window = window().ok_or("No window")?;
    let navigator = window.navigator();

    // Check if wake lock is supported
    if js_sys::Reflect::has(&navigator, &JsValue::from_str("wakeLock")).unwrap_or(false) {
        let wake_lock = js_sys::Reflect::get(&navigator, &JsValue::from_str("wakeLock"))?;

        // Request wake lock
        let promise = js_sys::Reflect::apply(
            &js_sys::Reflect::get(&wake_lock, &JsValue::from_str("request"))?.unchecked_into(),
            &wake_lock,
            &js_sys::Array::of1(&JsValue::from_str("screen")),
        )?;

        let result = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise)).await?;

        log::info!("Wake lock acquired");

        // Store the sentinel globally
        WAKE_LOCK.with(|lock| {
            *lock.borrow_mut() = Some(result);
        });

        Ok(())
    } else {
        log::warn!("Wake Lock API not supported in this browser");
        Ok(())
    }
}

/// Release the wake lock
fn release_wake_lock() {
    WAKE_LOCK.with(|lock| {
        if let Some(sentinel) = lock.borrow_mut().take() {
            // Call release() method on the sentinel
            if let Ok(release_fn) = js_sys::Reflect::get(&sentinel, &JsValue::from_str("release")) {
                if let Some(func) = release_fn.dyn_ref::<js_sys::Function>() {
                    let _ = func.call0(&sentinel);
                    log::info!("Wake lock released");
                }
            }
        }
    });
}

/// Set the testing active state
fn set_testing_active(active: bool) {
    IS_TESTING_ACTIVE.with(|state| {
        *state.borrow_mut() = active;
    });
    
    // Issue 005: Track test start time
    if active {
        TEST_START_TIME.with(|time| {
            *time.borrow_mut() = Some(js_sys::Date::now());
        });
    } else {
        TEST_START_TIME.with(|time| {
            *time.borrow_mut() = None;
        });
    }
}

/// Check if testing is currently active
#[wasm_bindgen]
pub fn is_testing_active() -> bool {
    IS_TESTING_ACTIVE.with(|state| *state.borrow())
}

/// Issue 005: Get current test metadata for recordings
#[wasm_bindgen]
pub fn get_test_metadata() -> JsValue {
    let is_testing = IS_TESTING_ACTIVE.with(|state| *state.borrow());
    let start_time = TEST_START_TIME.with(|time| *time.borrow());
    
    if !is_testing || start_time.is_none() {
        return JsValue::NULL;
    }
    
    let start_ms = start_time.unwrap();
    let end_ms = js_sys::Date::now();
    
    // Format timestamps as ISO 8601 UTC strings
    let start_date = js_sys::Date::new(&JsValue::from_f64(start_ms));
    let end_date = js_sys::Date::new(&JsValue::from_f64(end_ms));
    let start_iso = start_date.to_iso_string();
    let end_iso = end_date.to_iso_string();
    
    // Check if we have active IPv4/IPv6 connections by checking peer connections
    let has_peers = ACTIVE_PEERS.with(|peers| peers.borrow().len() > 0);
    
    // Create metadata object
    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &"ipv4Active".into(), &JsValue::from_bool(has_peers)).unwrap();
    js_sys::Reflect::set(&obj, &"ipv6Active".into(), &JsValue::from_bool(has_peers)).unwrap();
    js_sys::Reflect::set(&obj, &"testStartTime".into(), &start_iso).unwrap();
    js_sys::Reflect::set(&obj, &"testEndTime".into(), &end_iso).unwrap();
    js_sys::Reflect::set(&obj, &"testActive".into(), &JsValue::from_bool(is_testing)).unwrap();
    
    obj.into()
}

/// Register a peer connection to be tracked
fn register_peer(peer: RtcPeerConnection) {
    ACTIVE_PEERS.with(|peers| {
        peers.borrow_mut().push(peer);
    });
}

/// Register an interval to be tracked
fn register_interval(interval: Interval) {
    ACTIVE_INTERVALS.with(|intervals| {
        intervals.borrow_mut().push(interval);
    });
}

/// Clear all tracked connections and intervals
fn clear_active_resources() {
    // Close all peer connections
    ACTIVE_PEERS.with(|peers| {
        let mut peers_mut = peers.borrow_mut();
        for peer in peers_mut.drain(..) {
            peer.close();
        }
    });

    // Cancel all intervals
    ACTIVE_INTERVALS.with(|intervals| {
        let mut intervals_mut = intervals.borrow_mut();
        intervals_mut.clear();
    });

    // Reset chart collection start time
    CHART_COLLECTION_START_MS.with(|start| {
        *start.borrow_mut() = None;
    });

    // Reset abort flag
    ABORT_TESTING.with(|abort| {
        *abort.borrow_mut() = false;
    });
}

/// Generate a new UUID for survey session
fn generate_uuid() -> String {
    let random_bytes: [u8; 16] = [
        (js_sys::Math::random() * 256.0) as u8,
        (js_sys::Math::random() * 256.0) as u8,
        (js_sys::Math::random() * 256.0) as u8,
        (js_sys::Math::random() * 256.0) as u8,
        (js_sys::Math::random() * 256.0) as u8,
        (js_sys::Math::random() * 256.0) as u8,
        (js_sys::Math::random() * 256.0) as u8,
        (js_sys::Math::random() * 256.0) as u8,
        (js_sys::Math::random() * 256.0) as u8,
        (js_sys::Math::random() * 256.0) as u8,
        (js_sys::Math::random() * 256.0) as u8,
        (js_sys::Math::random() * 256.0) as u8,
        (js_sys::Math::random() * 256.0) as u8,
        (js_sys::Math::random() * 256.0) as u8,
        (js_sys::Math::random() * 256.0) as u8,
        (js_sys::Math::random() * 256.0) as u8,
    ];
    format!("{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        random_bytes[0], random_bytes[1], random_bytes[2], random_bytes[3],
        random_bytes[4], random_bytes[5],
        random_bytes[6], random_bytes[7],
        random_bytes[8], random_bytes[9],
        random_bytes[10], random_bytes[11], random_bytes[12], random_bytes[13], random_bytes[14], random_bytes[15])
}

/// Set the abort testing flag
fn set_abort_testing(abort: bool) {
    ABORT_TESTING.with(|flag| {
        *flag.borrow_mut() = abort;
    });
}

/// Check if testing should be aborted
fn should_abort_testing() -> bool {
    ABORT_TESTING.with(|flag| *flag.borrow())
}

/// Get current survey session ID
fn get_survey_session_id() -> String {
    SURVEY_SESSION_ID.with(|id| id.borrow().clone())
}

/// Set current survey session ID
fn set_survey_session_id(id: String) {
    SURVEY_SESSION_ID.with(|session_id| {
        *session_id.borrow_mut() = id;
    });
}

#[wasm_bindgen]
pub async fn start_measurement() -> Result<(), JsValue> {
    // Default to 1 connection per address family
    start_measurement_with_count(1).await
}

/// Start measurement with multiple connections per address family for ECMP testing
#[wasm_bindgen]
pub async fn start_measurement_with_count(conn_count: u8) -> Result<(), JsValue> {
    let count = conn_count.clamp(1, 16) as usize;
    log::info!(
        "Starting dual-stack network measurement with {} connections per address family...",
        count
    );

    // Fetch client configuration from server
    let client_config = fetch_client_config().await;
    let connection_delay_ms = client_config.webrtc_connection_delay_ms;
    log::info!("Using WebRTC connection delay: {}ms", connection_delay_ms);

    // Request wake lock to prevent device from sleeping
    if let Err(e) = request_wake_lock().await {
        log::warn!("Failed to acquire wake lock: {:?}", e);
        // Continue anyway - wake lock is optional
    }

    // Create IPv4 connections
    let mut ipv4_connections = Vec::with_capacity(count);
    let mut parent_id: Option<String> = None;

    for i in 0..count {
        // Add delay between connection attempts (except for the first one)
        if i > 0 && connection_delay_ms > 0 {
            sleep_ms(connection_delay_ms).await;
        }

        let conn = webrtc::WebRtcConnection::new_with_ip_version_and_mode(
            "ipv4",
            parent_id.clone(),
            None,
            None, // conn_id will be auto-generated
        )
        .await?;

        if i == 0 {
            parent_id = Some(conn.client_id.clone());
        }
        log::info!(
            "IPv4 connection {} created with client_id: {}, conn_id: {}",
            i,
            conn.client_id,
            conn.conn_id
        );

        // Register the connection with JavaScript for display
        register_peer_connection_js(
            "ipv4",
            i,
            &conn.conn_id,
            WEBRTC_MANAGED_ADDRESS,
            WEBRTC_MANAGED_ADDRESS,
        );

        // Set up callback to update addresses when connection is established
        conn.setup_address_update_callback("ipv4", i);

        ipv4_connections.push(conn);
    }

    // Create IPv6 connections (use parent from first IPv4 connection)
    let mut ipv6_connections = Vec::with_capacity(count);
    for i in 0..count {
        // Add delay between connection attempts (including between IPv4 and IPv6 groups)
        if connection_delay_ms > 0 {
            sleep_ms(connection_delay_ms).await;
        }

        let conn = webrtc::WebRtcConnection::new_with_ip_version_and_mode(
            "ipv6",
            parent_id.clone(),
            None,
            None, // conn_id will be auto-generated
        )
        .await?;
        log::info!(
            "IPv6 connection {} created with client_id: {}, conn_id: {}",
            i,
            conn.client_id,
            conn.conn_id
        );

        // Register the connection with JavaScript for display
        register_peer_connection_js(
            "ipv6",
            i,
            &conn.conn_id,
            WEBRTC_MANAGED_ADDRESS,
            WEBRTC_MANAGED_ADDRESS,
        );

        // Set up callback to update addresses when connection is established
        conn.setup_address_update_callback("ipv6", i);

        ipv6_connections.push(conn);
    }

    // Collect states for calculation and UI updates
    let mut calc_states: Vec<std::rc::Rc<std::cell::RefCell<measurements::MeasurementState>>> =
        Vec::new();
    for conn in &ipv4_connections {
        calc_states.push(conn.state.clone());
    }
    for conn in &ipv6_connections {
        calc_states.push(conn.state.clone());
    }

    // Start latency-sensitive metric calculation loop
    let calc_states_for_interval = calc_states.clone();
    let calc_interval = gloo_timers::callback::Interval::new(100, move || {
        for state in &calc_states_for_interval {
            state.borrow_mut().calculate_metrics();
        }
    });
    register_interval(calc_interval);

    // Collect all connection states for UI updates
    let ipv4_states: Vec<_> = ipv4_connections.iter().map(|c| c.state.clone()).collect();
    let ipv6_states: Vec<_> = ipv6_connections.iter().map(|c| c.state.clone()).collect();
    let conn_count = count;

    // Start UI update loop that updates all connections
    let ui_interval = gloo_timers::callback::Interval::new(500, move || {
        // Update metrics for each IPv4 connection
        for (i, state) in ipv4_states.iter().enumerate() {
            let state_ref = state.borrow();
            if conn_count > 1 {
                // Multi-connection mode: update connection-specific tables
                update_ui_connection("ipv4", i, &state_ref.metrics);
            } else if i == 0 {
                // Single connection mode: update default tables
                // (first iteration only, handled below)
            }
        }

        // Update metrics for each IPv6 connection
        for (i, state) in ipv6_states.iter().enumerate() {
            let state_ref = state.borrow();
            if conn_count > 1 {
                // Multi-connection mode: update connection-specific tables
                update_ui_connection("ipv6", i, &state_ref.metrics);
            }
        }

        // For single connection or chart updates, use first connection of each type
        if !ipv4_states.is_empty() && !ipv6_states.is_empty() {
            let ipv4_metrics = ipv4_states[0].borrow();
            let ipv6_metrics = ipv6_states[0].borrow();
            update_ui_dual(&ipv4_metrics.metrics, &ipv6_metrics.metrics);
        }
    });
    register_interval(ui_interval);

    // Register all peer connections and mark testing as active
    for conn in ipv4_connections {
        register_peer(conn.peer.clone());
        std::mem::forget(conn);
    }
    for conn in ipv6_connections {
        register_peer(conn.peer.clone());
        std::mem::forget(conn);
    }

    set_testing_active(true);

    Ok(())
}

/// Stop the measurement and release the wake lock (deprecated - use stop_testing)
#[wasm_bindgen]
pub fn stop_measurement() {
    log::info!("Stopping measurement...");
    release_wake_lock();
}

/// Stop all active testing, close connections, and clean up resources
#[wasm_bindgen]
pub fn stop_testing() {
    log::info!("Stopping all active testing...");

    // Set abort flag to stop any ongoing phases
    set_abort_testing(true);

    // Close all connections and clear intervals
    clear_active_resources();

    // Release wake lock
    release_wake_lock();

    // Mark testing as inactive
    set_testing_active(false);

    log::info!("All testing stopped and resources cleaned up");
}

/// Analyze the network: first perform traceroute, then start measurements
#[wasm_bindgen]
pub async fn analyze_network() -> Result<(), JsValue> {
    // Default to 1 connection per address family
    analyze_network_with_count(1).await
}

/// Analyze the network with multiple connections: new phased approach
/// Phase 0: Generate survey session ID, establish all connections, wait for ServerSideReady
/// Phase 1: Traceroute (5 rounds, client-driven)
/// Phase 2: MTU Traceroute (5 rounds with different packet sizes)
/// Phase 3: Get measuring time, start measurements
#[wasm_bindgen]
pub async fn analyze_network_with_count(conn_count: u8) -> Result<(), JsValue> {
    let count = conn_count.clamp(1, 16) as usize;
    log::info!(
        "Starting network analysis with {} connections per address family...",
        count
    );

    // Reset abort flag at start
    set_abort_testing(false);

    // Generate survey session UUID
    let survey_session_id = generate_uuid();
    set_survey_session_id(survey_session_id.clone());
    log::info!("Generated survey session ID: {}", survey_session_id);

    // Notify JavaScript of the survey session ID for PCAP downloads
    notify_survey_session_id_js(&survey_session_id);

    // Fetch client configuration from server
    let client_config = fetch_client_config().await;
    let connection_delay_ms = client_config.webrtc_connection_delay_ms;
    log::info!("Using WebRTC connection delay: {}ms", connection_delay_ms);

    // Request wake lock to prevent device from sleeping
    if let Err(e) = request_wake_lock().await {
        log::warn!("Failed to acquire wake lock: {:?}", e);
    }

    // PHASE 0: Establish all connections
    log::info!("PHASE 0: Establishing WebRTC connections...");
    set_doc_status("PHASE 0: Establishing WebRTC connections...");

    // Create IPv4 connections
    let mut ipv4_connections: Vec<webrtc::WebRtcConnection> = Vec::with_capacity(count);
    let mut parent_id: Option<String> = None;

    for i in 0..count {
        if should_abort_testing() {
            log::info!("Testing aborted during connection setup");
            return Ok(());
        }

        if i > 0 && connection_delay_ms > 0 {
            sleep_ms(connection_delay_ms).await;
        }

        let conn = webrtc::WebRtcConnection::new_with_ip_version_and_mode(
            "ipv4",
            parent_id.clone(),
            None, // No mode - server doesn't auto-start anything
            None,
        )
        .await?;

        if i == 0 {
            parent_id = Some(conn.client_id.clone());
        }

        // Enable traceroute mode to prevent measurement data collection
        conn.set_traceroute_mode(true);

        log::info!(
            "IPv4 connection {} created with client_id: {}, conn_id: {}",
            i,
            conn.client_id,
            conn.conn_id
        );

        register_peer_connection_js(
            "ipv4",
            i,
            &conn.conn_id,
            WEBRTC_MANAGED_ADDRESS,
            WEBRTC_MANAGED_ADDRESS,
        );
        conn.setup_address_update_callback("ipv4", i);

        // NOTE: Do NOT send StartSurveySession here - control channel is not ready yet

        ipv4_connections.push(conn);
    }

    // Create IPv6 connections
    let mut ipv6_connections: Vec<webrtc::WebRtcConnection> = Vec::with_capacity(count);
    for i in 0..count {
        if should_abort_testing() {
            log::info!("Testing aborted during connection setup");
            return Ok(());
        }

        if connection_delay_ms > 0 {
            sleep_ms(connection_delay_ms).await;
        }

        let conn = webrtc::WebRtcConnection::new_with_ip_version_and_mode(
            "ipv6",
            parent_id.clone(),
            None,
            None,
        )
        .await?;

        conn.set_traceroute_mode(true);

        log::info!(
            "IPv6 connection {} created with client_id: {}, conn_id: {}",
            i,
            conn.client_id,
            conn.conn_id
        );

        register_peer_connection_js(
            "ipv6",
            i,
            &conn.conn_id,
            WEBRTC_MANAGED_ADDRESS,
            WEBRTC_MANAGED_ADDRESS,
        );
        conn.setup_address_update_callback("ipv6", i);

        // NOTE: Do NOT send StartSurveySession here - control channel is not ready yet

        ipv6_connections.push(conn);
    }

    // Wait for all control channels to be ready before sending StartSurveySession
    log::info!("Waiting for control channels to be ready...");
    set_doc_status("PHASE 0.1: Waiting for control channels to be ready...");

    for (i, conn) in ipv4_connections.iter_mut().enumerate() {
        if should_abort_testing() {
            log::info!("Testing aborted while waiting for control channels");
            return Ok(());
        }

        if conn
            .wait_for_control_channel_ready(CONTROL_CHANNEL_READY_TIMEOUT_MS)
            .await
        {
            if let Err(e) = conn.send_start_survey_session(&survey_session_id).await {
                log::warn!(
                    "Failed to send StartSurveySession for IPv4 connection {}: {:?}",
                    i,
                    e
                );
            } else {
                log::info!("Sent StartSurveySession for IPv4 connection {}", i);
            }
        } else {
            log::warn!(
                "Control channel not ready for IPv4 connection {}, skipping StartSurveySession",
                i
            );
        }
    }

    for (i, conn) in ipv6_connections.iter_mut().enumerate() {
        if should_abort_testing() {
            log::info!("Testing aborted while waiting for control channels");
            return Ok(());
        }

        if conn
            .wait_for_control_channel_ready(CONTROL_CHANNEL_READY_TIMEOUT_MS)
            .await
        {
            if let Err(e) = conn.send_start_survey_session(&survey_session_id).await {
                log::warn!(
                    "Failed to send StartSurveySession for IPv6 connection {}: {:?}",
                    i,
                    e
                );
            } else {
                log::info!("Sent StartSurveySession for IPv6 connection {}", i);
            }
        } else {
            log::warn!(
                "Control channel not ready for IPv6 connection {}, skipping StartSurveySession",
                i
            );
        }
    }

    // Wait for all ServerSideReady messages
    log::info!("Waiting for ServerSideReady from all connections...");
    set_doc_status("PHASE 0.2: Waiting for server ready on all connections...");

    let timeout_ms: u32 = 30000; // 30 second timeout for all connections to be ready
    let start_time = current_time_ms();

    loop {
        let all_connections: Vec<_> = ipv4_connections
            .iter()
            .chain(ipv6_connections.iter())
            .filter(|x| !x.failed)
            .collect();
        let total_connections = all_connections.len();

        if should_abort_testing() {
            log::info!("Testing aborted while waiting for ServerSideReady");
            return Ok(());
        }

        let ready_count = all_connections
            .iter()
            .filter(|c| c.state.borrow().server_side_ready)
            .count();

        if ready_count == total_connections {
            log::info!("All {} connections are ready", total_connections);
            break;
        } else {
            log::info!(
                "Ready {} out of {} connections...",
                ready_count,
                total_connections
            );
        }

        if (current_time_ms() - start_time) > timeout_ms as u64 {
            log::warn!(
                "Timeout waiting for ServerSideReady, proceeding with {}/{} ready",
                ready_count,
                total_connections
            );
            break;
        }

        sleep_ms(100).await;
    }

    // PHASE 1: Traceroute (5 rounds)
    log::info!(
        "PHASE 1: Starting traceroute ({} rounds)...",
        TRACEROUTE_ROUNDS
    );
    set_doc_status("PHASE 1: Starting traceroute with connection(s) 5-tuple(s)...");

    for round in 0..TRACEROUTE_ROUNDS {
        if should_abort_testing() {
            log::info!("Testing aborted during traceroute phase");
            return Ok(());
        }
        let mut total_probe_conns = 0;
        for conn in ipv4_connections.iter().chain(ipv6_connections.iter()) {
            if !conn.failed {
                total_probe_conns += 1;
            }
        }

        log::info!("Traceroute round {}/{}", round + 1, TRACEROUTE_ROUNDS);

        // Send StartTraceroute to all connections with stagger delay
        for conn in ipv4_connections.iter().chain(ipv6_connections.iter()) {
            if should_abort_testing() {
                return Ok(());
            }
            if conn.failed {
                log::warn!("conn is failed, ignore");
                continue;
            }

            if let Err(e) = conn.send_start_traceroute(&survey_session_id).await {
                log::warn!("Failed to send StartTraceroute: {:?}", e);
            }
            let mut count = DEFAULT_TRACEROUTE_STAGGER_DELAY_MS / TRACE_POLL_CHECK_MS;
            loop {
                sleep_ms(TRACE_POLL_CHECK_MS).await;
                {
                    let st = conn.state.borrow();
                    let n_active = st.traceroute_started - st.traceroute_done;
                    if count == 0 || n_active < total_probe_conns.min(3) {
                        break;
                    }
                    log::warn!("Active traceroutes: {}, countdown: {}", n_active, count);
                }
                count -= 1;
            }
        }

        // Wait at least TRACEROUTE_ROUND_MIN_WAIT_MS before next round
        sleep_ms(TRACEROUTE_ROUND_MIN_WAIT_MS).await;
    }

    log::info!("PHASE 1 complete: Traceroute finished");

    // Add a brief pause between phases to allow server processing to complete
    //sleep_ms(1000).await;

    // PHASE 2: MTU Traceroute (5 rounds with different packet sizes)
    log::info!(
        "PHASE 2: Starting MTU traceroute ({} rounds with sizes {:?})...",
        MTU_TRACEROUTE_ROUNDS,
        MTU_SIZES
    );

    for (round, &packet_size) in MTU_SIZES.iter().enumerate() {
        let status = format!(
            "PHASE 2.{}: Doing MTU traceroute with size {}",
            round, &packet_size
        );
        set_doc_status(&status);
        if should_abort_testing() {
            log::info!("Testing aborted during MTU traceroute phase");
            return Ok(());
        }

        let mut total_probe_conns = 0;
        for conn in ipv4_connections.iter().chain(ipv6_connections.iter()) {
            if !conn.failed {
                total_probe_conns += 1;
            }
        }

        log::info!(
            "MTU traceroute round {}/{} with packet_size={}",
            round + 1,
            MTU_TRACEROUTE_ROUNDS,
            packet_size
        );

        let wait_timeout_ms = if round == MTU_SIZES.len() { 3000 } else { 0 };

        for conn in ipv4_connections.iter().chain(ipv6_connections.iter()) {
            if should_abort_testing() {
                return Ok(());
            }
            if conn.failed {
                log::warn!("conn is failed, ignore");
                continue;
            }
            let path_ttl = if let Some(path_ttl) = conn.state.borrow().path_ttl {
                path_ttl
            } else {
                16
            };

            if let Err(e) = conn
                .send_start_mtu_traceroute(
                    &survey_session_id,
                    packet_size,
                    path_ttl,
                    wait_timeout_ms,
                )
                .await
            {
                log::warn!("Failed to send StartMtuTraceroute: {:?}", e);
            }

            let mut count = DEFAULT_TRACEROUTE_STAGGER_DELAY_MS / TRACE_POLL_CHECK_MS;
            loop {
                sleep_ms(TRACE_POLL_CHECK_MS).await;
                {
                    let st = conn.state.borrow();
                    let n_active = st.mtu_traceroute_started - st.mtu_traceroute_done;
                    if count == 0 || n_active < total_probe_conns.min(4) {
                        break;
                    }
                    log::warn!("Active mtu_traceroutes: {}, countdown: {}", n_active, count);
                }
                count -= 1;
            }
        }

        let mut count = DEFAULT_TRACEROUTE_STAGGER_DELAY_MS / TRACE_POLL_CHECK_MS;
        loop {
            sleep_ms(TRACE_POLL_CHECK_MS).await;
            {
                let mut total_active = 0;
                for conn in ipv4_connections.iter().chain(ipv6_connections.iter()) {
                    if !conn.failed {
                        let st = conn.state.borrow();
                        let n_active = st.mtu_traceroute_started - st.mtu_traceroute_done;
                        total_active += n_active;
                    }
                }
                if count == 0 || total_active == 0 {
                    break;
                }
                log::warn!(
                    "Still Active mtu_traceroutes: {}, countdown: {}",
                    total_active,
                    count
                );
            }
            count -= 1;
        }

        sleep_ms(MTU_TRACEROUTE_ROUND_MIN_WAIT_MS).await;
    }

    log::info!("PHASE 2 complete: MTU traceroute finished");

    // Add a brief pause between phases to allow server processing to complete
    // sleep_ms(1000).await;

    // PHASE 3: Get measuring time and start probe streams for baseline measurement
    log::info!("PHASE 3: Starting probe streams for baseline measurement...");
    set_doc_status("PHASE 3: Starting probe streams for baseline measurement...");

    // Send GetMeasuringTime to first connection
    if let Some(first_conn) = ipv4_connections.first() {
        if let Err(e) = first_conn.send_get_measuring_time(&survey_session_id).await {
            log::warn!("Failed to send GetMeasuringTime: {:?}", e);
        }

        // Wait for response
        sleep_ms(1000).await;

        let measuring_time = first_conn.state.borrow().measuring_time_ms;
        log::info!("Received measuring time: {:?}ms", measuring_time);
    }

    if should_abort_testing() {
        log::info!("Testing aborted before measurement phase");
        return Ok(());
    }

    // Clear metrics before starting measurements
    log::info!("Clearing metrics before probe stream phase...");
    for conn in &ipv4_connections {
        conn.state.borrow_mut().clear_metrics();
    }
    for conn in &ipv6_connections {
        conn.state.borrow_mut().clear_metrics();
    }

    // Set the chart collection start time
    let chart_start_time = current_time_ms() + CHART_COLLECTION_DELAY_MS;
    CHART_COLLECTION_START_MS.with(|start| {
        *start.borrow_mut() = Some(chart_start_time);
    });
    log::info!(
        "Chart data collection will begin in {} seconds",
        CHART_COLLECTION_DELAY_MS / 1000
    );

    // Send StartProbeStreams to all connections
    for conn in ipv4_connections.iter().chain(ipv6_connections.iter()) {
        if should_abort_testing() {
            return Ok(());
        }

        if let Err(e) = conn.send_start_probe_streams(&survey_session_id).await {
            log::warn!("Failed to send StartProbeStreams: {:?}", e);
        }
    }

    log::info!("StartProbeStreams sent to all connections");

    // Start client-side probe sender for each connection
    for conn in ipv4_connections.iter().chain(ipv6_connections.iter()) {
        let state = conn.state.clone();
        let probe_channel = conn.get_probe_channel();
        let conn_id = conn.conn_id.clone();

        if let Some(channel) = probe_channel {
            // Start sending measurement probes at configured rate
            let interval =
                gloo_timers::callback::Interval::new(common::PROBE_INTERVAL_MS, move || {
                    let mut state = state.borrow_mut();
                    if !state.probe_streams_active {
                        return;
                    }

                    let seq = state.measurement_probe_seq;
                    state.measurement_probe_seq += 1;
                    let feedback = state.last_feedback.clone();

                    let probe = common::MeasurementProbePacket {
                        seq,
                        sent_at_ms: current_time_ms(),
                        direction: common::Direction::ClientToServer,
                        conn_id: conn_id.clone(),
                        feedback,
                    };

                    if let Ok(json) = serde_json::to_string(&probe) {
                        if let Err(e) = channel.send_with_str(&json) {
                            log::error!("Failed to send measurement probe: {:?}", e);
                        }
                    }
                });
            register_interval(interval);
        }
    }

    // Start per-second stats reporter on control channel
    for conn in ipv4_connections.iter().chain(ipv6_connections.iter()) {
        let state = conn.state.clone();
        let control_channel = conn.control_channel.clone();
        let conn_id = conn.conn_id.clone();
        let survey_id = survey_session_id.clone();

        let interval = gloo_timers::callback::Interval::new(1000, move || {
            let state_ref = state.borrow();
            if !state_ref.probe_streams_active {
                return;
            }

            // Calculate S2C stats from received measurement probes
            let s2c_stats = calculate_client_s2c_stats(&state_ref);

            // Get server-reported C2S stats
            let c2s_stats = state_ref
                .server_reported_c2s_stats
                .clone()
                .unwrap_or_default();

            drop(state_ref);

            let report = common::ControlMessage::ProbeStats(common::ProbeStatsReport {
                conn_id: conn_id.clone(),
                survey_session_id: survey_id.clone(),
                timestamp_ms: current_time_ms(),
                c2s_stats,
                s2c_stats: s2c_stats.clone(),
            });

            // Store calculated S2C stats
            state.borrow_mut().calculated_s2c_stats = Some(s2c_stats);

            // Send stats on control channel
            if let Some(channel) = control_channel.borrow().as_ref() {
                if let Ok(json) = serde_json::to_string(&report) {
                    if let Err(e) = channel.send_with_str(&json) {
                        log::error!("Failed to send probe stats: {:?}", e);
                    }
                }
            }
        });
        register_interval(interval);
    }

    // Collect states for calculation and UI updates
    let mut calc_states: Vec<Rc<RefCell<measurements::MeasurementState>>> = Vec::new();
    for conn in &ipv4_connections {
        calc_states.push(conn.state.clone());
    }
    for conn in &ipv6_connections {
        calc_states.push(conn.state.clone());
    }

    // Start latency-sensitive metric calculation loop
    let calc_states_for_interval = calc_states.clone();
    let calc_interval = gloo_timers::callback::Interval::new(100, move || {
        for state in &calc_states_for_interval {
            state.borrow_mut().calculate_metrics();
        }
    });
    register_interval(calc_interval);

    // Collect all connection states for UI updates
    let ipv4_states: Vec<_> = ipv4_connections.iter().map(|c| c.state.clone()).collect();
    let ipv6_states: Vec<_> = ipv6_connections.iter().map(|c| c.state.clone()).collect();
    let conn_count_ui = count;

    // Start UI update loop
    let ui_interval = gloo_timers::callback::Interval::new(500, move || {
        for (i, state) in ipv4_states.iter().enumerate() {
            let state_ref = state.borrow();
            if conn_count_ui > 1 {
                update_ui_connection("ipv4", i, &state_ref.metrics);
            }
        }

        for (i, state) in ipv6_states.iter().enumerate() {
            let state_ref = state.borrow();
            if conn_count_ui > 1 {
                update_ui_connection("ipv6", i, &state_ref.metrics);
            }
        }

        if !ipv4_states.is_empty() && !ipv6_states.is_empty() {
            let ipv4_metrics = ipv4_states[0].borrow();
            let ipv6_metrics = ipv6_states[0].borrow();
            update_ui_dual(&ipv4_metrics.metrics, &ipv6_metrics.metrics);
        }
    });
    register_interval(ui_interval);

    // Register all peer connections and mark testing as active
    for conn in ipv4_connections {
        register_peer(conn.peer.clone());
        std::mem::forget(conn);
    }
    for conn in ipv6_connections {
        register_peer(conn.peer.clone());
        std::mem::forget(conn);
    }

    set_testing_active(true);

    log::info!("Network analysis complete, measurements running...");

    Ok(())
}

/// Calculate S2C stats from received measurement probes (called by client)
fn calculate_client_s2c_stats(state: &measurements::MeasurementState) -> common::DirectionStats {
    let now_ms = current_time_ms();
    let stats_cutoff = now_ms.saturating_sub(common::PROBE_FEEDBACK_WINDOW_MS);

    // Filter to probes received in the stats window
    let recent_probes: Vec<_> = state
        .received_measurement_probes
        .iter()
        .filter(|p| p.received_at_ms >= stats_cutoff)
        .collect();

    if recent_probes.is_empty() {
        return common::DirectionStats::default();
    }

    let baseline = if state.baseline_delay_count > 0 {
        state.baseline_delay_sum / state.baseline_delay_count as f64
    } else {
        0.0
    };

    // Calculate delay deviations from baseline
    // Use signed arithmetic to handle clock skew between client and server
    let mut delay_deviations: Vec<f64> = recent_probes
        .iter()
        .map(|p| {
            let delay = (p.received_at_ms as i64 - p.sent_at_ms as i64) as f64;
            delay - baseline
        })
        .collect();

    delay_deviations.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // Calculate percentiles
    let len = delay_deviations.len();
    let p50_idx = len / 2;
    let p99_idx = (len * 99) / 100;

    let delay_deviation_ms = [
        delay_deviations[p50_idx],                 // 50th percentile
        delay_deviations[p99_idx.min(len - 1)],    // 99th percentile
        *delay_deviations.first().unwrap_or(&0.0), // min
        *delay_deviations.last().unwrap_or(&0.0),  // max
    ];

    // Calculate jitter (consecutive delay differences)
    // Use signed arithmetic to handle clock skew
    let mut jitters: Vec<f64> = Vec::new();
    let mut prev_delay: Option<f64> = None;
    for p in &recent_probes {
        let delay = (p.received_at_ms as i64 - p.sent_at_ms as i64) as f64;
        if let Some(prev) = prev_delay {
            jitters.push((delay - prev).abs());
        }
        prev_delay = Some(delay);
    }

    jitters.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let jitter_len = jitters.len().max(1);
    let jitter_ms = if jitters.is_empty() {
        [0.0; 4]
    } else {
        [
            jitters[jitter_len / 2],
            jitters[(jitter_len * 99) / 100],
            *jitters.first().unwrap_or(&0.0),
            *jitters.last().unwrap_or(&0.0),
        ]
    };

    // Calculate loss rate
    let min_seq = recent_probes.iter().map(|p| p.seq).min().unwrap_or(0);
    let max_seq = recent_probes.iter().map(|p| p.seq).max().unwrap_or(0);
    let expected = (max_seq.saturating_sub(min_seq) + 1) as f64;
    let received = recent_probes.len() as f64;
    let loss_rate = if expected > 0.0 {
        ((expected - received) / expected * 100.0).max(0.0)
    } else {
        0.0
    };

    // Calculate reorder rate
    let mut reorders = 0;
    let mut max_seq_seen = 0u64;
    for p in &recent_probes {
        if p.seq < max_seq_seen {
            reorders += 1;
        }
        max_seq_seen = max_seq_seen.max(p.seq);
    }
    let reorder_rate = if !recent_probes.is_empty() {
        (reorders as f64 / recent_probes.len() as f64) * 100.0
    } else {
        0.0
    };

    common::DirectionStats {
        delay_deviation_ms,
        rtt_ms: [0.0; 4], // RTT requires echo, not calculated here
        jitter_ms,
        loss_rate,
        reorder_rate,
        probe_count: recent_probes.len() as u32,
        baseline_delay_ms: baseline,
    }
}

fn update_ui_dual(ipv4_metrics: &common::ClientMetrics, ipv6_metrics: &common::ClientMetrics) {
    let window = match window() {
        Some(w) => w,
        None => return,
    };

    let document = match window.document() {
        Some(d) => d,
        None => return,
    };

    let format_bytes = |bytes: f64| -> String {
        if bytes >= 1024.0 * 1024.0 {
            format!("{:.2} MB/s", bytes / (1024.0 * 1024.0))
        } else if bytes >= 1024.0 {
            format!("{:.2} KB/s", bytes / 1024.0)
        } else {
            format!("{:.0} B/s", bytes)
        }
    };

    let format_ms = |ms: f64| -> String {
        if ms > 0.0 {
            format!("{:.1}", ms)
        } else {
            "-".to_string()
        }
    };

    let format_pct = |pct: f64| -> String {
        if pct > 0.0 {
            format!("{:.1}%", pct)
        } else {
            "0%".to_string()
        }
    };

    let dtm = {
        use wasm_timer::SystemTime;
        let now = SystemTime::now();
        // format!("{} = {:?}: {:?}", current_time_ms(), &now, &ipv4_metrics);
        let dbg_message = format!("..");
        format!("{} = {}", current_time_ms(), dbg_message)
    };
    set_element_text(&document, "ayxx", &dtm);

    // Update IPv4 metrics
    set_element_text(
        &document,
        "ipv4-s2c-tp-1",
        &format_bytes(ipv4_metrics.s2c_throughput[0]),
    );
    set_element_text(
        &document,
        "ipv4-s2c-tp-10",
        &format_bytes(ipv4_metrics.s2c_throughput[1]),
    );
    set_element_text(
        &document,
        "ipv4-s2c-tp-60",
        &format_bytes(ipv4_metrics.s2c_throughput[2]),
    );
    set_element_text(
        &document,
        "ipv4-s2c-delay-1",
        &format_ms(ipv4_metrics.s2c_delay_avg[0]),
    );
    set_element_text(
        &document,
        "ipv4-s2c-delay-10",
        &format_ms(ipv4_metrics.s2c_delay_avg[1]),
    );
    set_element_text(
        &document,
        "ipv4-s2c-delay-60",
        &format_ms(ipv4_metrics.s2c_delay_avg[2]),
    );
    set_element_text(
        &document,
        "ipv4-s2c-jitter-1",
        &format_ms(ipv4_metrics.s2c_jitter[0]),
    );
    set_element_text(
        &document,
        "ipv4-s2c-jitter-10",
        &format_ms(ipv4_metrics.s2c_jitter[1]),
    );
    set_element_text(
        &document,
        "ipv4-s2c-jitter-60",
        &format_ms(ipv4_metrics.s2c_jitter[2]),
    );
    set_element_text(
        &document,
        "ipv4-s2c-loss-1",
        &format_pct(ipv4_metrics.s2c_loss_rate[0]),
    );
    set_element_text(
        &document,
        "ipv4-s2c-loss-10",
        &format_pct(ipv4_metrics.s2c_loss_rate[1]),
    );
    set_element_text(
        &document,
        "ipv4-s2c-loss-60",
        &format_pct(ipv4_metrics.s2c_loss_rate[2]),
    );
    set_element_text(
        &document,
        "ipv4-s2c-reorder-1",
        &format_pct(ipv4_metrics.s2c_reorder_rate[0]),
    );
    set_element_text(
        &document,
        "ipv4-s2c-reorder-10",
        &format_pct(ipv4_metrics.s2c_reorder_rate[1]),
    );
    set_element_text(
        &document,
        "ipv4-s2c-reorder-60",
        &format_pct(ipv4_metrics.s2c_reorder_rate[2]),
    );

    // Update IPv6 metrics
    set_element_text(
        &document,
        "ipv6-s2c-tp-1",
        &format_bytes(ipv6_metrics.s2c_throughput[0]),
    );
    set_element_text(
        &document,
        "ipv6-s2c-tp-10",
        &format_bytes(ipv6_metrics.s2c_throughput[1]),
    );
    set_element_text(
        &document,
        "ipv6-s2c-tp-60",
        &format_bytes(ipv6_metrics.s2c_throughput[2]),
    );
    set_element_text(
        &document,
        "ipv6-s2c-delay-1",
        &format_ms(ipv6_metrics.s2c_delay_avg[0]),
    );
    set_element_text(
        &document,
        "ipv6-s2c-delay-10",
        &format_ms(ipv6_metrics.s2c_delay_avg[1]),
    );
    set_element_text(
        &document,
        "ipv6-s2c-delay-60",
        &format_ms(ipv6_metrics.s2c_delay_avg[2]),
    );
    set_element_text(
        &document,
        "ipv6-s2c-jitter-1",
        &format_ms(ipv6_metrics.s2c_jitter[0]),
    );
    set_element_text(
        &document,
        "ipv6-s2c-jitter-10",
        &format_ms(ipv6_metrics.s2c_jitter[1]),
    );
    set_element_text(
        &document,
        "ipv6-s2c-jitter-60",
        &format_ms(ipv6_metrics.s2c_jitter[2]),
    );
    set_element_text(
        &document,
        "ipv6-s2c-loss-1",
        &format_pct(ipv6_metrics.s2c_loss_rate[0]),
    );
    set_element_text(
        &document,
        "ipv6-s2c-loss-10",
        &format_pct(ipv6_metrics.s2c_loss_rate[1]),
    );
    set_element_text(
        &document,
        "ipv6-s2c-loss-60",
        &format_pct(ipv6_metrics.s2c_loss_rate[2]),
    );
    set_element_text(
        &document,
        "ipv6-s2c-reorder-1",
        &format_pct(ipv6_metrics.s2c_reorder_rate[0]),
    );
    set_element_text(
        &document,
        "ipv6-s2c-reorder-10",
        &format_pct(ipv6_metrics.s2c_reorder_rate[1]),
    );
    set_element_text(
        &document,
        "ipv6-s2c-reorder-60",
        &format_pct(ipv6_metrics.s2c_reorder_rate[2]),
    );

    // Update chart with metrics data
    call_add_metrics_data(ipv4_metrics, ipv6_metrics);
}

/// Notify JavaScript of the current survey session ID for survey-specific PCAP downloads.
///
/// # Parameters
/// - `survey_session_id`: The unique survey session ID (UUID) for this test run
fn notify_survey_session_id_js(survey_session_id: &str) {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::JsValue;

    let window = match window() {
        Some(w) => w,
        None => return,
    };

    // Call JavaScript function setSurveySessionId(sessionId)
    if let Ok(set_fn) = js_sys::Reflect::get(&window, &JsValue::from_str("setSurveySessionId")) {
        if let Some(func) = set_fn.dyn_ref::<js_sys::Function>() {
            if let Err(e) = func.call1(&JsValue::NULL, &JsValue::from_str(survey_session_id)) {
                log::warn!("Failed to call setSurveySessionId: {:?}", e);
            } else {
                log::info!(
                    "Notified JavaScript of survey session ID: {}",
                    survey_session_id
                );
            }
        }
    }
}

/// Register a peer connection with JavaScript for display in the peer connections list.
///
/// # Parameters
/// - `ip_version`: The IP version of the connection ("ipv4" or "ipv6")
/// - `conn_index`: The zero-based index of this connection within its IP version group
/// - `conn_id`: The unique connection ID (UUID) that matches the conn_id in traceroute data
/// - `local_address`: The local address string (IP:port or placeholder)
/// - `remote_address`: The remote address string (IP:port or placeholder)
fn register_peer_connection_js(
    ip_version: &str,
    conn_index: usize,
    conn_id: &str,
    local_address: &str,
    remote_address: &str,
) {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::JsValue;

    let window = match window() {
        Some(w) => w,
        None => return,
    };

    // Call JavaScript function registerPeerConnection(ipVersion, connIndex, connId, localAddress, remoteAddress)
    if let Ok(register_fn) =
        js_sys::Reflect::get(&window, &JsValue::from_str("registerPeerConnection"))
    {
        if let Some(func) = register_fn.dyn_ref::<js_sys::Function>() {
            let args = js_sys::Array::new();
            args.push(&JsValue::from_str(ip_version));
            args.push(&JsValue::from_f64(conn_index as f64));
            args.push(&JsValue::from_str(conn_id));
            args.push(&JsValue::from_str(local_address));
            args.push(&JsValue::from_str(remote_address));

            if let Err(e) = func.apply(&JsValue::NULL, &args) {
                log::warn!("Failed to call registerPeerConnection: {:?}", e);
            } else {
                log::info!(
                    "Registered peer connection: {} {} conn_id={}",
                    ip_version,
                    conn_index,
                    conn_id
                );
            }
        }
    }
}

/// Update UI for a specific connection index
fn update_ui_connection(ip_version: &str, conn_index: usize, metrics: &common::ClientMetrics) {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::JsValue;

    let window = match window() {
        Some(w) => w,
        None => return,
    };

    // Convert metrics to JS object
    let metrics_obj = js_sys::Object::new();

    // Helper function to set array property
    let set_array_prop = |obj: &js_sys::Object, key: &str, values: &[f64]| {
        let arr = js_sys::Array::new();
        for &val in values {
            arr.push(&JsValue::from_f64(val));
        }
        let _ = js_sys::Reflect::set(obj, &JsValue::from_str(key), &arr);
    };

    set_array_prop(&metrics_obj, "s2c_throughput", &metrics.s2c_throughput);
    set_array_prop(&metrics_obj, "s2c_delay_avg", &metrics.s2c_delay_avg);
    set_array_prop(&metrics_obj, "s2c_jitter", &metrics.s2c_jitter);
    set_array_prop(&metrics_obj, "s2c_loss_rate", &metrics.s2c_loss_rate);
    set_array_prop(&metrics_obj, "s2c_reorder_rate", &metrics.s2c_reorder_rate);

    // Call JavaScript function updateConnectionMetrics(ipVersion, connIndex, metrics)
    if let Ok(update_fn) =
        js_sys::Reflect::get(&window, &JsValue::from_str("updateConnectionMetrics"))
    {
        if let Some(func) = update_fn.dyn_ref::<js_sys::Function>() {
            if let Err(e) = func.call3(
                &JsValue::NULL,
                &JsValue::from_str(ip_version),
                &JsValue::from_f64(conn_index as f64),
                &metrics_obj,
            ) {
                log::warn!("Failed to call updateConnectionMetrics: {:?}", e);
            }
        }
    }
}

fn call_add_metrics_data(
    ipv4_metrics: &common::ClientMetrics,
    ipv6_metrics: &common::ClientMetrics,
) {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::JsValue;

    // Check if chart data collection should begin (after the delay period)
    let should_collect = CHART_COLLECTION_START_MS.with(|start| {
        match *start.borrow() {
            Some(start_time) => current_time_ms() >= start_time,
            None => true, // If no start time is set, allow collection (e.g., for start_measurement)
        }
    });

    if !should_collect {
        return;
    }

    let window = match window() {
        Some(w) => w,
        None => return,
    };

    // Convert metrics to JS objects
    let ipv4_obj = js_sys::Object::new();
    let ipv6_obj = js_sys::Object::new();

    // Helper function to set array property
    let set_array_prop = |obj: &js_sys::Object, key: &str, values: &[f64]| {
        let arr = js_sys::Array::new();
        for &val in values {
            arr.push(&JsValue::from_f64(val));
        }
        let _ = js_sys::Reflect::set(obj, &JsValue::from_str(key), &arr);
    };

    // Set IPv4 metrics
    set_array_prop(&ipv4_obj, "s2c_throughput", &ipv4_metrics.s2c_throughput);
    set_array_prop(&ipv4_obj, "s2c_delay_avg", &ipv4_metrics.s2c_delay_avg);
    set_array_prop(&ipv4_obj, "s2c_jitter", &ipv4_metrics.s2c_jitter);
    set_array_prop(&ipv4_obj, "s2c_loss_rate", &ipv4_metrics.s2c_loss_rate);
    set_array_prop(
        &ipv4_obj,
        "s2c_reorder_rate",
        &ipv4_metrics.s2c_reorder_rate,
    );

    // Set IPv6 metrics
    set_array_prop(&ipv6_obj, "s2c_throughput", &ipv6_metrics.s2c_throughput);
    set_array_prop(&ipv6_obj, "s2c_delay_avg", &ipv6_metrics.s2c_delay_avg);
    set_array_prop(&ipv6_obj, "s2c_jitter", &ipv6_metrics.s2c_jitter);
    set_array_prop(&ipv6_obj, "s2c_loss_rate", &ipv6_metrics.s2c_loss_rate);
    set_array_prop(
        &ipv6_obj,
        "s2c_reorder_rate",
        &ipv6_metrics.s2c_reorder_rate,
    );

    // Call JavaScript function
    if let Ok(add_metrics_fn) = js_sys::Reflect::get(&window, &JsValue::from_str("addMetricsData"))
    {
        if let Some(func) = add_metrics_fn.dyn_ref::<js_sys::Function>() {
            if let Err(e) = func.call2(&JsValue::NULL, &ipv4_obj, &ipv6_obj) {
                log::warn!("Failed to call addMetricsData: {:?}", e);
            }
        }
    }
}

fn set_doc_status(status_text: &str) {
    let window = match window() {
        Some(w) => w,
        None => return,
    };

    let document = match window.document() {
        Some(d) => d,
        None => return,
    };
    set_element_text(&document, "status", status_text);
}

fn update_ui(metrics: &common::ClientMetrics) {
    let window = match window() {
        Some(w) => w,
        None => return,
    };

    let document = match window.document() {
        Some(d) => d,
        None => return,
    };

    let format_bytes = |bytes: f64| -> String {
        if bytes >= 1024.0 * 1024.0 {
            format!("{:.2} MB/s", bytes / (1024.0 * 1024.0))
        } else if bytes >= 1024.0 {
            format!("{:.2} KB/s", bytes / 1024.0)
        } else {
            format!("{:.0} B/s", bytes)
        }
    };

    let format_ms = |ms: f64| -> String {
        if ms > 0.0 {
            format!("{:.1}", ms)
        } else {
            "-".to_string()
        }
    };

    let format_pct = |pct: f64| -> String {
        if pct > 0.0 {
            format!("{:.1}%", pct)
        } else {
            "0%".to_string()
        }
    };

    // C2S metrics (client sends, server measures - we don't have direct access)
    // For now, show as "N/A" or leave empty since C2S is measured on server side

    // S2C Throughput
    set_element_text(
        &document,
        "s2c-tp-1",
        &format_bytes(metrics.s2c_throughput[0]),
    );
    set_element_text(
        &document,
        "s2c-tp-10",
        &format_bytes(metrics.s2c_throughput[1]),
    );
    set_element_text(
        &document,
        "s2c-tp-60",
        &format_bytes(metrics.s2c_throughput[2]),
    );

    // S2C Delay
    set_element_text(
        &document,
        "s2c-delay-1",
        &format_ms(metrics.s2c_delay_avg[0]),
    );
    set_element_text(
        &document,
        "s2c-delay-10",
        &format_ms(metrics.s2c_delay_avg[1]),
    );
    set_element_text(
        &document,
        "s2c-delay-60",
        &format_ms(metrics.s2c_delay_avg[2]),
    );

    // S2C Jitter
    set_element_text(&document, "s2c-jitter-1", &format_ms(metrics.s2c_jitter[0]));
    set_element_text(
        &document,
        "s2c-jitter-10",
        &format_ms(metrics.s2c_jitter[1]),
    );
    set_element_text(
        &document,
        "s2c-jitter-60",
        &format_ms(metrics.s2c_jitter[2]),
    );

    // S2C Loss Rate
    set_element_text(
        &document,
        "s2c-loss-1",
        &format_pct(metrics.s2c_loss_rate[0]),
    );
    set_element_text(
        &document,
        "s2c-loss-10",
        &format_pct(metrics.s2c_loss_rate[1]),
    );
    set_element_text(
        &document,
        "s2c-loss-60",
        &format_pct(metrics.s2c_loss_rate[2]),
    );

    // S2C Reorder Rate
    set_element_text(
        &document,
        "s2c-reorder-1",
        &format_pct(metrics.s2c_reorder_rate[0]),
    );
    set_element_text(
        &document,
        "s2c-reorder-10",
        &format_pct(metrics.s2c_reorder_rate[1]),
    );
    set_element_text(
        &document,
        "s2c-reorder-60",
        &format_pct(metrics.s2c_reorder_rate[2]),
    );

    // Also try the original IDs for backward compatibility
    set_element_text(&document, "c2s-tp-1", "N/A");
    set_element_text(&document, "c2s-tp-10", "N/A");
    set_element_text(&document, "c2s-tp-60", "N/A");
    set_element_text(&document, "c2s-delay-1", "N/A");
    set_element_text(&document, "c2s-delay-10", "N/A");
    set_element_text(&document, "c2s-delay-60", "N/A");
    set_element_text(&document, "c2s-jitter-1", "N/A");
    set_element_text(&document, "c2s-jitter-10", "N/A");
    set_element_text(&document, "c2s-jitter-60", "N/A");
}

fn set_element_text(document: &Document, id: &str, text: &str) {
    if let Some(element) = document.get_element_by_id(id) {
        element.set_text_content(Some(text));
    }
}

// Recorder sensor callbacks
#[wasm_bindgen]
pub fn on_gps_update(
    latitude: f64,
    longitude: f64,
    accuracy: f64,
    altitude: Option<f64>,
    altitude_accuracy: Option<f64>,
    heading: Option<f64>,
    speed: Option<f64>,
) {
    if let Ok(mut manager_guard) = SENSOR_MANAGER.lock() {
        if let Some(ref mut mgr) = *manager_guard {
            let gps_data = recorder::types::GpsData {
                latitude,
                longitude,
                altitude,
                accuracy,
                altitude_accuracy,
                heading,
                speed,
            };
            mgr.update_gps(gps_data);
        }
    }
}

#[wasm_bindgen]
pub fn on_orientation(alpha: Option<f64>, beta: Option<f64>, gamma: Option<f64>, absolute: bool) {
    if let Ok(mut manager_guard) = SENSOR_MANAGER.lock() {
        if let Some(ref mut mgr) = *manager_guard {
            let orientation_data = recorder::types::OrientationData {
                alpha,
                beta,
                gamma,
                absolute,
            };
            mgr.update_orientation(orientation_data);
        }
    }
}

#[wasm_bindgen]
pub fn on_motion(
    timestamp_utc: String,
    current_time: f64,
    acc_x: f64,
    acc_y: f64,
    acc_z: f64,
    acc_g_x: f64,
    acc_g_y: f64,
    acc_g_z: f64,
    rot_alpha: f64,
    rot_beta: f64,
    rot_gamma: f64,
) {
    if let Ok(mut manager_guard) = SENSOR_MANAGER.lock() {
        if let Some(ref mut mgr) = *manager_guard {
            let acceleration = recorder::types::AccelerationData {
                x: acc_x,
                y: acc_y,
                z: acc_z,
            };
            let acceleration_g = recorder::types::AccelerationData {
                x: acc_g_x,
                y: acc_g_y,
                z: acc_g_z,
            };
            let rotation = recorder::types::RotationData {
                alpha: rot_alpha,
                beta: rot_beta,
                gamma: rot_gamma,
            };
            mgr.add_motion_event(timestamp_utc, current_time, acceleration, acceleration_g, rotation);
        }
    }
}

#[wasm_bindgen]
pub fn on_magnetometer(alpha: Option<f64>, beta: Option<f64>, gamma: Option<f64>, absolute: bool) {
    if let Ok(mut manager_guard) = SENSOR_MANAGER.lock() {
        if let Some(ref mut mgr) = *manager_guard {
            let mag_data = recorder::types::OrientationData {
                alpha,
                beta,
                gamma,
                absolute,
            };
            mgr.update_magnetometer(mag_data);
        }
    }
}

#[wasm_bindgen]
pub fn set_sensor_overlay_enabled(enabled: bool) {
    if let Ok(mut manager_guard) = SENSOR_MANAGER.lock() {
        if let Some(ref mut mgr) = *manager_guard {
            mgr.set_overlay_enabled(enabled);
        }
    }
}

// Recorder initialization
#[wasm_bindgen]
pub fn init_recorder() {
    recorder::ui::init_recorder_panel();
}

// Recorder download and management functions (Issue 012)
#[wasm_bindgen]
pub async fn download_video(id: String) -> Result<(), JsValue> {
    use recorder::utils::log;

    log(&format!("[Recorder] Downloading video: {}", id));

    let window = web_sys::window().ok_or("No window")?;
    let document = window.document().ok_or("No document")?;

    // Initialize database before accessing (Issue 032)
    recorder::storage::openDb().await?;

    // Get recording from IndexedDB
    let recording_js = recorder::storage::getRecording(&id).await?;

    let obj = js_sys::Object::from(recording_js);
    let video_blob_js = js_sys::Reflect::get(&obj, &"videoBlob".into())?;
    let video_blob: web_sys::Blob = video_blob_js.dyn_into()?;

    let metadata_js = js_sys::Reflect::get(&obj, &"metadata".into())?;
    let mime_type = js_sys::Reflect::get(&metadata_js, &"mimeType".into())?
        .as_string()
        .unwrap_or("video/webm".to_string());

    // Create blob URL
    let url = web_sys::Url::create_object_url_with_blob(&video_blob)?;

    // Create anchor element and trigger download
    let a: web_sys::HtmlAnchorElement = document.create_element("a")?.dyn_into()?;
    a.set_href(&url);

    let extension = if mime_type.contains("mp4") { "mp4" } else { "webm" };
    a.set_download(&format!("netpoke_recording_{}.{}", id, extension));
    a.click();

    // Revoke URL after delay
    let url_clone = url.clone();
    let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move || {
        let _ = web_sys::Url::revoke_object_url(&url_clone);
    }) as Box<dyn Fn()>);
    window.set_timeout_with_callback_and_timeout_and_arguments_0(
        closure.as_ref().unchecked_ref(),
        1000,
    )?;
    closure.forget();

    log(&format!("[Recorder] Video download triggered: {}", id));
    Ok(())
}

#[wasm_bindgen]
pub async fn download_motion_data(id: String) -> Result<(), JsValue> {
    use recorder::utils::log;

    log(&format!("[Recorder] Downloading motion data: {}", id));

    let window = web_sys::window().ok_or("No window")?;
    let document = window.document().ok_or("No document")?;

    // Initialize database before accessing (Issue 032)
    recorder::storage::openDb().await?;

    // Get recording from IndexedDB
    let recording_js = recorder::storage::getRecording(&id).await?;

    let obj = js_sys::Object::from(recording_js);
    let metadata_js = js_sys::Reflect::get(&obj, &"metadata".into())?;
    let motion_data_js = js_sys::Reflect::get(&obj, &"motionData".into())?;

    // Create combined JSON
    let export_data = js_sys::Object::new();
    js_sys::Reflect::set(&export_data, &"metadata".into(), &metadata_js)?;
    js_sys::Reflect::set(&export_data, &"motionData".into(), &motion_data_js)?;

    let json_string = js_sys::JSON::stringify(&export_data)?
        .as_string()
        .ok_or("Failed to stringify JSON")?;

    // Create blob
    let array = js_sys::Uint8Array::from(json_string.as_bytes());
    let blob_parts = js_sys::Array::new();
    blob_parts.push(&array);
    
    let mut blob_options = web_sys::BlobPropertyBag::new();
    blob_options.type_("application/json");
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&blob_parts, &blob_options)?;

    // Create blob URL
    let url = web_sys::Url::create_object_url_with_blob(&blob)?;

    // Create anchor element and trigger download
    let a: web_sys::HtmlAnchorElement = document.create_element("a")?.dyn_into()?;
    a.set_href(&url);
    a.set_download(&format!("netpoke_motion_{}.json", id));
    a.click();

    // Revoke URL after delay
    let url_clone = url.clone();
    let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move || {
        let _ = web_sys::Url::revoke_object_url(&url_clone);
    }) as Box<dyn Fn()>);
    window.set_timeout_with_callback_and_timeout_and_arguments_0(
        closure.as_ref().unchecked_ref(),
        1000,
    )?;
    closure.forget();

    log(&format!("[Recorder] Motion data download triggered: {}", id));
    Ok(())
}

#[wasm_bindgen]
pub async fn delete_recording_by_id(id: String) -> Result<(), JsValue> {
    use recorder::storage::IndexedDbWrapper;
    use recorder::utils::log;

    log(&format!("[Recorder] Deleting recording: {}", id));

    // Confirm deletion
    let window = web_sys::window().ok_or("No window")?;
    let confirmed = window.confirm_with_message(&format!(
        "Are you sure you want to delete recording {}?",
        id
    ))?;

    if !confirmed {
        log("[Recorder] Deletion cancelled by user");
        return Ok(());
    }

    // Delete from IndexedDB
    let db = IndexedDbWrapper::open().await?;
    db.delete_recording(&id).await?;

    log(&format!("[Recorder] Recording deleted: {}", id));

    // Refresh recordings list (call JavaScript function if available)
    if let Ok(refresh_fn) = js_sys::Reflect::get(&window, &"refreshRecordingsList".into()) {
        if refresh_fn.is_function() {
            let func: js_sys::Function = refresh_fn.dyn_into()?;
            let _ = func.call0(&window);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_basic() {
        assert_eq!(2 + 2, 4);
    }
}
