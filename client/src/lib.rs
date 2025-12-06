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

    // Will implement in next tasks
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_basic() {
        assert_eq!(2 + 2, 4);
    }
}
