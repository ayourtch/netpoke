mod webrtc;
mod signaling;
mod measurements;
use crate::measurements::current_time_ms;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{window, Document, RtcPeerConnection};
use std::cell::RefCell;
use gloo_timers::callback::Interval;
use serde::Deserialize;

// Path analysis timeout in milliseconds (30 seconds)
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

// Global state for tracking active connections and testing status
thread_local! {
    static WAKE_LOCK: RefCell<Option<JsValue>> = RefCell::new(None);
    static ACTIVE_PEERS: RefCell<Vec<RtcPeerConnection>> = RefCell::new(Vec::new());
    static ACTIVE_INTERVALS: RefCell<Vec<Interval>> = RefCell::new(Vec::new());
    static IS_TESTING_ACTIVE: RefCell<bool> = RefCell::new(false);
    // Timestamp (in ms) when chart data collection should begin (after delay period)
    static CHART_COLLECTION_START_MS: RefCell<Option<u64>> = RefCell::new(None);
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
            log::warn!("Failed to create request: {:?}, using default client config", e);
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
            log::warn!("Failed to get JSON from response: {:?}, using default client config", e);
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
            log::warn!("Failed to deserialize client config: {:?}, using default", e);
            ClientConfig::default()
        }
    }
}

/// Sleep for the specified number of milliseconds
async fn sleep_ms(ms: u32) {
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let window = web_sys::window().expect("no global window available");
        window
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms as i32)
            .expect("Failed to set timeout");
    });
    wasm_bindgen_futures::JsFuture::from(promise).await.expect("Failed to sleep");
}

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::default());

    log::info!("WASM client initialized");
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
            &js_sys::Array::of1(&JsValue::from_str("screen"))
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
}

/// Check if testing is currently active
#[wasm_bindgen]
pub fn is_testing_active() -> bool {
    IS_TESTING_ACTIVE.with(|state| *state.borrow())
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
    log::info!("Starting dual-stack network measurement with {} connections per address family...", count);

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
            None  // conn_id will be auto-generated
        ).await?;
        
        if i == 0 {
            parent_id = Some(conn.client_id.clone());
        }
        log::info!("IPv4 connection {} created with client_id: {}, conn_id: {}", i, conn.client_id, conn.conn_id);
        
        // Register the connection with JavaScript for display
        register_peer_connection_js("ipv4", i, &conn.conn_id, WEBRTC_MANAGED_ADDRESS, WEBRTC_MANAGED_ADDRESS);
        
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
            None  // conn_id will be auto-generated
        ).await?;
        log::info!("IPv6 connection {} created with client_id: {}, conn_id: {}", i, conn.client_id, conn.conn_id);
        
        // Register the connection with JavaScript for display
        register_peer_connection_js("ipv6", i, &conn.conn_id, WEBRTC_MANAGED_ADDRESS, WEBRTC_MANAGED_ADDRESS);
        
        // Set up callback to update addresses when connection is established
        conn.setup_address_update_callback("ipv6", i);
        
        ipv6_connections.push(conn);
    }

    // Collect states for calculation and UI updates
    let mut calc_states: Vec<std::rc::Rc<std::cell::RefCell<measurements::MeasurementState>>> = Vec::new();
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

