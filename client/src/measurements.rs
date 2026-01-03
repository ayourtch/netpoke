use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{RtcDataChannel, MessageEvent};
use common::{ProbePacket, Direction, BulkPacket, ClientMetrics};
use std::cell::RefCell;
use std::rc::Rc;
use std::collections::VecDeque;
use js_sys::Uint8Array;

#[derive(Debug)]
pub struct MeasurementState {
    pub test_count: u64,
    pub test_debug: String,
    pub probe_seq: u64,
    pub conn_id: String,
    pub metrics: ClientMetrics,
    pub received_probes: VecDeque<ReceivedProbe>,
    pub received_bulk_bytes: VecDeque<ReceivedBulk>,
    pub traceroute_started: usize,
    pub traceroute_done: usize,
    pub mtu_traceroute_started: usize,
    pub mtu_traceroute_done: usize,
    pub traceroute_active: bool,
    pub server_side_ready: bool,
    pub measuring_time_ms: Option<u64>,
    // The first TTL that successfully reaches the client on this conn
    pub path_ttl: Option<i32>,
    // Probe stream fields
    pub probe_streams_active: bool,
    pub measurement_probe_seq: u64,
    pub received_measurement_probes: VecDeque<ReceivedMeasurementProbe>,
    pub baseline_delay_sum: f64,
    pub baseline_delay_count: u64,
    pub last_feedback: common::ProbeFeedback,
    pub server_reported_c2s_stats: Option<common::DirectionStats>,
    pub calculated_s2c_stats: Option<common::DirectionStats>,
}

#[derive(Clone, Debug)]
pub struct ReceivedProbe {
    pub seq: u64,
    pub sent_at_ms: u64,
    pub received_at_ms: u64,
}

#[derive(Clone, Debug)]
pub struct ReceivedBulk {
    pub bytes: u64,
    pub received_at_ms: u64,
}

/// Received measurement probe for probe stream measurements
#[derive(Clone, Debug)]
pub struct ReceivedMeasurementProbe {
    pub seq: u64,
    pub sent_at_ms: u64,
    pub received_at_ms: u64,
    pub feedback: common::ProbeFeedback,
}

impl MeasurementState {
    pub fn new() -> Self {
        Self::with_conn_id(String::new())
    }
    
    pub fn with_conn_id(conn_id: String) -> Self {
        Self {
            test_count: 0,
            test_debug: "".into(),
            probe_seq: 0,
            conn_id,
            metrics: ClientMetrics::default(),
            received_probes: VecDeque::new(),
            received_bulk_bytes: VecDeque::new(),
            traceroute_started: 0,
            traceroute_done: 0,
            mtu_traceroute_started: 0,
            mtu_traceroute_done: 0,
            traceroute_active: false,
            server_side_ready: false,
            measuring_time_ms: None,
            path_ttl: None,
            // Probe stream fields
            probe_streams_active: false,
            measurement_probe_seq: 0,
            received_measurement_probes: VecDeque::new(),
            baseline_delay_sum: 0.0,
            baseline_delay_count: 0,
            last_feedback: common::ProbeFeedback::default(),
            server_reported_c2s_stats: None,
            calculated_s2c_stats: None,
        }
    }

    pub fn clear_metrics(&mut self) {
        // Clear accumulated measurement data
        self.received_probes.clear();
        self.received_bulk_bytes.clear();
        // Reset metrics to default values
        self.metrics = ClientMetrics::default();
        log::info!("Cleared metrics for conn_id: {}", self.conn_id);
    }

    pub fn set_traceroute_active(&mut self, active: bool) {
        self.traceroute_active = active;
        log::info!("Set traceroute_active = {} for conn_id: {}", active, self.conn_id);
    }

