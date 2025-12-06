use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response, window};

#[derive(Serialize)]
struct SignalingStartRequest {
    sdp: String,
}

#[derive(Deserialize)]
struct SignalingStartResponse {
    client_id: String,
    sdp: String,
}

pub async fn send_offer(offer_sdp: String) -> Result<(String, String), JsValue> {
    let window = window().ok_or("No window")?;

    let req_body = SignalingStartRequest { sdp: offer_sdp };
    let body_str = serde_json::to_string(&req_body)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    let mut opts = RequestInit::new();
    opts.set_method("POST");
    opts.set_mode(RequestMode::Cors);
    opts.set_body(&JsValue::from_str(&body_str));

    let url = format!("{}/api/signaling/start",
                     window.location().origin().map_err(|_| JsValue::from_str("No origin"))?);

    let request = Request::new_with_str_and_init(&url, &opts)?;
    request.headers().set("Content-Type", "application/json")?;

    let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;
    let resp: Response = resp_value.dyn_into()?;

    let json = JsFuture::from(resp.json()?).await?;
    let response: SignalingStartResponse = serde_wasm_bindgen::from_value(json)?;

    Ok((response.client_id, response.sdp))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signaling_request_serialization() {
        let req = SignalingStartRequest {
            sdp: "test-sdp".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("test-sdp"));
    }
}
