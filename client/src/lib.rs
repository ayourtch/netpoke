mod webrtc;
mod signaling;
mod measurements;

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
    log::info!("Starting network measurement...");

    let connection = webrtc::WebRtcConnection::new().await?;
    log::info!("Connected with client_id: {}", connection.client_id);

    // Start UI update loop
    let state = connection.state.clone();
    gloo_timers::callback::Interval::new(100, move || {
        // Calculate metrics
        {
            let mut state_ref = state.borrow_mut();
            state_ref.calculate_metrics();
        }
        
        // Update UI
        let state_ref = state.borrow();
        update_ui(&state_ref.metrics);
    }).forget();

    // Keep connection alive
    std::mem::forget(connection);

    Ok(())
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