    pub fn calculate_metrics(&mut self) {
        let now_ms = current_time_ms();

        // Calculate for each time window: 1s, 10s, 60s
        let windows = [1_000u64, 10_000, 60_000];

        for (i, &window_ms) in windows.iter().enumerate() {
            let cutoff = now_ms.saturating_sub(window_ms);

            // Server-to-client metrics (from received probes)
            let recent_probes: Vec<_> = self.received_probes.iter()
                .filter(|p| p.received_at_ms >= cutoff)
                .cloned()
                .collect();

            if !recent_probes.is_empty() {
                // Calculate delay
                let delays: Vec<f64> = recent_probes.iter()
                    .map(|p| (p.received_at_ms as i64 - p.sent_at_ms as i64).abs() as f64)
                    .collect();

                let avg_delay = delays.iter().sum::<f64>() / delays.len() as f64;
                self.metrics.s2c_delay_avg[i] = avg_delay;

                // Calculate jitter (std dev of delay)
                let variance = delays.iter()
                    .map(|d| (d - avg_delay).powi(2))
                    .sum::<f64>() / delays.len() as f64;
                self.metrics.s2c_jitter[i] = variance.sqrt();

                // Calculate loss rate
                if recent_probes.len() >= 2 {
                    let min_seq = recent_probes.iter().map(|p| p.seq).min().unwrap();
                    let max_seq = recent_probes.iter().map(|p| p.seq).max().unwrap();
                    let expected = (max_seq - min_seq + 1) as f64;
                    let received = recent_probes.len() as f64;
                    self.metrics.s2c_loss_rate[i] = ((expected - received) / expected * 100.0).max(0.0);
                }

                // Calculate reordering rate
                let mut reorders = 0;
                let mut last_seq = 0u64;
                for p in &recent_probes {
                    if p.seq < last_seq {
                        reorders += 1;
                    }
                    last_seq = p.seq;
                }
                self.metrics.s2c_reorder_rate[i] = (reorders as f64 / recent_probes.len() as f64) * 100.0;
            }

            // Server-to-client throughput (from received bulk)
            let recent_bulk: Vec<_> = self.received_bulk_bytes.iter()
                .filter(|b| b.received_at_ms >= cutoff)
                .cloned()
                .collect();

            if !recent_bulk.is_empty() {
                let total_bytes: u64 = recent_bulk.iter().map(|b| b.bytes).sum();
                let time_window_sec = window_ms as f64 / 1000.0;
                self.metrics.s2c_throughput[i] = total_bytes as f64 / time_window_sec;
            }
        }
    }
}

