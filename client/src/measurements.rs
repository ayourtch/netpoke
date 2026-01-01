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
    pub traceroute_active: bool,
    pub server_side_ready: bool,
    pub measuring_time_ms: Option<u64>,
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
            traceroute_active: false,
            server_side_ready: false,
            measuring_time_ms: None,
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
        log::info!("Probe channel opened");

        // Start sending probes every 50ms
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
                    
                    common::ControlMessage::MtuHop(mtu_msg) => {
                        let expected_conn_id = state_for_handler.borrow().conn_id.clone();
                        if !expected_conn_id.is_empty() && mtu_msg.conn_id != expected_conn_id {
                            log::warn!(
                                "MtuHopMessage conn_id mismatch: received '{}' but expected '{}', ignoring",
                                mtu_msg.conn_id, expected_conn_id
                            );
                            return;
                        }
                        
                        // Display the MTU hop message
                        let conn_prefix = if mtu_msg.conn_id.len() >= 8 {
                            &mtu_msg.conn_id[..8]
                        } else {
                            &mtu_msg.conn_id
                        };
                        let mtu_str = mtu_msg.mtu.map(|m| format!(" MTU:{}", m)).unwrap_or_default();
                        append_server_message(&format!(
                            "[{}][MTU Hop {}] size={} RTT:{:.2}ms{}",
                            conn_prefix,
                            mtu_msg.hop,
                            mtu_msg.packet_size,
                            mtu_msg.rtt_ms,
                            mtu_str
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
                    
                    // Client-to-server messages (should not be received here)
                    _ => {
                        log::warn!("Received unexpected client-to-server control message type");
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
                let _ = add_fn.call1(&JsValue::NULL, &js_obj);
            }
        }
    }
}

pub fn current_time_ms() -> u64 {
    js_sys::Date::now() as u64
}

pub fn setup_testprobe_channel(channel: RtcDataChannel, state: Rc<RefCell<MeasurementState>>) {
    let onopen = Closure::wrap(Box::new(move || {
        log::info!("TestProbe channel opened");
    }) as Box<dyn FnMut()>);

    channel.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    onopen.forget();

    // Handle incoming test probes from server - echo them back
    let channel_for_echo = channel.clone();
    let state_for_handler = state.clone();
    let onmessage = Closure::wrap(Box::new(move |ev: MessageEvent| {
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
            
            log::debug!("conn {:?}: Received test probe test_seq {} ttl {} from server, echoing back", &testprobe.conn_id, testprobe.test_seq, ttl);
            
            // Echo test probe back to server with received timestamp
            testprobe.timestamp_ms = now_ms;
            if let Ok(json) = serde_json::to_string(&testprobe) {
                if let Err(e) = channel_for_echo.send_with_str(&json) {
                    log::error!("Failed to echo test probe back: {:?}", e);
                }
            }
        }
    }) as Box<dyn FnMut(MessageEvent)>);

    channel.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();
}
