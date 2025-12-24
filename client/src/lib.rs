mod webrtc;
mod signaling;
mod measurements;
use crate::measurements::current_time_ms;

use wasm_bindgen::prelude::*;
use web_sys::{window, Document};

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::default());

    log::info!("WASM client initialized");
}

#[wasm_bindgen]
pub async fn start_measurement() -> Result<(), JsValue> {
    log::info!("Starting dual-stack network measurement...");

    // Create IPv4 connection (first connection, no parent)
    let ipv4_connection = webrtc::WebRtcConnection::new_with_ip_version("ipv4", None).await?;
    let parent_id = Some(ipv4_connection.client_id.clone());
    log::info!("IPv4 connected with client_id: {}", ipv4_connection.client_id);

    // Create IPv6 connection (second connection, with parent from IPv4)
    let ipv6_connection = webrtc::WebRtcConnection::new_with_ip_version("ipv6", parent_id).await?;
    log::info!("IPv6 connected with client_id: {}", ipv6_connection.client_id);

    // Start UI update loop
    let state_ipv4 = ipv4_connection.state.clone();
    let state_ipv6 = ipv6_connection.state.clone();

    gloo_timers::callback::Interval::new(100, move || {
        // Calculate metrics for both connections
        {
            let mut state_ref = state_ipv4.borrow_mut();
            state_ref.calculate_metrics();
        }
        {
            let mut state_ref = state_ipv6.borrow_mut();
            state_ref.calculate_metrics();
        }

        // Update UI with both sets of metrics
        let state_ipv4_ref = state_ipv4.borrow();
        let state_ipv6_ref = state_ipv6.borrow();

        let dbg_message = format!("{:?}", &state_ipv4_ref);
        update_ui_dual(&dbg_message, &state_ipv4_ref.metrics, &state_ipv6_ref.metrics);
    }).forget();

    // Keep connections alive
    std::mem::forget(ipv4_connection);
    std::mem::forget(ipv6_connection);

    Ok(())
}

fn update_ui_dual(dbg_message: &str, ipv4_metrics: &common::ClientMetrics, ipv6_metrics: &common::ClientMetrics) {
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