pub fn setup_probe_channel(
    channel: RtcDataChannel,
    state: Rc<RefCell<MeasurementState>>,
) {
    let state_sender = state.clone();
    let channel_clone = channel.clone();

    let onopen = Closure::wrap(Box::new(move || {
        log::info!("Probe channel opened - no probe sending for now");
        return;

        // Start sending probes every 
        let state = state_sender.clone();
        let channel = channel_clone.clone();
        
        let interval = gloo_timers::callback::Interval::new(50, move || {
            let mut state = state.borrow_mut();
            let probe = ProbePacket {
                seq: state.probe_seq,
                timestamp_ms: current_time_ms(),
                direction: Direction::ClientToServer,
                send_options: None,  // Client doesn't send options yet
                conn_id: state.conn_id.clone(),
            };
            state.probe_seq += 1;

            if let Ok(json) = serde_json::to_string(&probe) {
                if let Err(e) = channel.send_with_str(&json) {
                    log::error!("Failed to send probe: {:?}", e);
                }
            }
        });

        // Keep interval alive
        interval.forget();
    }) as Box<dyn FnMut()>);

    channel.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    onopen.forget();

    // Handle incoming probes from server
    let state_receiver = state.clone();
    let channel_for_echo = channel.clone();
    let onmessage = Closure::wrap(Box::new(move |ev: MessageEvent| {
           let val = js_sys::JSON::stringify(&ev);
           let array = Uint8Array::new(&ev.data());
           let a_vec = array.to_vec();
           let s = String::from_utf8_lossy(&a_vec);
        {
           let mut state = state_receiver.borrow_mut();
           state.test_count += 1;
           // state.test_debug = format!("Evt: {:?}", &s);
        }
        //if let Some(txt) = ev.data().as_string() {
        if true {
            let txt = s.clone();
            
            // Try to parse as MeasurementProbePacket first if probe streams are active
            {
                let probe_streams_active = state_receiver.borrow().probe_streams_active;
                if probe_streams_active {
                    if let Ok(probe) = serde_json::from_str::<common::MeasurementProbePacket>(&txt) {
                        let now_ms = current_time_ms();
                        let delay = now_ms.saturating_sub(probe.sent_at_ms) as f64;
                        
                        let mut state = state_receiver.borrow_mut();
                        
                        // Store received probe
                        state.received_measurement_probes.push_back(ReceivedMeasurementProbe {
                            seq: probe.seq,
                            sent_at_ms: probe.sent_at_ms,
                            received_at_ms: now_ms,
                            feedback: probe.feedback.clone(),
                        });
                        
                        // Update baseline delay (exponential moving average with outlier exclusion)
                        let baseline = if state.baseline_delay_count > 0 {
                            state.baseline_delay_sum / state.baseline_delay_count as f64
                        } else {
                            delay
                        };
                        
                        if state.baseline_delay_count < common::BASELINE_MIN_SAMPLES || 
                           delay < baseline * common::BASELINE_OUTLIER_MULTIPLIER {
                            state.baseline_delay_sum += delay;
                            state.baseline_delay_count += 1;
                        }
                        
                        // Update feedback for outgoing probes
                        state.last_feedback.highest_seq = state.last_feedback.highest_seq.max(probe.seq);
                        state.last_feedback.highest_seq_received_at_ms = now_ms;
                        
                        // Keep only last PROBE_STATS_WINDOW_MS of probes for stats calculation
                        let cutoff = now_ms.saturating_sub(common::PROBE_STATS_WINDOW_MS);
                        while let Some(p) = state.received_measurement_probes.front() {
                            if p.received_at_ms < cutoff {
                                state.received_measurement_probes.pop_front();
                            } else {
                                break;
                            }
                        }
                        
                        // Count recent probes and reorders for feedback
                        let mut recent_count = 0u32;
                        let mut recent_reorders = 0u32;
                        let mut last_seq = 0u64;
                        let feedback_cutoff = now_ms.saturating_sub(common::PROBE_FEEDBACK_WINDOW_MS);
                        for p in state.received_measurement_probes.iter() {
                            if p.received_at_ms >= feedback_cutoff {
                                recent_count += 1;
                                if p.seq < last_seq {
                                    recent_reorders += 1;
                                }
                                last_seq = last_seq.max(p.seq);
                            }
                        }
                        state.last_feedback.recent_count = recent_count;
                        state.last_feedback.recent_reorders = recent_reorders;
                        
                        return;  // Handled as measurement probe
                    }
                }
            }
            
            // Fall back to regular probe handling
            if let Ok(mut probe) = serde_json::from_str::<ProbePacket>(&txt) {
                let now_ms = current_time_ms();
                let mut state = state_receiver.borrow_mut();

                // Skip probe data collection during traceroute phase
                if !state.traceroute_active {
                    state.received_probes.push_back(ReceivedProbe {
                        seq: probe.seq,
                        sent_at_ms: probe.timestamp_ms,
                        received_at_ms: now_ms,
                    });

                    // Keep only last 60 seconds of probes
                    let cutoff = now_ms.saturating_sub(60_000);
                    while let Some(p) = state.received_probes.front() {
                        if p.received_at_ms < cutoff {
                            state.received_probes.pop_front();
                        } else {
                            break;
                        }
                    }
                } else {
                    log::trace!("Skipping probe data collection during traceroute phase (seq: {})", probe.seq);
                }

                // Echo probe back to server with received timestamp
                probe.timestamp_ms = now_ms;
                if let Ok(json) = serde_json::to_string(&probe) {
                    if let Err(e) = channel_for_echo.send_with_str(&json) {
                        log::error!("Failed to echo probe back: {:?}", e);
                    }
                }
            }
        }
    }) as Box<dyn FnMut(MessageEvent)>);

    channel.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();
}

