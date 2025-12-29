mod webrtc;
mod signaling;
mod measurements;
use crate::measurements::current_time_ms;

use wasm_bindgen::prelude::*;
use web_sys::{window, Document};
use std::cell::RefCell;

// Path analysis timeout in milliseconds (30 seconds)
const PATH_ANALYSIS_TIMEOUT_MS: u32 = 30000;

// Mode constants for measurement type
const MODE_TRACEROUTE: &str = "traceroute";
const MODE_MEASUREMENT: &str = "measurement";

// Global wake lock sentinel (stored as JsValue since WakeLockSentinel might not be exposed)
thread_local! {
    static WAKE_LOCK: RefCell<Option<JsValue>> = RefCell::new(None);
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

    // Request wake lock to prevent device from sleeping
    if let Err(e) = request_wake_lock().await {
        log::warn!("Failed to acquire wake lock: {:?}", e);
        // Continue anyway - wake lock is optional
    }

    // Create IPv4 connections
    let mut ipv4_connections = Vec::with_capacity(count);
    let mut parent_id: Option<String> = None;
    
    for i in 0..count {
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
        ipv4_connections.push(conn);
    }

    // Create IPv6 connections (use parent from first IPv4 connection)
    let mut ipv6_connections = Vec::with_capacity(count);
    for i in 0..count {
        let conn = webrtc::WebRtcConnection::new_with_ip_version_and_mode(
            "ipv6", 
            parent_id.clone(), 
            None, 
            None  // conn_id will be auto-generated
        ).await?;
        log::info!("IPv6 connection {} created with client_id: {}, conn_id: {}", i, conn.client_id, conn.conn_id);
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
    gloo_timers::callback::Interval::new(100, move || {
        for state in &calc_states_for_interval {
            state.borrow_mut().calculate_metrics();
        }
    }).forget();

    // Collect all connection states for UI updates
    let ipv4_states: Vec<_> = ipv4_connections.iter().map(|c| c.state.clone()).collect();
    let ipv6_states: Vec<_> = ipv6_connections.iter().map(|c| c.state.clone()).collect();
    let conn_count = count;
    
    // Start UI update loop that updates all connections
    gloo_timers::callback::Interval::new(500, move || {
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
    }).forget();

    // Keep connections alive
    for conn in ipv4_connections {
        std::mem::forget(conn);
    }
    for conn in ipv6_connections {
        std::mem::forget(conn);
    }

    Ok(())
}

/// Stop the measurement and release the wake lock
#[wasm_bindgen]
pub fn stop_measurement() {
    log::info!("Stopping measurement...");
    release_wake_lock();
}

/// Analyze the network: first perform traceroute, then start measurements
#[wasm_bindgen]
pub async fn analyze_network() -> Result<(), JsValue> {
    // Default to 1 connection per address family
    analyze_network_with_count(1).await
}

/// Analyze the network path (traceroute) once and then close connections
#[wasm_bindgen]
pub async fn analyze_path() -> Result<(), JsValue> {
    // Default to 1 connection per address family
    analyze_path_with_count(1).await
}

/// Analyze the network path with multiple connections per address family for ECMP testing
#[wasm_bindgen]
pub async fn analyze_path_with_count(conn_count: u8) -> Result<(), JsValue> {
    let count = conn_count.clamp(1, 16) as usize;
    log::info!("Starting path analysis (traceroute) with {} connections per address family...", count);

    // Request wake lock to prevent device from sleeping
    if let Err(e) = request_wake_lock().await {
        log::warn!("Failed to acquire wake lock: {:?}", e);
        // Continue anyway - wake lock is optional
    }

    // Create IPv4 connections with traceroute mode
    let mut ipv4_connections = Vec::with_capacity(count);
    let mut parent_id: Option<String> = None;
    
    for i in 0..count {
        let conn = webrtc::WebRtcConnection::new_with_ip_version_and_mode(
            "ipv4", 
            parent_id.clone(), 
            Some(MODE_TRACEROUTE.to_string()), 
            None  // conn_id will be auto-generated
        ).await?;
        
        if i == 0 {
            parent_id = Some(conn.client_id.clone());
        }
        
        // Enable traceroute mode to prevent measurement data collection
        conn.set_traceroute_mode(true);
        
        log::info!("IPv4 traceroute connection {} created with client_id: {}, conn_id: {}", i, conn.client_id, conn.conn_id);
        ipv4_connections.push(conn);
    }

    // Create IPv6 connections with traceroute mode
    let mut ipv6_connections = Vec::with_capacity(count);
    for i in 0..count {
        let conn = webrtc::WebRtcConnection::new_with_ip_version_and_mode(
            "ipv6", 
            parent_id.clone(), 
            Some(MODE_TRACEROUTE.to_string()), 
            None  // conn_id will be auto-generated
        ).await?;
        
        // Enable traceroute mode to prevent measurement data collection
        conn.set_traceroute_mode(true);
        
        log::info!("IPv6 traceroute connection {} created with client_id: {}, conn_id: {}", i, conn.client_id, conn.conn_id);
        ipv6_connections.push(conn);
    }

    // Collect peers for cleanup
    let peers: Vec<_> = ipv4_connections.iter()
        .chain(ipv6_connections.iter())
        .map(|c| c.peer.clone())
        .collect();

    // Wait for path analysis timeout to collect traceroute data
    log::info!("Collecting traceroute data for {} seconds...", PATH_ANALYSIS_TIMEOUT_MS / 1000);
    
    // Use a timer to wait
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let window = web_sys::window().expect("no global window available during path analysis timeout setup");
        window
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, PATH_ANALYSIS_TIMEOUT_MS as i32)
            .expect("Failed to set timeout for path analysis");
    });
    wasm_bindgen_futures::JsFuture::from(promise).await?;

    log::info!("Path analysis complete, closing {} connections...", peers.len());

    // Close all connections
    for peer in peers {
        peer.close();
    }

    // Release wake lock
    release_wake_lock();

    log::info!("Path analysis finished");

    Ok(())
}