/// Analyze the network with multiple connections: perform traceroute first, then start measurements
#[wasm_bindgen]
pub async fn analyze_network_with_count(conn_count: u8) -> Result<(), JsValue> {
    let count = conn_count.clamp(1, 16) as usize;
    log::info!("Starting network analysis with {} connections per address family...", count);

    // Fetch client configuration from server
    let client_config = fetch_client_config().await;
    let connection_delay_ms = client_config.webrtc_connection_delay_ms;
    log::info!("Using WebRTC connection delay: {}ms", connection_delay_ms);

    // Request wake lock to prevent device from sleeping
    if let Err(e) = request_wake_lock().await {
        log::warn!("Failed to acquire wake lock: {:?}", e);
        // Continue anyway - wake lock is optional
    }

    // PHASE 1: Create connections with traceroute mode and collect path data
    log::info!("PHASE 1: Establishing WebRTC connections and analyzing paths (traceroute)...");
    
    // Create IPv4 connections with traceroute mode
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
            Some(MODE_TRACEROUTE.to_string()), 
            None  // conn_id will be auto-generated
        ).await?;
        
        if i == 0 {
            parent_id = Some(conn.client_id.clone());
        }
        
        // Enable traceroute mode to prevent measurement data collection during Phase 1
        conn.set_traceroute_mode(true);
        
        log::info!("IPv4 connection {} created with client_id: {}, conn_id: {}", i, conn.client_id, conn.conn_id);
        
        // Register the connection with JavaScript for display
        register_peer_connection_js("ipv4", i, &conn.conn_id, WEBRTC_MANAGED_ADDRESS, WEBRTC_MANAGED_ADDRESS);
        
        // Set up callback to update addresses when connection is established
        conn.setup_address_update_callback("ipv4", i);
        
        ipv4_connections.push(conn);
    }

    // Create IPv6 connections with traceroute mode
    let mut ipv6_connections = Vec::with_capacity(count);
    for i in 0..count {
        // Add delay between connection attempts (including between IPv4 and IPv6 groups)
        if connection_delay_ms > 0 {
            sleep_ms(connection_delay_ms).await;
        }
        
        let conn = webrtc::WebRtcConnection::new_with_ip_version_and_mode(
            "ipv6", 
            parent_id.clone(), 
            Some(MODE_TRACEROUTE.to_string()), 
            None  // conn_id will be auto-generated
        ).await?;
        
        // Enable traceroute mode to prevent measurement data collection during Phase 1
        conn.set_traceroute_mode(true);
        
        log::info!("IPv6 connection {} created with client_id: {}, conn_id: {}", i, conn.client_id, conn.conn_id);
        
        // Register the connection with JavaScript for display
        register_peer_connection_js("ipv6", i, &conn.conn_id, WEBRTC_MANAGED_ADDRESS, WEBRTC_MANAGED_ADDRESS);
        
        // Set up callback to update addresses when connection is established
        conn.setup_address_update_callback("ipv6", i);
        sleep_ms(500).await;
        
        ipv6_connections.push(conn);
    }

    // Wait for traceroute data to be collected
    log::info!("Collecting traceroute data for {} seconds...", PATH_ANALYSIS_TIMEOUT_MS / 1000);
    
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let window = web_sys::window().expect("no global window available during traceroute timeout");
        window
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, PATH_ANALYSIS_TIMEOUT_MS as i32)
            .expect("Failed to set timeout for traceroute");
    });
    wasm_bindgen_futures::JsFuture::from(promise).await?;

    log::info!("PHASE 2: Traceroute complete, starting network measurements...");

    // Send stop traceroute messages to all connections
    for conn in &ipv4_connections {
        if let Err(e) = conn.send_stop_traceroute().await {
            log::warn!("Failed to send stop traceroute for IPv4 connection {}: {:?}", conn.conn_id, e);
        }
    }
    for conn in &ipv6_connections {
        if let Err(e) = conn.send_stop_traceroute().await {
            log::warn!("Failed to send stop traceroute for IPv6 connection {}: {:?}", conn.conn_id, e);
        }
    }
    log::info!("Stop traceroute messages sent to all connections");

    // Clear metrics before starting Phase 2 measurements
    log::info!("Clearing metrics before Phase 2...");
    for conn in &ipv4_connections {
        conn.state.borrow_mut().clear_metrics();
    }
    for conn in &ipv6_connections {
        conn.state.borrow_mut().clear_metrics();
    }
    log::info!("Client-side metrics cleared");

    // Set the chart collection start time to now + delay
    // This ensures the Live Metrics Graph only starts collecting data 10 seconds after Phase 1 finishes
    let chart_start_time = current_time_ms() + CHART_COLLECTION_DELAY_MS;
    CHART_COLLECTION_START_MS.with(|start| {
        *start.borrow_mut() = Some(chart_start_time);
    });
    log::info!("Chart data collection will begin in {} seconds", CHART_COLLECTION_DELAY_MS / 1000);

    // PHASE 2: Start measurement loops on the same connections
    // Collect states for calculation and UI updates
    let mut calc_states: Vec<std::rc::Rc<std::cell::RefCell<measurements::MeasurementState>>> = Vec::new();
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
    
    // Start UI update loop that updates all connections
    let ui_interval = gloo_timers::callback::Interval::new(500, move || {
        // Update metrics for each IPv4 connection
        for (i, state) in ipv4_states.iter().enumerate() {
            let state_ref = state.borrow();
            if conn_count_ui > 1 {
                // Multi-connection mode: update connection-specific tables
                update_ui_connection("ipv4", i, &state_ref.metrics);
            }
            // Note: Single connection mode (conn_count_ui == 1) is handled by update_ui_dual below
        }
        
        // Update metrics for each IPv6 connection
        for (i, state) in ipv6_states.iter().enumerate() {
            let state_ref = state.borrow();
            if conn_count_ui > 1 {
                // Multi-connection mode: update connection-specific tables
                update_ui_connection("ipv6", i, &state_ref.metrics);
            }
            // Note: Single connection mode (conn_count_ui == 1) is handled by update_ui_dual below
        }
        
        // For single connection mode or chart updates, use first connection of each type
        // This also updates the default metrics tables in single connection mode
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
    set_element_text(&document, "ipv4-s2c-tp-1", &format_bytes(ipv4_metrics.s2c_throughput[0]));
    set_element_text(&document, "ipv4-s2c-tp-10", &format_bytes(ipv4_metrics.s2c_throughput[1]));
    set_element_text(&document, "ipv4-s2c-tp-60", &format_bytes(ipv4_metrics.s2c_throughput[2]));
    set_element_text(&document, "ipv4-s2c-delay-1", &format_ms(ipv4_metrics.s2c_delay_avg[0]));
    set_element_text(&document, "ipv4-s2c-delay-10", &format_ms(ipv4_metrics.s2c_delay_avg[1]));
    set_element_text(&document, "ipv4-s2c-delay-60", &format_ms(ipv4_metrics.s2c_delay_avg[2]));
    set_element_text(&document, "ipv4-s2c-jitter-1", &format_ms(ipv4_metrics.s2c_jitter[0]));
    set_element_text(&document, "ipv4-s2c-jitter-10", &format_ms(ipv4_metrics.s2c_jitter[1]));
    set_element_text(&document, "ipv4-s2c-jitter-60", &format_ms(ipv4_metrics.s2c_jitter[2]));
    set_element_text(&document, "ipv4-s2c-loss-1", &format_pct(ipv4_metrics.s2c_loss_rate[0]));
    set_element_text(&document, "ipv4-s2c-loss-10", &format_pct(ipv4_metrics.s2c_loss_rate[1]));
    set_element_text(&document, "ipv4-s2c-loss-60", &format_pct(ipv4_metrics.s2c_loss_rate[2]));
    set_element_text(&document, "ipv4-s2c-reorder-1", &format_pct(ipv4_metrics.s2c_reorder_rate[0]));
    set_element_text(&document, "ipv4-s2c-reorder-10", &format_pct(ipv4_metrics.s2c_reorder_rate[1]));
    set_element_text(&document, "ipv4-s2c-reorder-60", &format_pct(ipv4_metrics.s2c_reorder_rate[2]));

    // Update IPv6 metrics
    set_element_text(&document, "ipv6-s2c-tp-1", &format_bytes(ipv6_metrics.s2c_throughput[0]));
    set_element_text(&document, "ipv6-s2c-tp-10", &format_bytes(ipv6_metrics.s2c_throughput[1]));
    set_element_text(&document, "ipv6-s2c-tp-60", &format_bytes(ipv6_metrics.s2c_throughput[2]));
    set_element_text(&document, "ipv6-s2c-delay-1", &format_ms(ipv6_metrics.s2c_delay_avg[0]));
    set_element_text(&document, "ipv6-s2c-delay-10", &format_ms(ipv6_metrics.s2c_delay_avg[1]));
    set_element_text(&document, "ipv6-s2c-delay-60", &format_ms(ipv6_metrics.s2c_delay_avg[2]));
    set_element_text(&document, "ipv6-s2c-jitter-1", &format_ms(ipv6_metrics.s2c_jitter[0]));
    set_element_text(&document, "ipv6-s2c-jitter-10", &format_ms(ipv6_metrics.s2c_jitter[1]));
    set_element_text(&document, "ipv6-s2c-jitter-60", &format_ms(ipv6_metrics.s2c_jitter[2]));
    set_element_text(&document, "ipv6-s2c-loss-1", &format_pct(ipv6_metrics.s2c_loss_rate[0]));
    set_element_text(&document, "ipv6-s2c-loss-10", &format_pct(ipv6_metrics.s2c_loss_rate[1]));
    set_element_text(&document, "ipv6-s2c-loss-60", &format_pct(ipv6_metrics.s2c_loss_rate[2]));
    set_element_text(&document, "ipv6-s2c-reorder-1", &format_pct(ipv6_metrics.s2c_reorder_rate[0]));
    set_element_text(&document, "ipv6-s2c-reorder-10", &format_pct(ipv6_metrics.s2c_reorder_rate[1]));
    set_element_text(&document, "ipv6-s2c-reorder-60", &format_pct(ipv6_metrics.s2c_reorder_rate[2]));

    // Update chart with metrics data
    call_add_metrics_data(ipv4_metrics, ipv6_metrics);
}

/// Register a peer connection with JavaScript for display in the peer connections list.
/// 
/// # Parameters
/// - `ip_version`: The IP version of the connection ("ipv4" or "ipv6")
/// - `conn_index`: The zero-based index of this connection within its IP version group
/// - `conn_id`: The unique connection ID (UUID) that matches the conn_id in traceroute data
/// - `local_address`: The local address string (IP:port or placeholder)
/// - `remote_address`: The remote address string (IP:port or placeholder)
fn register_peer_connection_js(ip_version: &str, conn_index: usize, conn_id: &str, local_address: &str, remote_address: &str) {
    use wasm_bindgen::JsValue;
    use wasm_bindgen::JsCast;
    
    let window = match window() {
        Some(w) => w,
        None => return,
    };
    
    // Call JavaScript function registerPeerConnection(ipVersion, connIndex, connId, localAddress, remoteAddress)
    if let Ok(register_fn) = js_sys::Reflect::get(&window, &JsValue::from_str("registerPeerConnection")) {
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
                log::info!("Registered peer connection: {} {} conn_id={}", ip_version, conn_index, conn_id);
            }
        }
    }
}