pub fn setup_bulk_channel(
    channel: RtcDataChannel,
    state: Rc<RefCell<MeasurementState>>,
) {
    let channel_clone = channel.clone();

    let state_sender = state.clone();
    let onopen = Closure::wrap(Box::new(move || {
        log::info!("Bulk channel opened");

        // Start sending bulk data every 10ms
        let channel = channel_clone.clone();
        let state_sender = state_sender.clone();
        
        let interval = gloo_timers::callback::Interval::new(10, move || {
            if state_sender.borrow().traceroute_active {
                /* do not send bulk data while traceroute is active */
                return;
            }
            let bulk = BulkPacket::new(1024);
            if let Ok(json) = serde_json::to_string(&bulk) {
                if let Err(e) = channel.send_with_str(&json) {
                    log::error!("Failed to send bulk: {:?}", e);
                }
            }
        });

        interval.forget();
    }) as Box<dyn FnMut()>);

    channel.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    onopen.forget();

    // Handle incoming bulk from server
    let state_receiver = state.clone();
    let onmessage = Closure::wrap(Box::new(move |ev: MessageEvent| {
        let now_ms = current_time_ms();
        let bytes = if let Some(txt) = ev.data().as_string() {
            txt.len() as u64
        } else if let Ok(blob) = ev.data().dyn_into::<web_sys::Blob>() {
            blob.size() as u64
        } else {
            0
        };

        if bytes > 0 {
            let mut state = state_receiver.borrow_mut();
            
            // Skip bulk data collection during traceroute phase
            if !state.traceroute_active {
                state.received_bulk_bytes.push_back(ReceivedBulk {
                    bytes,
                    received_at_ms: now_ms,
                });

                // Keep only last 60 seconds of bulk data
                let cutoff = now_ms.saturating_sub(60_000);
                while let Some(b) = state.received_bulk_bytes.front() {
                    if b.received_at_ms < cutoff {
                        state.received_bulk_bytes.pop_front();
                    } else {
                        break;
                    }
                }
            } else {
                log::trace!("Skipping bulk data collection during traceroute phase ({} bytes)", bytes);
            }
        }
    }) as Box<dyn FnMut(MessageEvent)>);

    channel.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();
}

