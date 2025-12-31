/// API handlers for client configuration
///
/// This endpoint exposes configuration settings that the WASM client
/// needs to know about, such as the delay between WebRTC connection attempts.

use axum::{extract::State, Json};
use serde::Serialize;
use std::sync::Arc;

/// Client configuration that is exposed to the WASM client
#[derive(Debug, Clone, Serialize)]
pub struct ClientConfigResponse {
    /// Delay in milliseconds between WebRTC connection establishment attempts
    pub webrtc_connection_delay_ms: u32,
}

/// Shared state for client config API
#[derive(Clone)]
pub struct ClientConfigState {
    pub webrtc_connection_delay_ms: u32,
}

/// Get client configuration
///
/// Returns configuration settings that the WASM client needs
pub async fn get_client_config(
    State(config_state): State<Arc<ClientConfigState>>,
) -> Json<ClientConfigResponse> {
    Json(ClientConfigResponse {
        webrtc_connection_delay_ms: config_state.webrtc_connection_delay_ms,
    })
}
