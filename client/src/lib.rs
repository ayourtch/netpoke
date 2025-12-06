mod webrtc;
mod signaling;

use wasm_bindgen::prelude::*;

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

    // Connection is now established with data channels
    // Keep connection alive by forgetting it (not ideal, but works for now)
    std::mem::forget(connection);

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_basic() {
        assert_eq!(2 + 2, 4);
    }
}