pub fn setup_control_channel(channel: RtcDataChannel, state: Rc<RefCell<MeasurementState>>) {
    let onopen = Closure::wrap(Box::new(move || {
        log::info!("Control channel opened");
    }) as Box<dyn FnMut()>);

    channel.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    onopen.forget();

    // Handle incoming messages from server (e.g., traceroute hop information)
    let state_for_handler = state.clone();
    let onmessage = Closure::wrap(Box::new(move |ev: MessageEvent| {
        let array = Uint8Array::new(&ev.data());
        let data = array.to_vec();
        let text = String::from_utf8_lossy(&data);

        // Try to parse as ControlMessage enum
        match serde_json::from_str::<common::ControlMessage>(&text) {
            Ok(control_msg) => {
                match control_msg {
                    common::ControlMessage::ServerSideReady(ready_msg) => {
                        let expected_conn_id = state_for_handler.borrow().conn_id.clone();
                        if !expected_conn_id.is_empty() && ready_msg.conn_id != expected_conn_id {
                            log::warn!(
                                "ServerSideReadyMessage conn_id mismatch: received '{}' but expected '{}', ignoring",
                                ready_msg.conn_id, expected_conn_id
                            );
                            return;
                        }
                        
                        log::info!("Received ServerSideReady for conn_id: {}", ready_msg.conn_id);
                        state_for_handler.borrow_mut().server_side_ready = true;
                    }
                    
                    common::ControlMessage::MeasuringTimeResponse(time_msg) => {
                        let expected_conn_id = state_for_handler.borrow().conn_id.clone();
                        if !expected_conn_id.is_empty() && time_msg.conn_id != expected_conn_id {
                            log::warn!(
                                "MeasuringTimeResponseMessage conn_id mismatch: received '{}' but expected '{}', ignoring",
                                time_msg.conn_id, expected_conn_id
                            );
                            return;
                        }
                        
                        log::info!("Received MeasuringTimeResponse: {}ms for conn_id: {}", time_msg.max_duration_ms, time_msg.conn_id);
                        state_for_handler.borrow_mut().measuring_time_ms = Some(time_msg.max_duration_ms);
                    }
                    common::ControlMessage::TracerouteCompleted(traceroute_completed_msg) => {
                        let expected_conn_id = state_for_handler.borrow().conn_id.clone();
                        if !expected_conn_id.is_empty() && traceroute_completed_msg.conn_id != expected_conn_id {
                            log::warn!(
                                "TracerouteCompleted conn_id mismatch: received '{}' but expected '{}', ignoring",
                                traceroute_completed_msg.conn_id, expected_conn_id
                            );
                            return;
                        }
                        state_for_handler.borrow_mut().traceroute_done += 1;
                    }
                    common::ControlMessage::MtuTracerouteCompleted(mtu_traceroute_completed_msg) => {
                        let expected_conn_id = state_for_handler.borrow().conn_id.clone();
                        if !expected_conn_id.is_empty() && mtu_traceroute_completed_msg.conn_id != expected_conn_id {
                            log::warn!(
                                "MtuTracerouteCompleted conn_id mismatch: received '{}' but expected '{}', ignoring",
                                mtu_traceroute_completed_msg.conn_id, expected_conn_id
                            );
                            return;
                        }
                        state_for_handler.borrow_mut().mtu_traceroute_done += 1;
                    }
                    
                    common::ControlMessage::MtuHop(mtu_msg) => {
                        let expected_conn_id = state_for_handler.borrow().conn_id.clone();
                        if !expected_conn_id.is_empty() && mtu_msg.conn_id != expected_conn_id {
                            log::warn!(
                                "MtuHopMessage conn_id mismatch: received '{}' but expected '{}', ignoring",
                                mtu_msg.conn_id, expected_conn_id
                            );
                            return;
                        }
                        
                        // Pass structured data to visualization function
                        update_mtu_visualization(&mtu_msg);
                        
                        // Display the MTU hop message
                        let conn_prefix = if mtu_msg.conn_id.len() >= 8 {
                            &mtu_msg.conn_id[..8]
                        } else {
                            &mtu_msg.conn_id
                        };
                        let mtu_str = mtu_msg.mtu.map(|m| format!(" MTU:{}", m)).unwrap_or_default();
                        let ip_str = mtu_msg.ip_address.as_ref().map(|ip| format!(" from {}", ip)).unwrap_or_default();
                        append_server_message(&format!(
                            "[{}][MTU Hop {}] size={} RTT:{:.2}ms{}{}",
                            conn_prefix,
                            mtu_msg.hop,
                            mtu_msg.packet_size,
                            mtu_msg.rtt_ms,
                            mtu_str,
                            ip_str
                        ));
                    }
                    
                    common::ControlMessage::TraceHop(hop_msg) => {
                        // Validate conn_id matches this connection
                        let expected_conn_id = state_for_handler.borrow().conn_id.clone();
                        if !expected_conn_id.is_empty() && hop_msg.conn_id != expected_conn_id {
                            log::warn!(
                                "TraceHopMessage conn_id mismatch: received '{}' but expected '{}', ignoring",
                                hop_msg.conn_id, expected_conn_id
                            );
                            return;
                        }
                        
                        // Pass structured data to visualization function
                        update_traceroute_visualization(&hop_msg);
                        
                        // Display the hop message in the UI with short conn_id prefix for multi-connection differentiation
                        let conn_prefix = if hop_msg.conn_id.len() >= 8 {
                            &hop_msg.conn_id[..8]
                        } else {
                            &hop_msg.conn_id
                        };
                        append_server_message(&format!(
                            "[{}][Hop {}] {} (RTT: {:.2}ms)",
                            conn_prefix,
                            hop_msg.hop,
                            hop_msg.message,
                            hop_msg.rtt_ms
                        ));
                    }
                    
                    common::ControlMessage::ProbeStats(stats_msg) => {
                        // Server is reporting its calculated stats (C2S) and our previously sent S2C stats
                        let expected_conn_id = state_for_handler.borrow().conn_id.clone();
                        if !expected_conn_id.is_empty() && stats_msg.conn_id != expected_conn_id {
                            log::warn!(
                                "ProbeStatsReport conn_id mismatch: received '{}' but expected '{}', ignoring",
                                stats_msg.conn_id, expected_conn_id
                            );
                            return;
                        }
                        
                        log::debug!("Received ProbeStats from server: c2s_loss={:.2}%, s2c_loss={:.2}%",
                            stats_msg.c2s_stats.loss_rate, stats_msg.s2c_stats.loss_rate);
                        
                        // Store server-reported C2S stats for visualization
                        state_for_handler.borrow_mut().server_reported_c2s_stats = Some(stats_msg.c2s_stats.clone());
                        
                        // Update the visualization with the stats
                        update_probe_stats_visualization(&stats_msg);
                    }
                    
                    // Client-to-server messages (should not be received here)
                    x => {
                        log::warn!("Received unexpected client-to-server control message type: {:?}", &x);
                    }
                }
            }
            Err(e) => {
                // Display as plain text if not a recognized message type
                log::debug!("Received non-control message (parse error: {}): {}", e, text);
                append_server_message(&format!("Server: {}", text));
            }
        }
    }) as Box<dyn FnMut(MessageEvent)>);

    channel.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();
}

