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
        }
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

    let onopen = Closure::wrap(Box::new(move || {
        log::info!("Bulk channel opened");

        // Start sending bulk data every 10ms
        let channel = channel_clone.clone();
        
        let interval = gloo_timers::callback::Interval::new(10, move || {
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
        }
    }) as Box<dyn FnMut(MessageEvent)>);

    channel.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();
}

pub fn setup_control_channel(channel: RtcDataChannel) {
    let onopen = Closure::wrap(Box::new(move || {
        log::info!("Control channel opened");
    }) as Box<dyn FnMut()>);

    channel.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    onopen.forget();

    // Handle incoming messages from server (e.g., traceroute hop information)
    let onmessage = Closure::wrap(Box::new(move |ev: MessageEvent| {
        let array = Uint8Array::new(&ev.data());
        let data = array.to_vec();
        let text = String::from_utf8_lossy(&data);

        // Try to parse as TraceHopMessage
        if let Ok(hop_msg) = serde_json::from_str::<common::TraceHopMessage>(&text) {
            // Display the hop message in the UI
            append_server_message(&format!(
                "[Hop {}] {} (RTT: {:.2}ms)",
                hop_msg.hop,
                hop_msg.message,
                hop_msg.rtt_ms
            ));
        } else {
            // Display as plain text if not a recognized message type
            log::debug!("Received control message: {}", text);
            append_server_message(&format!("Server: {}", text));
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

pub fn current_time_ms() -> u64 {
    js_sys::Date::now() as u64
}

pub fn setup_testprobe_channel(channel: RtcDataChannel) {
    let onopen = Closure::wrap(Box::new(move || {
        log::info!("TestProbe channel opened");
    }) as Box<dyn FnMut()>);

    channel.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    onopen.forget();

    // Handle incoming test probes from server - echo them back
    let channel_for_echo = channel.clone();
    let onmessage = Closure::wrap(Box::new(move |ev: MessageEvent| {
        let array = Uint8Array::new(&ev.data());
        let data = array.to_vec();
        let text = String::from_utf8_lossy(&data);

        // Try to parse as TestProbePacket
        if let Ok(mut testprobe) = serde_json::from_str::<common::TestProbePacket>(&text) {
            let now_ms = current_time_ms();
            
            log::debug!("Received test probe test_seq {} from server, echoing back", testprobe.test_seq);
            
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