/// Update UI for a specific connection index
fn update_ui_connection(ip_version: &str, conn_index: usize, metrics: &common::ClientMetrics) {
    use wasm_bindgen::JsValue;
    use wasm_bindgen::JsCast;
    
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
    if let Ok(update_fn) = js_sys::Reflect::get(&window, &JsValue::from_str("updateConnectionMetrics")) {
        if let Some(func) = update_fn.dyn_ref::<js_sys::Function>() {
            if let Err(e) = func.call3(
                &JsValue::NULL, 
                &JsValue::from_str(ip_version), 
                &JsValue::from_f64(conn_index as f64), 
                &metrics_obj
            ) {
                log::warn!("Failed to call updateConnectionMetrics: {:?}", e);
            }
        }
    }
}

fn call_add_metrics_data(ipv4_metrics: &common::ClientMetrics, ipv6_metrics: &common::ClientMetrics) {
    use wasm_bindgen::JsValue;
    use wasm_bindgen::JsCast;
    
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
    set_array_prop(&ipv4_obj, "s2c_reorder_rate", &ipv4_metrics.s2c_reorder_rate);

    // Set IPv6 metrics
    set_array_prop(&ipv6_obj, "s2c_throughput", &ipv6_metrics.s2c_throughput);
    set_array_prop(&ipv6_obj, "s2c_delay_avg", &ipv6_metrics.s2c_delay_avg);
    set_array_prop(&ipv6_obj, "s2c_jitter", &ipv6_metrics.s2c_jitter);
    set_array_prop(&ipv6_obj, "s2c_loss_rate", &ipv6_metrics.s2c_loss_rate);
    set_array_prop(&ipv6_obj, "s2c_reorder_rate", &ipv6_metrics.s2c_reorder_rate);

    // Call JavaScript function
    if let Ok(add_metrics_fn) = js_sys::Reflect::get(&window, &JsValue::from_str("addMetricsData")) {
        if let Some(func) = add_metrics_fn.dyn_ref::<js_sys::Function>() {
            if let Err(e) = func.call2(&JsValue::NULL, &ipv4_obj, &ipv6_obj) {
                log::warn!("Failed to call addMetricsData: {:?}", e);
            }
        }
    }
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
    set_element_text(&document, "s2c-tp-1", &format_bytes(metrics.s2c_throughput[0]));
    set_element_text(&document, "s2c-tp-10", &format_bytes(metrics.s2c_throughput[1]));
    set_element_text(&document, "s2c-tp-60", &format_bytes(metrics.s2c_throughput[2]));

    // S2C Delay
    set_element_text(&document, "s2c-delay-1", &format_ms(metrics.s2c_delay_avg[0]));
    set_element_text(&document, "s2c-delay-10", &format_ms(metrics.s2c_delay_avg[1]));
    set_element_text(&document, "s2c-delay-60", &format_ms(metrics.s2c_delay_avg[2]));

    // S2C Jitter
    set_element_text(&document, "s2c-jitter-1", &format_ms(metrics.s2c_jitter[0]));
    set_element_text(&document, "s2c-jitter-10", &format_ms(metrics.s2c_jitter[1]));
    set_element_text(&document, "s2c-jitter-60", &format_ms(metrics.s2c_jitter[2]));

    // S2C Loss Rate
    set_element_text(&document, "s2c-loss-1", &format_pct(metrics.s2c_loss_rate[0]));
    set_element_text(&document, "s2c-loss-10", &format_pct(metrics.s2c_loss_rate[1]));
    set_element_text(&document, "s2c-loss-60", &format_pct(metrics.s2c_loss_rate[2]));

    // S2C Reorder Rate  
    set_element_text(&document, "s2c-reorder-1", &format_pct(metrics.s2c_reorder_rate[0]));
    set_element_text(&document, "s2c-reorder-10", &format_pct(metrics.s2c_reorder_rate[1]));
    set_element_text(&document, "s2c-reorder-60", &format_pct(metrics.s2c_reorder_rate[2]));

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

#[cfg(test)]
mod tests {
    #[test]
    fn test_basic() {
        assert_eq!(2 + 2, 4);
    }
}