/// Append a message to the server messages text area
fn append_server_message(message: &str) {
    use wasm_bindgen::JsCast;
    use web_sys::{window, HtmlTextAreaElement};

    if let Some(window) = window() {
        if let Some(document) = window.document() {
            if let Some(textarea) = document.get_element_by_id("server-messages") {
                if let Ok(textarea) = textarea.dyn_into::<HtmlTextAreaElement>() {
                    let current = textarea.value();
                    let new_value = if current.is_empty() {
                        message.to_string()
                    } else {
                        format!("{}\n{}", current, message)
                    };
                    textarea.set_value(&new_value);
                    
                    // Auto-scroll to bottom
                    textarea.set_scroll_top(textarea.scroll_height());
                }
            }
        }
    }
}

/// Update the traceroute visualization with hop data
fn update_traceroute_visualization(hop_msg: &common::TraceHopMessage) {
    use wasm_bindgen::JsValue;
    use web_sys::window;
    
    if let Some(window) = window() {
        // Create a JavaScript object with the hop data
        let js_obj = js_sys::Object::new();
        
        // Set properties
        js_sys::Reflect::set(&js_obj, &JsValue::from_str("hop"), &JsValue::from_f64(hop_msg.hop as f64)).ok();
        js_sys::Reflect::set(&js_obj, &JsValue::from_str("conn_id"), &JsValue::from_str(&hop_msg.conn_id)).ok();
        js_sys::Reflect::set(&js_obj, &JsValue::from_str("rtt_ms"), &JsValue::from_f64(hop_msg.rtt_ms)).ok();
        js_sys::Reflect::set(&js_obj, &JsValue::from_str("message"), &JsValue::from_str(&hop_msg.message)).ok();
        
        if let Some(ref ip) = hop_msg.ip_address {
            js_sys::Reflect::set(&js_obj, &JsValue::from_str("ip_address"), &JsValue::from_str(ip)).ok();
        } else {
            js_sys::Reflect::set(&js_obj, &JsValue::from_str("ip_address"), &JsValue::NULL).ok();
        }
        
        // Include original UDP address/port info for cross-checking
        js_sys::Reflect::set(&js_obj, &JsValue::from_str("original_src_port"), &JsValue::from_f64(hop_msg.original_src_port as f64)).ok();
        js_sys::Reflect::set(&js_obj, &JsValue::from_str("original_dest_addr"), &JsValue::from_str(&hop_msg.original_dest_addr)).ok();
        
        // Call the JavaScript function
        if let Ok(add_fn) = js_sys::Reflect::get(&window, &JsValue::from_str("addTracerouteHop")) {
            if let Ok(add_fn) = add_fn.dyn_into::<js_sys::Function>() {
                if let Err(e) = add_fn.call1(&JsValue::NULL, &js_obj) {
                    log::warn!("Failed to call addTracerouteHop JavaScript function: {:?}", e);
                }
            } else {
                log::warn!("addTracerouteHop is not a function");
            }
        } else {
            log::warn!("addTracerouteHop function not found in window object");
        }
    }
}