/// Analyze the network with multiple connections: perform traceroute first, then start measurements
#[wasm_bindgen]
pub async fn analyze_network_with_count(conn_count: u8) -> Result<(), JsValue> {
    let count = conn_count.clamp(1, 16) as usize;
    log::info!("Starting network analysis with {} connections per address family...", count);

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
        ipv4_connections.push(conn);
    }

    // Create IPv6 connections with traceroute mode
    let mut ipv6_connections = Vec::with_capacity(count);
    for i in 0..count {
        let conn = webrtc::WebRtcConnection::new_with_ip_version_and_mode(
            "ipv6", 
            parent_id.clone(), 
            Some(MODE_TRACEROUTE.to_string()), 
            None  // conn_id will be auto-generated
        ).await?;
        
        // Enable traceroute mode to prevent measurement data collection during Phase 1
        conn.set_traceroute_mode(true);
        
        log::info!("IPv6 connection {} created with client_id: {}, conn_id: {}", i, conn.client_id, conn.conn_id);
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
    // Note: We use .forget() to prevent the interval from being dropped, allowing it to run
    // indefinitely for continuous measurements until the page is closed by the user.
    let calc_states_for_interval = calc_states.clone();
    gloo_timers::callback::Interval::new(100, move || {
        for state in &calc_states_for_interval {
            state.borrow_mut().calculate_metrics();
        }
    }).forget();

    // Collect all connection states for UI updates
    let ipv4_states: Vec<_> = ipv4_connections.iter().map(|c| c.state.clone()).collect();
    let ipv6_states: Vec<_> = ipv6_connections.iter().map(|c| c.state.clone()).collect();
    let conn_count_ui = count;
    
    // Start UI update loop that updates all connections
    // Note: We use .forget() to prevent the interval from being dropped, allowing it to run
    // indefinitely for continuous UI updates until the page is closed by the user.
    gloo_timers::callback::Interval::new(500, move || {
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
    }).forget();

    // Keep connections alive for measurements
    // Note: We intentionally use std::mem::forget here to keep connections alive indefinitely.
    // This is required for long-running network measurements. The WebRTC connections will be
    // cleaned up by the browser when the page is closed or navigated away.
    for conn in ipv4_connections {
        std::mem::forget(conn);
    }
    for conn in ipv6_connections {
        std::mem::forget(conn);
    }

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