/// Update the MTU visualization with MTU hop data
fn update_mtu_visualization(mtu_msg: &common::MtuHopMessage) {
    use wasm_bindgen::JsValue;
    use web_sys::window;
    
    if let Some(window) = window() {
        // Create a JavaScript object with the MTU hop data
        let js_obj = js_sys::Object::new();
        
        // Set properties
        js_sys::Reflect::set(&js_obj, &JsValue::from_str("hop"), &JsValue::from_f64(mtu_msg.hop as f64)).ok();
        js_sys::Reflect::set(&js_obj, &JsValue::from_str("conn_id"), &JsValue::from_str(&mtu_msg.conn_id)).ok();
        js_sys::Reflect::set(&js_obj, &JsValue::from_str("rtt_ms"), &JsValue::from_f64(mtu_msg.rtt_ms)).ok();
        js_sys::Reflect::set(&js_obj, &JsValue::from_str("message"), &JsValue::from_str(&mtu_msg.message)).ok();
        js_sys::Reflect::set(&js_obj, &JsValue::from_str("packet_size"), &JsValue::from_f64(mtu_msg.packet_size as f64)).ok();
        
        if let Some(ref ip) = mtu_msg.ip_address {
            js_sys::Reflect::set(&js_obj, &JsValue::from_str("ip_address"), &JsValue::from_str(ip)).ok();
        } else {
            js_sys::Reflect::set(&js_obj, &JsValue::from_str("ip_address"), &JsValue::NULL).ok();
        }
        
        if let Some(mtu) = mtu_msg.mtu {
            js_sys::Reflect::set(&js_obj, &JsValue::from_str("mtu"), &JsValue::from_f64(mtu as f64)).ok();
        } else {
            js_sys::Reflect::set(&js_obj, &JsValue::from_str("mtu"), &JsValue::NULL).ok();
        }
        
        // Call the JavaScript function
        if let Ok(add_fn) = js_sys::Reflect::get(&window, &JsValue::from_str("addMtuHop")) {
            if let Ok(add_fn) = add_fn.dyn_into::<js_sys::Function>() {
                if let Err(e) = add_fn.call1(&JsValue::NULL, &js_obj) {
                    log::warn!("Failed to call addMtuHop JavaScript function: {:?}", e);
                }
            } else {
                log::warn!("addMtuHop is not a function");
            }
        } else {
            log::warn!("addMtuHop function not found in window object");
        }
    }
}

/// Update the probe stats visualization
fn update_probe_stats_visualization(stats_msg: &common::ProbeStatsReport) {
    use wasm_bindgen::JsValue;
    use web_sys::window;
    
    if let Some(window) = window() {
        // Create a JavaScript object with the stats data
        let js_obj = js_sys::Object::new();
        
        // Set connection info
        js_sys::Reflect::set(&js_obj, &JsValue::from_str("conn_id"), &JsValue::from_str(&stats_msg.conn_id)).ok();
        js_sys::Reflect::set(&js_obj, &JsValue::from_str("timestamp_ms"), &JsValue::from_f64(stats_msg.timestamp_ms as f64)).ok();
        
        // Helper to create stats object
        let create_stats_obj = |stats: &common::DirectionStats| -> js_sys::Object {
            let obj = js_sys::Object::new();
            
            // Delay deviation array
            let delay_arr = js_sys::Array::new();
            for &v in &stats.delay_deviation_ms {
                delay_arr.push(&JsValue::from_f64(v));
            }
            js_sys::Reflect::set(&obj, &JsValue::from_str("delay_deviation_ms"), &delay_arr).ok();
            
            // RTT array
            let rtt_arr = js_sys::Array::new();
            for &v in &stats.rtt_ms {
                rtt_arr.push(&JsValue::from_f64(v));
            }
            js_sys::Reflect::set(&obj, &JsValue::from_str("rtt_ms"), &rtt_arr).ok();
            
            // Jitter array
            let jitter_arr = js_sys::Array::new();
            for &v in &stats.jitter_ms {
                jitter_arr.push(&JsValue::from_f64(v));
            }
            js_sys::Reflect::set(&obj, &JsValue::from_str("jitter_ms"), &jitter_arr).ok();
            
            // Scalar values
            js_sys::Reflect::set(&obj, &JsValue::from_str("loss_rate"), &JsValue::from_f64(stats.loss_rate)).ok();
            js_sys::Reflect::set(&obj, &JsValue::from_str("reorder_rate"), &JsValue::from_f64(stats.reorder_rate)).ok();
            js_sys::Reflect::set(&obj, &JsValue::from_str("probe_count"), &JsValue::from_f64(stats.probe_count as f64)).ok();
            js_sys::Reflect::set(&obj, &JsValue::from_str("baseline_delay_ms"), &JsValue::from_f64(stats.baseline_delay_ms)).ok();
            
            obj
        };
        
        let c2s_obj = create_stats_obj(&stats_msg.c2s_stats);
        let s2c_obj = create_stats_obj(&stats_msg.s2c_stats);
        
        js_sys::Reflect::set(&js_obj, &JsValue::from_str("c2s_stats"), &c2s_obj).ok();
        js_sys::Reflect::set(&js_obj, &JsValue::from_str("s2c_stats"), &s2c_obj).ok();
        
        // Call the JavaScript function
        if let Ok(update_fn) = js_sys::Reflect::get(&window, &JsValue::from_str("updateProbeStats")) {
            if let Ok(update_fn) = update_fn.dyn_into::<js_sys::Function>() {
                if let Err(e) = update_fn.call1(&JsValue::NULL, &js_obj) {
                    log::warn!("Failed to call updateProbeStats JavaScript function: {:?}", e);
                }
            }
        }
    }
}

pub fn current_time_ms() -> u64 {
    js_sys::Date::now() as u64
}

pub fn setup_testprobe_channel(channel: RtcDataChannel, control: Rc<RefCell<RtcDataChannel>>, state: Rc<RefCell<MeasurementState>>) {
    let onopen = Closure::wrap(Box::new(move || {
        log::info!("TestProbe channel opened");
    }) as Box<dyn FnMut()>);

    channel.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    onopen.forget();

    // Handle incoming test probes from server - echo them back
    let channel_for_echo = channel.clone();
    let state_for_handler = state.clone();
    let onmessage = Closure::wrap(Box::new(move |ev: MessageEvent| {
        use common::ControlMessage::TestProbeMessageEcho;
        let array = Uint8Array::new(&ev.data());
        let data = array.to_vec();
        let text = String::from_utf8_lossy(&data);

        // Try to parse as TestProbePacket
        if let Ok(mut testprobe) = serde_json::from_str::<common::TestProbePacket>(&text) {
            // Validate conn_id matches this connection
            let expected_conn_id = state_for_handler.borrow().conn_id.clone();
            if !expected_conn_id.is_empty() && testprobe.conn_id != expected_conn_id {
                log::warn!(
                    "TestProbePacket conn_id mismatch: received '{}' but expected '{}', ignoring",
                    testprobe.conn_id, expected_conn_id
                );
                return;
            }
            
            let now_ms = current_time_ms();
            let ttl: i32 = testprobe.send_options.map(|so| so.ttl.map(|t| t as i32).unwrap_or(-1)).unwrap_or(-2);
            
            log::debug!("conn {:?}: Received test probe test_seq {} ttl {} from server, echoing back on control channel", &testprobe.conn_id, testprobe.test_seq, ttl);

            if state_for_handler.borrow().path_ttl.is_none() {
                state_for_handler.borrow_mut().path_ttl = Some(ttl);
            }
            
            // Echo test probe back to server with received timestamp
            testprobe.timestamp_ms = now_ms;
            let testprobe = TestProbeMessageEcho(testprobe);
            if let Ok(json) = serde_json::to_string(&testprobe) {
                if let Err(e) = control.borrow_mut().send_with_str(&json) {
                    log::error!("Failed to echo test probe back on control channel: {:?}", e);
                }
            }
        }
    }) as Box<dyn FnMut(MessageEvent)>);

    channel.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();
}
