# Network Measurement System Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a WebRTC-based browser network measurement system that continuously monitors throughput, delay, jitter, loss, and reordering.

**Architecture:** Rust server with WebRTC data channels for bidirectional measurement. WASM client in browser. Real-time dashboard via WebSocket.

**Tech Stack:** Rust (tokio, axum, webrtc), WASM (wasm-bindgen, web-sys), WebRTC data channels, WebSocket

---

## Task 1: Project Structure and Cargo Workspace

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `server/Cargo.toml`
- Create: `client/Cargo.toml`
- Create: `common/Cargo.toml`
- Create: `.gitignore`

**Step 1: Create workspace Cargo.toml**

```toml
[workspace]
members = ["server", "client", "common"]
resolver = "2"

[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.35", features = ["full"] }
```

Run: `cat Cargo.toml` to verify

**Step 2: Create server Cargo.toml**

```toml
[package]
name = "wifi-verify-server"
version = "0.1.0"
edition = "2021"

[dependencies]
common = { path = "../common" }
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
axum = "0.7"
tower = "0.4"
tower-http = { version = "0.5", features = ["fs", "trace"] }
webrtc = "0.9"
tokio-tungstenite = "0.21"
futures = "0.3"
tracing = "0.1"
tracing-subscriber = "0.3"
uuid = { version = "1.6", features = ["v4", "serde"] }
```

Run: `cat server/Cargo.toml` to verify

**Step 3: Create client Cargo.toml**

```toml
[package]
name = "wifi-verify-client"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
common = { path = "../common" }
serde.workspace = true
serde_json.workspace = true
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
web-sys = { version = "0.3", features = [
    "Window",
    "Document",
    "Element",
    "HtmlElement",
    "RtcPeerConnection",
    "RtcConfiguration",
    "RtcDataChannel",
    "RtcDataChannelInit",
    "RtcSdpType",
    "RtcSessionDescriptionInit",
    "RtcIceCandidate",
    "RtcIceCandidateInit",
    "MessageEvent",
] }
js-sys = "0.3"
gloo-timers = { version = "0.3", features = ["futures"] }
serde-wasm-bindgen = "0.6"
console_error_panic_hook = "0.1"
wasm-logger = "0.2"
```

Run: `cat client/Cargo.toml` to verify

**Step 4: Create common Cargo.toml**

```toml
[package]
name = "common"
version = "0.1.0"
edition = "2021"

[dependencies]
serde.workspace = true
serde_json.workspace = true
```

Run: `cat common/Cargo.toml` to verify

**Step 5: Create .gitignore**

```
/target/
/client/pkg/
/client/target/
/server/target/
/common/target/
*.wasm
*.js
node_modules/
.DS_Store
```

Run: `cat .gitignore` to verify

**Step 6: Create placeholder main files**

Create `server/src/main.rs`:
```rust
fn main() {
    println!("Server starting...");
}
```

Create `client/src/lib.rs`:
```rust
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
}
```

Create `common/src/lib.rs`:
```rust
// Common types shared between server and client
```

**Step 7: Verify workspace builds**

Run: `cargo build`
Expected: All three crates compile successfully

**Step 8: Commit**

```bash
git add .
git commit -m "feat: initialize Cargo workspace with server, client, and common crates"
```

---

## Task 2: Common Protocol Types

**Files:**
- Modify: `common/src/lib.rs`
- Create: `common/src/protocol.rs`
- Create: `common/src/metrics.rs`

**Step 1: Write test for ProbePacket serialization**

Create `common/src/protocol.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Direction {
    ClientToServer,
    ServerToClient,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProbePacket {
    pub seq: u64,
    pub timestamp_ms: u64,
    pub direction: Direction,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_probe_packet_serialization() {
        let packet = ProbePacket {
            seq: 42,
            timestamp_ms: 1234567890,
            direction: Direction::ClientToServer,
        };

        let json = serde_json::to_string(&packet).unwrap();
        let deserialized: ProbePacket = serde_json::from_str(&json).unwrap();

        assert_eq!(packet, deserialized);
    }
}
```

**Step 2: Run test to verify it passes**

Run: `cargo test -p common`
Expected: PASS

**Step 3: Write test for BulkPacket**

Add to `common/src/protocol.rs`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkPacket {
    pub data: Vec<u8>,
}

impl BulkPacket {
    pub fn new(size: usize) -> Self {
        Self {
            data: vec![0u8; size],
        }
    }
}

#[cfg(test)]
mod tests {
    // ... existing test ...

    #[test]
    fn test_bulk_packet_creation() {
        let packet = BulkPacket::new(1024);
        assert_eq!(packet.data.len(), 1024);
    }
}
```

**Step 4: Run test**

Run: `cargo test -p common`
Expected: PASS (2 tests)

**Step 5: Write test for ClientMetrics**

Create `common/src/metrics.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientMetrics {
    // Throughput in bytes/sec for [1s, 10s, 60s] windows
    pub c2s_throughput: [f64; 3],
    pub s2c_throughput: [f64; 3],

    // Delay in milliseconds
    pub c2s_delay_avg: [f64; 3],
    pub s2c_delay_avg: [f64; 3],

    // Jitter (std dev of delay) in milliseconds
    pub c2s_jitter: [f64; 3],
    pub s2c_jitter: [f64; 3],

    // Loss rate as percentage
    pub c2s_loss_rate: [f64; 3],
    pub s2c_loss_rate: [f64; 3],

    // Reordering rate as percentage
    pub c2s_reorder_rate: [f64; 3],
    pub s2c_reorder_rate: [f64; 3],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_metrics_default() {
        let metrics = ClientMetrics::default();
        assert_eq!(metrics.c2s_throughput, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_client_metrics_serialization() {
        let mut metrics = ClientMetrics::default();
        metrics.c2s_throughput = [1000.0, 900.0, 850.0];

        let json = serde_json::to_string(&metrics).unwrap();
        let deserialized: ClientMetrics = serde_json::from_str(&json).unwrap();

        assert_eq!(metrics.c2s_throughput, deserialized.c2s_throughput);
    }
}
```

**Step 6: Run tests**

Run: `cargo test -p common`
Expected: PASS (4 tests)

**Step 7: Add DashboardMessage type**

Add to `common/src/protocol.rs`:
```rust
use crate::metrics::ClientMetrics;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub id: String,
    pub connected_at: u64, // Unix timestamp
    pub metrics: ClientMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardMessage {
    pub clients: Vec<ClientInfo>,
}

#[cfg(test)]
mod tests {
    // ... existing tests ...

    #[test]
    fn test_dashboard_message_serialization() {
        let msg = DashboardMessage {
            clients: vec![
                ClientInfo {
                    id: "client-1".to_string(),
                    connected_at: 1234567890,
                    metrics: ClientMetrics::default(),
                }
            ],
        };

        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: DashboardMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(msg.clients.len(), deserialized.clients.len());
    }
}
```

**Step 8: Run tests**

Run: `cargo test -p common`
Expected: PASS (5 tests)

**Step 9: Export modules from lib.rs**

Modify `common/src/lib.rs`:
```rust
pub mod protocol;
pub mod metrics;

pub use protocol::*;
pub use metrics::*;
```

**Step 10: Verify all tests pass**

Run: `cargo test -p common`
Expected: PASS (5 tests)

**Step 11: Commit**

```bash
git add common/
git commit -m "feat(common): add protocol types and metrics structures"
```

---

## Task 3: Server Basic HTTP with Static File Serving

**Files:**
- Modify: `server/src/main.rs`
- Create: `server/src/static_files.rs`
- Create: `server/static/index.html`
- Create: `server/static/dashboard.html`

**Step 1: Create basic axum server**

Modify `server/src/main.rs`:
```rust
use axum::{Router, routing::get};
use std::net::SocketAddr;
use tower_http::trace::TraceLayer;
use tracing_subscriber;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let app = Router::new()
        .route("/health", get(health_check))
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health_check() -> &'static str {
    "OK"
}
```

**Step 2: Test server starts**

Run: `cargo run -p wifi-verify-server` (in background or separate terminal)
Run: `curl http://localhost:3000/health`
Expected: Output "OK"

Kill server with Ctrl+C

**Step 3: Add static file serving**

Create `server/static/index.html`:
```html
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>Network Measurement Client</title>
</head>
<body>
    <h1>Network Measurement Client</h1>
    <div id="status">Initializing...</div>
    <div id="metrics">
        <h2>Metrics</h2>
        <pre id="metrics-display">No data yet</pre>
    </div>
</body>
</html>
```

Create `server/static/dashboard.html`:
```html
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>Network Measurement Dashboard</title>
    <style>
        table { border-collapse: collapse; width: 100%; }
        th, td { border: 1px solid #ddd; padding: 8px; text-align: left; }
        th { background-color: #4CAF50; color: white; }
        .disconnected { background-color: #ffcccc; }
    </style>
</head>
<body>
    <h1>Network Measurement Dashboard</h1>
    <div id="status">Connecting...</div>
    <table id="clients-table">
        <thead>
            <tr>
                <th>Client ID</th>
                <th>C2S Throughput (1s/10s/60s)</th>
                <th>S2C Throughput (1s/10s/60s)</th>
                <th>C2S Delay (1s/10s/60s)</th>
                <th>S2C Delay (1s/10s/60s)</th>
            </tr>
        </thead>
        <tbody id="clients-body">
            <tr><td colspan="5">No clients connected</td></tr>
        </tbody>
    </table>
    <script src="/dashboard.js"></script>
</body>
</html>
```

**Step 4: Update server to serve static files**

Modify `server/src/main.rs`:
```rust
use axum::{Router, routing::get};
use std::net::SocketAddr;
use tower_http::{trace::TraceLayer, services::ServeDir};
use tracing_subscriber;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = Router::new()
        .route("/health", get(health_check))
        .nest_service("/", ServeDir::new("server/static"))
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health_check() -> &'static str {
    "OK"
}
```

**Step 5: Test static file serving**

Run: `cargo run -p wifi-verify-server` (background)
Run: `curl http://localhost:3000/` (should see HTML)
Run: `curl http://localhost:3000/dashboard.html` (should see dashboard HTML)

Kill server

**Step 6: Commit**

```bash
git add server/
git commit -m "feat(server): add basic axum server with static file serving"
```

---

## Task 4: Server WebRTC Signaling Endpoints

**Files:**
- Modify: `server/src/main.rs`
- Create: `server/src/signaling.rs`
- Create: `server/src/state.rs`

**Step 1: Create shared state structure**

Create `server/src/state.rs`:
```rust
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub clients: Arc<RwLock<HashMap<String, ClientSession>>>,
}

pub struct ClientSession {
    pub id: String,
    pub pending_ice_candidates: Vec<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl ClientSession {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            pending_ice_candidates: Vec::new(),
        }
    }
}
```

**Step 2: Create signaling handlers**

Create `server/src/signaling.rs`:
```rust
use axum::{
    extract::State,
    Json,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct SignalingStartRequest {
    pub sdp: String,
}

#[derive(Debug, Serialize)]
pub struct SignalingStartResponse {
    pub client_id: String,
    pub sdp: String,
}

#[derive(Debug, Deserialize)]
pub struct IceCandidateRequest {
    pub client_id: String,
    pub candidate: String,
}

pub async fn signaling_start(
    State(state): State<AppState>,
    Json(req): Json<SignalingStartRequest>,
) -> Result<Json<SignalingStartResponse>, StatusCode> {
    tracing::info!("Received signaling start request");

    // For now, just echo back a dummy SDP answer
    // We'll implement real WebRTC in next task
    let client_id = uuid::Uuid::new_v4().to_string();

    let response = SignalingStartResponse {
        client_id,
        sdp: "dummy-answer-sdp".to_string(),
    };

    Ok(Json(response))
}

pub async fn ice_candidate(
    State(state): State<AppState>,
    Json(req): Json<IceCandidateRequest>,
) -> Result<StatusCode, StatusCode> {
    tracing::info!("Received ICE candidate for client {}", req.client_id);

    // Store ICE candidate (will implement properly in next task)

    Ok(StatusCode::OK)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signaling_request_deserialization() {
        let json = r#"{"sdp": "test-sdp"}"#;
        let req: SignalingStartRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.sdp, "test-sdp");
    }

    #[test]
    fn test_signaling_response_serialization() {
        let resp = SignalingStartResponse {
            client_id: "test-123".to_string(),
            sdp: "test-answer".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("test-123"));
    }
}
```

**Step 3: Run tests**

Run: `cargo test -p wifi-verify-server`
Expected: PASS (2 tests)

**Step 4: Wire up signaling endpoints**

Modify `server/src/main.rs`:
```rust
mod state;
mod signaling;

use axum::{Router, routing::{get, post}};
use std::net::SocketAddr;
use tower_http::{trace::TraceLayer, services::ServeDir};
use tracing_subscriber;
use state::AppState;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app_state = AppState::new();

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/signaling/start", post(signaling::signaling_start))
        .route("/api/signaling/ice", post(signaling::ice_candidate))
        .nest_service("/", ServeDir::new("server/static"))
        .with_state(app_state)
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health_check() -> &'static str {
    "OK"
}
```

**Step 5: Test endpoints**

Run: `cargo run -p wifi-verify-server` (background)
Run: `curl -X POST http://localhost:3000/api/signaling/start -H "Content-Type: application/json" -d '{"sdp":"test"}'`
Expected: JSON response with client_id and sdp

Kill server

**Step 6: Commit**

```bash
git add server/
git commit -m "feat(server): add WebRTC signaling endpoints"
```

---

## Task 5: Server WebRTC Peer Connection

**Files:**
- Modify: `server/src/signaling.rs`
- Modify: `server/src/state.rs`
- Create: `server/src/webrtc_manager.rs`

**Step 1: Update ClientSession to hold WebRTC peer**

Modify `server/src/state.rs`:
```rust
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use uuid::Uuid;
use webrtc::peer_connection::RTCPeerConnection;

#[derive(Clone)]
pub struct AppState {
    pub clients: Arc<RwLock<HashMap<String, Arc<ClientSession>>>>,
}

pub struct ClientSession {
    pub id: String,
    pub peer_connection: Arc<RTCPeerConnection>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}
```

**Step 2: Create WebRTC manager**

Create `server/src/webrtc_manager.rs`:
```rust
use webrtc::api::APIBuilder;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;
use std::sync::Arc;

pub async fn create_peer_connection() -> Result<Arc<RTCPeerConnection>, Box<dyn std::error::Error>> {
    let mut media_engine = MediaEngine::default();
    let mut registry = Registry::new();

    register_default_interceptors(&mut media_engine, &mut registry)?;

    let api = APIBuilder::new()
        .with_media_engine(media_engine)
        .with_interceptor_registry(registry)
        .build();

    let config = RTCConfiguration {
        ice_servers: vec![RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".to_owned()],
            ..Default::default()
        }],
        ..Default::default()
    };

    let peer_connection = Arc::new(api.new_peer_connection(config).await?);

    Ok(peer_connection)
}

pub async fn handle_offer(
    peer: &Arc<RTCPeerConnection>,
    offer_sdp: String,
) -> Result<String, Box<dyn std::error::Error>> {
    let offer = RTCSessionDescription::offer(offer_sdp)?;
    peer.set_remote_description(offer).await?;

    let answer = peer.create_answer(None).await?;
    peer.set_local_description(answer.clone()).await?;

    Ok(answer.sdp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_peer_connection() {
        let result = create_peer_connection().await;
        assert!(result.is_ok());
    }
}
```

**Step 3: Run test**

Run: `cargo test -p wifi-verify-server test_create_peer_connection`
Expected: PASS

**Step 4: Update signaling to use real WebRTC**

Modify `server/src/signaling.rs`:
```rust
use axum::{
    extract::State,
    Json,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use crate::state::{AppState, ClientSession};
use crate::webrtc_manager;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct SignalingStartRequest {
    pub sdp: String,
}

#[derive(Debug, Serialize)]
pub struct SignalingStartResponse {
    pub client_id: String,
    pub sdp: String,
}

#[derive(Debug, Deserialize)]
pub struct IceCandidateRequest {
    pub client_id: String,
    pub candidate: String,
}

pub async fn signaling_start(
    State(state): State<AppState>,
    Json(req): Json<SignalingStartRequest>,
) -> Result<Json<SignalingStartResponse>, StatusCode> {
    tracing::info!("Received signaling start request");

    // Create peer connection
    let peer = webrtc_manager::create_peer_connection()
        .await
        .map_err(|e| {
            tracing::error!("Failed to create peer connection: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Handle offer and create answer
    let answer_sdp = webrtc_manager::handle_offer(&peer, req.sdp)
        .await
        .map_err(|e| {
            tracing::error!("Failed to handle offer: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Store client session
    let client_id = uuid::Uuid::new_v4().to_string();
    let session = Arc::new(ClientSession {
        id: client_id.clone(),
        peer_connection: peer,
    });

    state.clients.write().await.insert(client_id.clone(), session);

    let response = SignalingStartResponse {
        client_id,
        sdp: answer_sdp,
    };

    Ok(Json(response))
}

pub async fn ice_candidate(
    State(state): State<AppState>,
    Json(req): Json<IceCandidateRequest>,
) -> Result<StatusCode, StatusCode> {
    tracing::info!("Received ICE candidate for client {}", req.client_id);

    let clients = state.clients.read().await;
    let session = clients.get(&req.client_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    // Parse and add ICE candidate
    let candidate_init = serde_json::from_str(&req.candidate)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    session.peer_connection
        .add_ice_candidate(candidate_init)
        .await
        .map_err(|e| {
            tracing::error!("Failed to add ICE candidate: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(StatusCode::OK)
}

// Keep existing tests...
```

**Step 5: Update main.rs to include webrtc_manager**

Modify `server/src/main.rs`:
```rust
mod state;
mod signaling;
mod webrtc_manager;

// ... rest unchanged
```

**Step 6: Run tests and build**

Run: `cargo test -p wifi-verify-server`
Run: `cargo build -p wifi-verify-server`
Expected: All tests pass, builds successfully

**Step 7: Commit**

```bash
git add server/
git commit -m "feat(server): implement WebRTC peer connection handling"
```

---

## Task 6: Server Data Channels Setup

**Files:**
- Modify: `server/src/webrtc_manager.rs`
- Modify: `server/src/state.rs`
- Create: `server/src/data_channels.rs`

**Step 1: Update state to hold data channels**

Modify `server/src/state.rs`:
```rust
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use uuid::Uuid;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::data_channel::RTCDataChannel;

#[derive(Clone)]
pub struct AppState {
    pub clients: Arc<RwLock<HashMap<String, Arc<ClientSession>>>>,
}

pub struct ClientSession {
    pub id: String,
    pub peer_connection: Arc<RTCPeerConnection>,
    pub data_channels: Arc<RwLock<DataChannels>>,
}

pub struct DataChannels {
    pub probe: Option<Arc<RTCDataChannel>>,
    pub bulk: Option<Arc<RTCDataChannel>>,
    pub control: Option<Arc<RTCDataChannel>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl DataChannels {
    pub fn new() -> Self {
        Self {
            probe: None,
            bulk: None,
            control: None,
        }
    }
}
```

**Step 2: Create data channel handlers**

Create `server/src/data_channels.rs`:
```rust
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::data_channel::RTCDataChannel;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::state::DataChannels;

pub async fn setup_data_channel_handlers(
    peer: &Arc<RTCPeerConnection>,
    channels: Arc<RwLock<DataChannels>>,
    client_id: String,
) {
    let channels_clone = channels.clone();
    let client_id_clone = client_id.clone();

    peer.on_data_channel(Box::new(move |dc: Arc<RTCDataChannel>| {
        let label = dc.label().to_string();
        let channels = channels_clone.clone();
        let client_id = client_id_clone.clone();

        Box::pin(async move {
            tracing::info!("Client {} opened data channel: {}", client_id, label);

            // Store the data channel
            let mut chans = channels.write().await;
            match label.as_str() {
                "probe" => chans.probe = Some(dc.clone()),
                "bulk" => chans.bulk = Some(dc.clone()),
                "control" => chans.control = Some(dc.clone()),
                _ => tracing::warn!("Unknown data channel: {}", label),
            }
            drop(chans);

            // Set up message handler
            let client_id_clone = client_id.clone();
            let label_clone = label.clone();
            dc.on_message(Box::new(move |msg: DataChannelMessage| {
                let client_id = client_id_clone.clone();
                let label = label_clone.clone();
                Box::pin(async move {
                    handle_message(&client_id, &label, msg).await;
                })
            }));
        })
    }));
}

async fn handle_message(client_id: &str, channel: &str, msg: DataChannelMessage) {
    tracing::debug!("Client {} received message on {}: {} bytes",
                   client_id, channel, msg.data.len());

    match channel {
        "probe" => handle_probe_message(client_id, msg).await,
        "bulk" => handle_bulk_message(client_id, msg).await,
        "control" => handle_control_message(client_id, msg).await,
        _ => {}
    }
}

async fn handle_probe_message(client_id: &str, msg: DataChannelMessage) {
    // Will implement in next task
    tracing::trace!("Probe message from {}", client_id);
}

async fn handle_bulk_message(client_id: &str, msg: DataChannelMessage) {
    // Will implement in next task
    tracing::trace!("Bulk message from {}: {} bytes", client_id, msg.data.len());
}

async fn handle_control_message(client_id: &str, msg: DataChannelMessage) {
    // Will implement in next task
    tracing::trace!("Control message from {}", client_id);
}
```

**Step 3: Update signaling to set up data channels**

Modify `server/src/signaling.rs` to add data channel setup:
```rust
// ... existing imports ...
use crate::data_channels;

pub async fn signaling_start(
    State(state): State<AppState>,
    Json(req): Json<SignalingStartRequest>,
) -> Result<Json<SignalingStartResponse>, StatusCode> {
    tracing::info!("Received signaling start request");

    let peer = webrtc_manager::create_peer_connection()
        .await
        .map_err(|e| {
            tracing::error!("Failed to create peer connection: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let client_id = uuid::Uuid::new_v4().to_string();
    let data_channels = Arc::new(tokio::sync::RwLock::new(crate::state::DataChannels::new()));

    // Set up data channel handlers
    data_channels::setup_data_channel_handlers(&peer, data_channels.clone(), client_id.clone()).await;

    let answer_sdp = webrtc_manager::handle_offer(&peer, req.sdp)
        .await
        .map_err(|e| {
            tracing::error!("Failed to handle offer: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let session = Arc::new(ClientSession {
        id: client_id.clone(),
        peer_connection: peer,
        data_channels,
    });

    state.clients.write().await.insert(client_id.clone(), session);

    let response = SignalingStartResponse {
        client_id,
        sdp: answer_sdp,
    };

    Ok(Json(response))
}

// ... rest unchanged
```

**Step 4: Update main.rs**

Modify `server/src/main.rs`:
```rust
mod state;
mod signaling;
mod webrtc_manager;
mod data_channels;

// ... rest unchanged
```

**Step 5: Build and verify**

Run: `cargo build -p wifi-verify-server`
Expected: Builds successfully

**Step 6: Commit**

```bash
git add server/
git commit -m "feat(server): add data channel setup and handlers"
```

---

## Task 7: Server Measurement Logic - Probe Sending

**Files:**
- Modify: `server/src/state.rs`
- Create: `server/src/measurements.rs`
- Modify: `server/src/data_channels.rs`

**Step 1: Add measurement state**

Modify `server/src/state.rs`:
```rust
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use std::time::Instant;
use uuid::Uuid;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::data_channel::RTCDataChannel;
use common::ClientMetrics;

#[derive(Clone)]
pub struct AppState {
    pub clients: Arc<RwLock<HashMap<String, Arc<ClientSession>>>>,
}

pub struct ClientSession {
    pub id: String,
    pub peer_connection: Arc<RTCPeerConnection>,
    pub data_channels: Arc<RwLock<DataChannels>>,
    pub metrics: Arc<RwLock<ClientMetrics>>,
    pub measurement_state: Arc<RwLock<MeasurementState>>,
    pub connected_at: Instant,
}

pub struct DataChannels {
    pub probe: Option<Arc<RTCDataChannel>>,
    pub bulk: Option<Arc<RTCDataChannel>>,
    pub control: Option<Arc<RTCDataChannel>>,
}

pub struct MeasurementState {
    pub probe_seq: u64,
    pub bulk_bytes_sent: u64,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl DataChannels {
    pub fn new() -> Self {
        Self {
            probe: None,
            bulk: None,
            control: None,
        }
    }
}

impl MeasurementState {
    pub fn new() -> Self {
        Self {
            probe_seq: 0,
            bulk_bytes_sent: 0,
        }
    }
}
```

**Step 2: Create measurements module**

Create `server/src/measurements.rs`:
```rust
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tokio::sync::RwLock;
use common::{ProbePacket, Direction, BulkPacket};
use crate::state::{ClientSession, MeasurementState};
use webrtc::data_channel::RTCDataChannel;

pub async fn start_probe_sender(
    session: Arc<ClientSession>,
) {
    let mut interval = interval(Duration::from_millis(50)); // 20 Hz

    loop {
        interval.tick().await;

        // Check if probe channel is ready
        let channels = session.data_channels.read().await;
        let probe_channel = match &channels.probe {
            Some(ch) if ch.ready_state() == webrtc::data_channel::RTCDataChannelState::Open => ch.clone(),
            _ => {
                drop(channels);
                continue;
            }
        };
        drop(channels);

        // Create and send probe packet
        let mut state = session.measurement_state.write().await;
        let seq = state.probe_seq;
        state.probe_seq += 1;
        drop(state);

        let probe = ProbePacket {
            seq,
            timestamp_ms: current_time_ms(),
            direction: Direction::ServerToClient,
        };

        if let Ok(json) = serde_json::to_vec(&probe) {
            if let Err(e) = probe_channel.send(&json.into()).await {
                tracing::error!("Failed to send probe: {}", e);
                break;
            }
        }
    }
}

pub async fn start_bulk_sender(
    session: Arc<ClientSession>,
) {
    let mut interval = interval(Duration::from_millis(10)); // 100 Hz for continuous throughput

    loop {
        interval.tick().await;

        let channels = session.data_channels.read().await;
        let bulk_channel = match &channels.bulk {
            Some(ch) if ch.ready_state() == webrtc::data_channel::RTCDataChannelState::Open => ch.clone(),
            _ => {
                drop(channels);
                continue;
            }
        };
        drop(channels);

        let bulk = BulkPacket::new(1024);

        if let Ok(data) = serde_json::to_vec(&bulk) {
            let bytes_sent = data.len() as u64;
            if let Err(e) = bulk_channel.send(&data.into()).await {
                tracing::error!("Failed to send bulk: {}", e);
                break;
            }

            let mut state = session.measurement_state.write().await;
            state.bulk_bytes_sent += bytes_sent;
        }
    }
}

fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_time_ms() {
        let t1 = current_time_ms();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let t2 = current_time_ms();
        assert!(t2 > t1);
        assert!(t2 - t1 >= 10);
    }
}
```

**Step 3: Run test**

Run: `cargo test -p wifi-verify-server test_current_time_ms`
Expected: PASS

**Step 4: Start measurement tasks when channels open**

Modify `server/src/data_channels.rs`:
```rust
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::data_channel::RTCDataChannel;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::state::{DataChannels, ClientSession};
use crate::measurements;

pub async fn setup_data_channel_handlers(
    peer: &Arc<RTCPeerConnection>,
    session: Arc<ClientSession>,
) {
    let session_clone = session.clone();

    peer.on_data_channel(Box::new(move |dc: Arc<RTCDataChannel>| {
        let label = dc.label().to_string();
        let session = session_clone.clone();

        Box::pin(async move {
            tracing::info!("Client {} opened data channel: {}", session.id, label);

            // Store the data channel
            let mut chans = session.data_channels.write().await;
            match label.as_str() {
                "probe" => {
                    chans.probe = Some(dc.clone());
                    // Start probe sender
                    let session_clone = session.clone();
                    tokio::spawn(async move {
                        measurements::start_probe_sender(session_clone).await;
                    });
                },
                "bulk" => {
                    chans.bulk = Some(dc.clone());
                    // Start bulk sender
                    let session_clone = session.clone();
                    tokio::spawn(async move {
                        measurements::start_bulk_sender(session_clone).await;
                    });
                },
                "control" => chans.control = Some(dc.clone()),
                _ => tracing::warn!("Unknown data channel: {}", label),
            }
            drop(chans);

            // Set up message handler
            let session_clone = session.clone();
            let label_clone = label.clone();
            dc.on_message(Box::new(move |msg: DataChannelMessage| {
                let session = session_clone.clone();
                let label = label_clone.clone();
                Box::pin(async move {
                    handle_message(session, &label, msg).await;
                })
            }));
        })
    }));
}

async fn handle_message(session: Arc<ClientSession>, channel: &str, msg: DataChannelMessage) {
    tracing::debug!("Client {} received message on {}: {} bytes",
                   session.id, channel, msg.data.len());

    match channel {
        "probe" => handle_probe_message(session, msg).await,
        "bulk" => handle_bulk_message(session, msg).await,
        "control" => handle_control_message(session, msg).await,
        _ => {}
    }
}

async fn handle_probe_message(session: Arc<ClientSession>, msg: DataChannelMessage) {
    // Will implement metric calculation in next task
    tracing::trace!("Probe message from {}", session.id);
}

async fn handle_bulk_message(session: Arc<ClientSession>, msg: DataChannelMessage) {
    tracing::trace!("Bulk message from {}: {} bytes", session.id, msg.data.len());
}

async fn handle_control_message(session: Arc<ClientSession>, msg: DataChannelMessage) {
    tracing::trace!("Control message from {}", session.id);
}
```

**Step 5: Update signaling to pass session**

Modify `server/src/signaling.rs`:
```rust
// Update signaling_start function to create full session first
pub async fn signaling_start(
    State(state): State<AppState>,
    Json(req): Json<SignalingStartRequest>,
) -> Result<Json<SignalingStartResponse>, StatusCode> {
    tracing::info!("Received signaling start request");

    let peer = webrtc_manager::create_peer_connection()
        .await
        .map_err(|e| {
            tracing::error!("Failed to create peer connection: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let client_id = uuid::Uuid::new_v4().to_string();
    let data_channels = Arc::new(tokio::sync::RwLock::new(crate::state::DataChannels::new()));
    let metrics = Arc::new(tokio::sync::RwLock::new(common::ClientMetrics::default()));
    let measurement_state = Arc::new(tokio::sync::RwLock::new(crate::state::MeasurementState::new()));

    let session = Arc::new(crate::state::ClientSession {
        id: client_id.clone(),
        peer_connection: peer.clone(),
        data_channels,
        metrics,
        measurement_state,
        connected_at: std::time::Instant::now(),
    });

    data_channels::setup_data_channel_handlers(&peer, session.clone()).await;

    let answer_sdp = webrtc_manager::handle_offer(&peer, req.sdp)
        .await
        .map_err(|e| {
            tracing::error!("Failed to handle offer: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    state.clients.write().await.insert(client_id.clone(), session);

    let response = SignalingStartResponse {
        client_id,
        sdp: answer_sdp,
    };

    Ok(Json(response))
}
```

**Step 6: Update main.rs**

Modify `server/src/main.rs`:
```rust
mod state;
mod signaling;
mod webrtc_manager;
mod data_channels;
mod measurements;

// ... rest unchanged
```

**Step 7: Build**

Run: `cargo build -p wifi-verify-server`
Expected: Builds successfully

**Step 8: Commit**

```bash
git add server/
git commit -m "feat(server): implement probe and bulk packet sending"
```

---

## Task 8: Server Measurement Logic - Metric Calculation

**Files:**
- Modify: `server/src/measurements.rs`
- Modify: `server/src/data_channels.rs`
- Modify: `server/src/state.rs`

**Step 1: Add packet tracking state**

Modify `server/src/state.rs`:
```rust
// ... existing imports ...
use std::collections::VecDeque;

// ... existing structs ...

pub struct MeasurementState {
    pub probe_seq: u64,
    pub bulk_bytes_sent: u64,
    pub received_probes: VecDeque<ReceivedProbe>,
    pub received_bulk_bytes: VecDeque<ReceivedBulk>,
    pub last_received_seq: Option<u64>,
}

#[derive(Clone)]
pub struct ReceivedProbe {
    pub seq: u64,
    pub sent_at_ms: u64,
    pub received_at_ms: u64,
}

#[derive(Clone)]
pub struct ReceivedBulk {
    pub bytes: u64,
    pub received_at_ms: u64,
}

impl MeasurementState {
    pub fn new() -> Self {
        Self {
            probe_seq: 0,
            bulk_bytes_sent: 0,
            received_probes: VecDeque::new(),
            received_bulk_bytes: VecDeque::new(),
            last_received_seq: None,
        }
    }
}
```

**Step 2: Implement probe reception and metric calculation**

Modify `server/src/measurements.rs` to add metric calculation:
```rust
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tokio::sync::RwLock;
use common::{ProbePacket, Direction, BulkPacket, ClientMetrics};
use crate::state::{ClientSession, MeasurementState, ReceivedProbe, ReceivedBulk};
use webrtc::data_channel::RTCDataChannel;
use webrtc::data_channel::data_channel_message::DataChannelMessage;

// ... existing start_probe_sender and start_bulk_sender functions ...

pub async fn handle_probe_packet(
    session: Arc<ClientSession>,
    msg: DataChannelMessage,
) {
    if let Ok(probe) = serde_json::from_slice::<ProbePacket>(&msg.data) {
        let now_ms = current_time_ms();

        let mut state = session.measurement_state.write().await;
        state.received_probes.push_back(ReceivedProbe {
            seq: probe.seq,
            sent_at_ms: probe.timestamp_ms,
            received_at_ms: now_ms,
        });

        // Keep only last 60 seconds of probes
        let cutoff = now_ms - 60_000;
        while let Some(p) = state.received_probes.front() {
            if p.received_at_ms < cutoff {
                state.received_probes.pop_front();
            } else {
                break;
            }
        }

        drop(state);

        // Recalculate metrics
        calculate_metrics(session).await;
    }
}

pub async fn handle_bulk_packet(
    session: Arc<ClientSession>,
    msg: DataChannelMessage,
) {
    let now_ms = current_time_ms();
    let bytes = msg.data.len() as u64;

    let mut state = session.measurement_state.write().await;
    state.received_bulk_bytes.push_back(ReceivedBulk {
        bytes,
        received_at_ms: now_ms,
    });

    // Keep only last 60 seconds
    let cutoff = now_ms - 60_000;
    while let Some(b) = state.received_bulk_bytes.front() {
        if b.received_at_ms < cutoff {
            state.received_bulk_bytes.pop_front();
        } else {
            break;
        }
    }

    drop(state);
    calculate_metrics(session).await;
}

async fn calculate_metrics(session: Arc<ClientSession>) {
    let state = session.measurement_state.read().await;
    let now_ms = current_time_ms();

    let mut metrics = ClientMetrics::default();

    // Calculate for each time window: 1s, 10s, 60s
    let windows = [1_000u64, 10_000, 60_000];

    for (i, &window_ms) in windows.iter().enumerate() {
        let cutoff = now_ms.saturating_sub(window_ms);

        // Client-to-server metrics (from received probes)
        let recent_probes: Vec<_> = state.received_probes.iter()
            .filter(|p| p.received_at_ms >= cutoff)
            .collect();

        if !recent_probes.is_empty() {
            // Calculate delay
            let delays: Vec<f64> = recent_probes.iter()
                .map(|p| (p.received_at_ms - p.sent_at_ms) as f64)
                .collect();

            let avg_delay = delays.iter().sum::<f64>() / delays.len() as f64;
            metrics.c2s_delay_avg[i] = avg_delay;

            // Calculate jitter (std dev of delay)
            let variance = delays.iter()
                .map(|d| (d - avg_delay).powi(2))
                .sum::<f64>() / delays.len() as f64;
            metrics.c2s_jitter[i] = variance.sqrt();

            // Calculate loss rate
            if recent_probes.len() >= 2 {
                let min_seq = recent_probes.iter().map(|p| p.seq).min().unwrap();
                let max_seq = recent_probes.iter().map(|p| p.seq).max().unwrap();
                let expected = (max_seq - min_seq + 1) as f64;
                let received = recent_probes.len() as f64;
                metrics.c2s_loss_rate[i] = ((expected - received) / expected * 100.0).max(0.0);
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
            metrics.c2s_reorder_rate[i] = (reorders as f64 / recent_probes.len() as f64) * 100.0;
        }

        // Client-to-server throughput (from received bulk)
        let recent_bulk: Vec<_> = state.received_bulk_bytes.iter()
            .filter(|b| b.received_at_ms >= cutoff)
            .collect();

        if !recent_bulk.is_empty() {
            let total_bytes: u64 = recent_bulk.iter().map(|b| b.bytes).sum();
            let time_window_sec = window_ms as f64 / 1000.0;
            metrics.c2s_throughput[i] = total_bytes as f64 / time_window_sec;
        }

        // Server-to-client throughput (from sent bulk)
        // Simplified: use bulk_bytes_sent / window
        // (In real implementation, would track sent timestamps)
        metrics.s2c_throughput[i] = 0.0; // Placeholder
    }

    drop(state);

    // Update session metrics
    *session.metrics.write().await = metrics;
}

// ... existing functions ...
```

**Step 3: Wire up handlers in data_channels**

Modify `server/src/data_channels.rs`:
```rust
// ... existing code ...

async fn handle_probe_message(session: Arc<ClientSession>, msg: DataChannelMessage) {
    measurements::handle_probe_packet(session, msg).await;
}

async fn handle_bulk_message(session: Arc<ClientSession>, msg: DataChannelMessage) {
    measurements::handle_bulk_packet(session, msg).await;
}

// ... rest unchanged ...
```

**Step 4: Build and verify**

Run: `cargo build -p wifi-verify-server`
Expected: Builds successfully

**Step 5: Commit**

```bash
git add server/
git commit -m "feat(server): implement metric calculation for probes and bulk data"
```

---

## Task 9: Server Dashboard WebSocket

**Files:**
- Create: `server/src/dashboard.rs`
- Modify: `server/src/main.rs`
- Create: `server/static/dashboard.js`

**Step 1: Create dashboard WebSocket handler**

Create `server/src/dashboard.rs`:
```rust
use axum::{
    extract::{State, ws::{WebSocket, WebSocketUpgrade, Message}},
    response::Response,
};
use futures::{stream::StreamExt, SinkExt};
use tokio::time::{interval, Duration};
use common::{DashboardMessage, ClientInfo};
use crate::state::AppState;

pub async fn dashboard_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(|socket| dashboard_ws(socket, state))
}

async fn dashboard_ws(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // Spawn task to send updates periodically
    let send_task = tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(1));

        loop {
            interval.tick().await;

            // Collect all client metrics
            let clients_lock = state.clients.read().await;
            let mut clients_info = Vec::new();

            for (_, session) in clients_lock.iter() {
                let metrics = session.metrics.read().await.clone();
                let connected_at = session.connected_at.elapsed().as_secs();

                clients_info.push(ClientInfo {
                    id: session.id.clone(),
                    connected_at,
                    metrics,
                });
            }
            drop(clients_lock);

            let msg = DashboardMessage {
                clients: clients_info,
            };

            if let Ok(json) = serde_json::to_string(&msg) {
                if sender.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
        }
    });

    // Receive task (handle incoming messages if needed)
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            // Dashboard doesn't send messages for now
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}
```

**Step 2: Wire up dashboard endpoint**

Modify `server/src/main.rs`:
```rust
mod state;
mod signaling;
mod webrtc_manager;
mod data_channels;
mod measurements;
mod dashboard;

use axum::{Router, routing::{get, post}};
use std::net::SocketAddr;
use tower_http::{trace::TraceLayer, services::ServeDir};
use tracing_subscriber;
use state::AppState;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app_state = AppState::new();

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/signaling/start", post(signaling::signaling_start))
        .route("/api/signaling/ice", post(signaling::ice_candidate))
        .route("/api/dashboard/ws", get(dashboard::dashboard_ws_handler))
        .nest_service("/", ServeDir::new("server/static"))
        .with_state(app_state)
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health_check() -> &'static str {
    "OK"
}
```

**Step 3: Create dashboard JavaScript**

Create `server/static/dashboard.js`:
```javascript
let ws = null;

function connect() {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${window.location.host}/api/dashboard/ws`;

    ws = new WebSocket(wsUrl);

    ws.onopen = () => {
        console.log('Dashboard WebSocket connected');
        document.getElementById('status').textContent = 'Connected';
        document.getElementById('status').style.color = 'green';
    };

    ws.onmessage = (event) => {
        const data = JSON.parse(event.data);
        updateClientsTable(data.clients);
    };

    ws.onerror = (error) => {
        console.error('WebSocket error:', error);
        document.getElementById('status').textContent = 'Error';
        document.getElementById('status').style.color = 'red';
    };

    ws.onclose = () => {
        console.log('Dashboard WebSocket closed');
        document.getElementById('status').textContent = 'Disconnected - Reconnecting...';
        document.getElementById('status').style.color = 'orange';
        setTimeout(connect, 1000);
    };
}

function updateClientsTable(clients) {
    const tbody = document.getElementById('clients-body');

    if (clients.length === 0) {
        tbody.innerHTML = '<tr><td colspan="5">No clients connected</td></tr>';
        return;
    }

    tbody.innerHTML = '';

    for (const client of clients) {
        const row = document.createElement('tr');

        const formatMetric = (values) => {
            return values.map(v => v.toFixed(2)).join(' / ');
        };

        const formatBytes = (values) => {
            return values.map(v => (v / 1024).toFixed(2) + ' KB/s').join(' / ');
        };

        row.innerHTML = `
            <td>${client.id}</td>
            <td>${formatBytes(client.metrics.c2s_throughput)}</td>
            <td>${formatBytes(client.metrics.s2c_throughput)}</td>
            <td>${formatMetric(client.metrics.c2s_delay_avg)} ms</td>
            <td>${formatMetric(client.metrics.s2c_delay_avg)} ms</td>
        `;

        tbody.appendChild(row);
    }
}

// Connect when page loads
connect();
```

**Step 4: Update dashboard.html to include script**

File already has `<script src="/dashboard.js"></script>` from Task 3, so no changes needed.

**Step 5: Build and verify**

Run: `cargo build -p wifi-verify-server`
Expected: Builds successfully

**Step 6: Manual test**

Run: `cargo run -p wifi-verify-server`
Open browser to `http://localhost:3000/dashboard.html`
Expected: Dashboard loads and WebSocket connects

Kill server

**Step 7: Commit**

```bash
git add server/
git commit -m "feat(server): add dashboard WebSocket for real-time client monitoring"
```

---

## Task 10: Client WASM Project Setup

**Files:**
- Modify: `client/Cargo.toml`
- Create: `client/build.sh`
- Modify: `server/static/index.html`

**Step 1: Install wasm-pack**

Run: `cargo install wasm-pack`
Expected: wasm-pack installs successfully

**Step 2: Create build script**

Create `client/build.sh`:
```bash
#!/bin/bash
set -e

echo "Building WASM client..."
wasm-pack build --target web --out-dir ../server/static/pkg

echo "WASM client built successfully!"
echo "Output: server/static/pkg/"
```

Run: `chmod +x client/build.sh`

**Step 3: Update client lib.rs with basic structure**

Modify `client/src/lib.rs`:
```rust
use wasm_bindgen::prelude::*;
use web_sys::console;

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
    use super::*;

    #[test]
    fn test_basic() {
        assert_eq!(2 + 2, 4);
    }
}
```

**Step 4: Run test**

Run: `cargo test -p wifi-verify-client`
Expected: PASS

**Step 5: Build WASM**

Run: `cd client && ./build.sh`
Expected: WASM builds successfully, outputs to `server/static/pkg/`

**Step 6: Update index.html to load WASM**

Modify `server/static/index.html`:
```html
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>Network Measurement Client</title>
    <style>
        body { font-family: Arial, sans-serif; padding: 20px; }
        #status { font-weight: bold; margin: 20px 0; }
        #metrics { margin-top: 20px; }
        table { border-collapse: collapse; margin-top: 10px; }
        th, td { border: 1px solid #ddd; padding: 8px; text-align: left; }
        th { background-color: #4CAF50; color: white; }
        button { padding: 10px 20px; font-size: 16px; }
    </style>
</head>
<body>
    <h1>Network Measurement Client</h1>
    <button id="start-btn" onclick="startMeasurement()">Start Measurement</button>
    <div id="status">Ready</div>
    <div id="metrics">
        <h2>Metrics</h2>
        <table>
            <tr><th>Metric</th><th>1s</th><th>10s</th><th>60s</th></tr>
            <tr><td>C2S Throughput</td><td id="c2s-tp-1">-</td><td id="c2s-tp-10">-</td><td id="c2s-tp-60">-</td></tr>
            <tr><td>S2C Throughput</td><td id="s2c-tp-1">-</td><td id="s2c-tp-10">-</td><td id="s2c-tp-60">-</td></tr>
            <tr><td>C2S Delay (ms)</td><td id="c2s-delay-1">-</td><td id="c2s-delay-10">-</td><td id="c2s-delay-60">-</td></tr>
            <tr><td>S2C Delay (ms)</td><td id="s2c-delay-1">-</td><td id="s2c-delay-10">-</td><td id="s2c-delay-60">-</td></tr>
            <tr><td>C2S Jitter (ms)</td><td id="c2s-jitter-1">-</td><td id="c2s-jitter-10">-</td><td id="c2s-jitter-60">-</td></tr>
            <tr><td>S2C Jitter (ms)</td><td id="s2c-jitter-1">-</td><td id="s2c-jitter-10">-</td><td id="s2c-jitter-60">-</td></tr>
        </table>
    </div>

    <script type="module">
        import init, { start_measurement } from './pkg/wifi_verify_client.js';

        async function run() {
            await init();
            console.log('WASM module loaded');
        }

        window.startMeasurement = async function() {
            document.getElementById('status').textContent = 'Starting...';
            try {
                await start_measurement();
                document.getElementById('status').textContent = 'Running';
            } catch (e) {
                document.getElementById('status').textContent = 'Error: ' + e;
                console.error(e);
            }
        };

        run();
    </script>
</body>
</html>
```

**Step 7: Test WASM loads**

Run: `cargo run -p wifi-verify-server`
Open browser to `http://localhost:3000/`
Open browser console
Expected: See "WASM module loaded" in console

Kill server

**Step 8: Commit**

```bash
git add client/ server/static/
git commit -m "feat(client): set up WASM project with build script"
```

---

## Task 11: Client WebRTC Connection

**Files:**
- Modify: `client/src/lib.rs`
- Create: `client/src/webrtc.rs`
- Create: `client/src/signaling.rs`

**Step 1: Create signaling client**

Create `client/src/signaling.rs`:
```rust
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
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
    opts.method("POST");
    opts.mode(RequestMode::Cors);
    opts.body(Some(&JsValue::from_str(&body_str)));

    let url = format!("{}/api/signaling/start",
                     window.location().origin().map_err(|e| JsValue::from_str("No origin"))?);

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
```

**Step 2: Run test**

Run: `cargo test -p wifi-verify-client`
Expected: PASS

**Step 3: Create WebRTC connection helper**

Create `client/src/webrtc.rs`:
```rust
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    RtcPeerConnection, RtcConfiguration, RtcDataChannel,
    RtcDataChannelInit, RtcSessionDescriptionInit, RtcSdpType,
};
use crate::signaling;

pub struct WebRtcConnection {
    pub peer: RtcPeerConnection,
    pub client_id: String,
}

impl WebRtcConnection {
    pub async fn new() -> Result<Self, JsValue> {
        log::info!("Creating RTCPeerConnection");

        let mut config = RtcConfiguration::new();
        // Use Google's public STUN server
        let ice_servers = js_sys::Array::new();
        let server = js_sys::Object::new();
        js_sys::Reflect::set(&server, &"urls".into(), &"stun:stun.l.google.com:19302".into())?;
        ice_servers.push(&server);
        config.ice_servers(&ice_servers);

        let peer = RtcPeerConnection::new_with_configuration(&config)?;

        log::info!("Creating data channels");

        // Create probe channel (unreliable, unordered)
        let mut probe_init = RtcDataChannelInit::new();
        probe_init.ordered(false);
        probe_init.max_retransmits(0);
        let _probe_channel = peer.create_data_channel_with_data_channel_dict("probe", &probe_init);

        // Create bulk channel (unreliable, unordered) for realistic throughput measurement
        let bulk_init = RtcDataChannelInit::new();
        bulk_init.set_ordered(false);
        bulk_init.set_max_retransmits(0);
        let _bulk_channel = peer.create_data_channel_with_data_channel_dict("bulk", &bulk_init);

        // Create control channel (reliable, ordered)
        let control_init = RtcDataChannelInit::new();
        let _control_channel = peer.create_data_channel_with_data_channel_dict("control", &control_init);

        log::info!("Creating offer");

        let offer = wasm_bindgen_futures::JsFuture::from(peer.create_offer()).await?;
        let offer_sdp = js_sys::Reflect::get(&offer, &"sdp".into())?
            .as_string()
            .ok_or("No SDP in offer")?;

        let mut offer_obj = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
        offer_obj.sdp(&offer_sdp);

        wasm_bindgen_futures::JsFuture::from(
            peer.set_local_description(&offer_obj)
        ).await?;

        log::info!("Sending offer to server");

        let (client_id, answer_sdp) = signaling::send_offer(offer_sdp).await?;

        log::info!("Received answer from server, client_id: {}", client_id);

        let mut answer_obj = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
        answer_obj.sdp(&answer_sdp);

        wasm_bindgen_futures::JsFuture::from(
            peer.set_remote_description(&answer_obj)
        ).await?;

        log::info!("WebRTC connection established");

        Ok(Self { peer, client_id })
    }
}
```

**Step 4: Update lib.rs to use WebRTC**

Modify `client/src/lib.rs`:
```rust
mod webrtc;
mod signaling;

use wasm_bindgen::prelude::*;
use web_sys::console;

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

    // Will implement measurement logic in next tasks

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_basic() {
        assert_eq!(2 + 2, 4);
    }
}
```

**Step 5: Build WASM**

Run: `cd client && ./build.sh`
Expected: Builds successfully

**Step 6: Manual test**

Run: `cargo run -p wifi-verify-server`
Open browser to `http://localhost:3000/`
Click "Start Measurement"
Check browser console
Expected: See WebRTC connection logs

Kill server

**Step 7: Commit**

```bash
git add client/
git commit -m "feat(client): implement WebRTC connection and signaling"
```

---

## Task 12: Client Data Channels and Measurement

**Files:**
- Modify: `client/src/webrtc.rs`
- Create: `client/src/measurements.rs`
- Modify: `client/src/lib.rs`

**Step 1: Create measurements module**

Create `client/src/measurements.rs`:
```rust
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{RtcDataChannel, MessageEvent};
use common::{ProbePacket, Direction, BulkPacket, ClientMetrics};
use std::sync::Arc;
use std::cell::RefCell;
use std::rc::Rc;

pub struct MeasurementState {
    pub probe_seq: u64,
    pub metrics: ClientMetrics,
}

impl MeasurementState {
    pub fn new() -> Self {
        Self {
            probe_seq: 0,
            metrics: ClientMetrics::default(),
        }
    }
}

pub fn setup_probe_channel(
    channel: RtcDataChannel,
    state: Rc<RefCell<MeasurementState>>,
) {
    let state_clone = state.clone();

    let onopen = Closure::wrap(Box::new(move || {
        log::info!("Probe channel opened");

        // Start sending probes every 50ms
        let state = state_clone.clone();
        let interval_id = gloo_timers::callback::Interval::new(50, move || {
            let mut state = state.borrow_mut();
            let probe = ProbePacket {
                seq: state.probe_seq,
                timestamp_ms: current_time_ms(),
                direction: Direction::ClientToServer,
            };
            state.probe_seq += 1;

            if let Ok(json) = serde_json::to_string(&probe) {
                // Note: We don't have direct access to channel here
                // Will fix in next step
                log::trace!("Would send probe seq {}", probe.seq);
            }
        });

        // Store interval_id to prevent it from being dropped
        // (In real implementation, would store this properly)
        std::mem::forget(interval_id);
    }) as Box<dyn FnMut()>);

    channel.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    onopen.forget();

    // Handle incoming probes from server
    let onmessage = Closure::wrap(Box::new(move |ev: MessageEvent| {
        if let Ok(txt) = ev.data().dyn_into::<js_sys::JsString>() {
            let data: String = txt.into();
            if let Ok(probe) = serde_json::from_str::<ProbePacket>(&data) {
                log::trace!("Received probe seq {}", probe.seq);
                // Will implement metric calculation in next step
            }
        }
    }) as Box<dyn FnMut(MessageEvent)>);

    channel.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();
}

pub fn setup_bulk_channel(channel: RtcDataChannel) {
    let onopen = Closure::wrap(Box::new(move || {
        log::info!("Bulk channel opened");

        // Start sending bulk data every 10ms
        let interval_id = gloo_timers::callback::Interval::new(10, move || {
            let bulk = BulkPacket::new(1024);
            if let Ok(json) = serde_json::to_string(&bulk) {
                log::trace!("Would send bulk {} bytes", json.len());
            }
        });

        std::mem::forget(interval_id);
    }) as Box<dyn FnMut()>);

    channel.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    onopen.forget();

    let onmessage = Closure::wrap(Box::new(move |ev: MessageEvent| {
        if let Ok(txt) = ev.data().dyn_into::<js_sys::JsString>() {
            let data: String = txt.into();
            log::trace!("Received bulk {} bytes", data.len());
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
}

fn current_time_ms() -> u64 {
    js_sys::Date::now() as u64
}
```

**Step 2: Update webrtc.rs to set up channel handlers**

Modify `client/src/webrtc.rs`:
```rust
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    RtcPeerConnection, RtcConfiguration, RtcDataChannel,
    RtcDataChannelInit, RtcSessionDescriptionInit, RtcSdpType,
};
use crate::{signaling, measurements};
use std::rc::Rc;
use std::cell::RefCell;

pub struct WebRtcConnection {
    pub peer: RtcPeerConnection,
    pub client_id: String,
    pub state: Rc<RefCell<measurements::MeasurementState>>,
}

impl WebRtcConnection {
    pub async fn new() -> Result<Self, JsValue> {
        log::info!("Creating RTCPeerConnection");

        let mut config = RtcConfiguration::new();
        let ice_servers = js_sys::Array::new();
        let server = js_sys::Object::new();
        js_sys::Reflect::set(&server, &"urls".into(), &"stun:stun.l.google.com:19302".into())?;
        ice_servers.push(&server);
        config.ice_servers(&ice_servers);

        let peer = RtcPeerConnection::new_with_configuration(&config)?;
        let state = Rc::new(RefCell::new(measurements::MeasurementState::new()));

        log::info!("Creating data channels");

        // Create probe channel
        let mut probe_init = RtcDataChannelInit::new();
        probe_init.ordered(false);
        probe_init.max_retransmits(0);
        let probe_channel = peer.create_data_channel_with_data_channel_dict("probe", &probe_init);
        measurements::setup_probe_channel(probe_channel, state.clone());

        // Create bulk channel
        let bulk_init = RtcDataChannelInit::new();
        let bulk_channel = peer.create_data_channel_with_data_channel_dict("bulk", &bulk_init);
        measurements::setup_bulk_channel(bulk_channel);

        // Create control channel
        let control_init = RtcDataChannelInit::new();
        let control_channel = peer.create_data_channel_with_data_channel_dict("control", &control_init);
        measurements::setup_control_channel(control_channel);

        log::info!("Creating offer");

        let offer = wasm_bindgen_futures::JsFuture::from(peer.create_offer()).await?;
        let offer_sdp = js_sys::Reflect::get(&offer, &"sdp".into())?
            .as_string()
            .ok_or("No SDP in offer")?;

        let mut offer_obj = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
        offer_obj.sdp(&offer_sdp);

        wasm_bindgen_futures::JsFuture::from(
            peer.set_local_description(&offer_obj)
        ).await?;

        log::info!("Sending offer to server");

        let (client_id, answer_sdp) = signaling::send_offer(offer_sdp).await?;

        log::info!("Received answer from server, client_id: {}", client_id);

        let mut answer_obj = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
        answer_obj.sdp(&answer_sdp);

        wasm_bindgen_futures::JsFuture::from(
            peer.set_remote_description(&answer_obj)
        ).await?;

        log::info!("WebRTC connection established");

        Ok(Self { peer, client_id, state })
    }
}
```

**Step 3: Update lib.rs**

Modify `client/src/lib.rs`:
```rust
mod webrtc;
mod signaling;
mod measurements;

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

    // Connection and measurement tasks are now running
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
```

**Step 4: Build WASM**

Run: `cd client && ./build.sh`
Expected: Builds successfully

**Step 5: Manual test**

Run: `cargo run -p wifi-verify-server`
Open browser to `http://localhost:3000/`
Click "Start Measurement"
Check browser console - should see data channels opening

Kill server

**Step 6: Commit**

```bash
git add client/
git commit -m "feat(client): implement data channels and basic measurement logic"
```

---

## Task 13: Client UI Updates

**Files:**
- Modify: `client/src/lib.rs`
- Modify: `client/src/measurements.rs`
- Modify: `server/static/index.html`

**Step 1: Add UI update function**

Modify `client/src/lib.rs`:
```rust
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
        update_ui(&state.borrow().metrics);
    }).forget();

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
        format!("{:.2} KB/s", bytes / 1024.0)
    };

    let format_ms = |ms: f64| -> String {
        format!("{:.2} ms", ms)
    };

    set_element_text(&document, "c2s-tp-1", &format_bytes(metrics.c2s_throughput[0]));
    set_element_text(&document, "c2s-tp-10", &format_bytes(metrics.c2s_throughput[1]));
    set_element_text(&document, "c2s-tp-60", &format_bytes(metrics.c2s_throughput[2]));

    set_element_text(&document, "s2c-tp-1", &format_bytes(metrics.s2c_throughput[0]));
    set_element_text(&document, "s2c-tp-10", &format_bytes(metrics.s2c_throughput[1]));
    set_element_text(&document, "s2c-tp-60", &format_bytes(metrics.s2c_throughput[2]));

    set_element_text(&document, "c2s-delay-1", &format_ms(metrics.c2s_delay_avg[0]));
    set_element_text(&document, "c2s-delay-10", &format_ms(metrics.c2s_delay_avg[1]));
    set_element_text(&document, "c2s-delay-60", &format_ms(metrics.c2s_delay_avg[2]));

    set_element_text(&document, "s2c-delay-1", &format_ms(metrics.s2c_delay_avg[0]));
    set_element_text(&document, "s2c-delay-10", &format_ms(metrics.s2c_delay_avg[1]));
    set_element_text(&document, "s2c-delay-60", &format_ms(metrics.s2c_delay_avg[2]));

    set_element_text(&document, "c2s-jitter-1", &format_ms(metrics.c2s_jitter[0]));
    set_element_text(&document, "c2s-jitter-10", &format_ms(metrics.c2s_jitter[1]));
    set_element_text(&document, "c2s-jitter-60", &format_ms(metrics.c2s_jitter[2]));

    set_element_text(&document, "s2c-jitter-1", &format_ms(metrics.s2c_jitter[0]));
    set_element_text(&document, "s2c-jitter-10", &format_ms(metrics.s2c_jitter[1]));
    set_element_text(&document, "s2c-jitter-60", &format_ms(metrics.s2c_jitter[2]));
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
```

**Step 2: Build WASM**

Run: `cd client && ./build.sh`
Expected: Builds successfully

**Step 3: Manual test**

Run: `cargo run -p wifi-verify-server`
Open browser to `http://localhost:3000/`
Click "Start Measurement"
Expected: See metrics updating in the table (even if zeros)

Kill server

**Step 4: Commit**

```bash
git add client/ server/static/
git commit -m "feat(client): add UI updates for real-time metrics display"
```

---

## Task 14: Final Integration and Testing

**Files:**
- Create: `README.md`
- Create: `.github/workflows/ci.yml` (optional)

**Step 1: Create README**

Create `README.md`:
```markdown
# WiFi Verify - Network Measurement System

Browser-based network measurement tool using WebRTC to continuously monitor:
- Throughput (upload/download)
- Delay (latency)
- Jitter (delay variation)
- Packet loss
- Packet reordering

## Architecture

- **Server**: Rust (tokio, axum, webrtc-rs)
- **Client**: Rust → WASM (wasm-bindgen, web-sys)
- **Protocol**: WebRTC data channels
- **Dashboard**: Real-time WebSocket updates

## Quick Start

### Prerequisites

- Rust (stable)
- wasm-pack: `cargo install wasm-pack`

### Build

```bash
# Build client WASM
cd client && ./build.sh && cd ..

# Build server
cargo build --release -p wifi-verify-server
```

### Run

```bash
cargo run --release -p wifi-verify-server
```

Then open in browser:
- Client: http://localhost:3000/
- Dashboard: http://localhost:3000/dashboard.html

## Development

```bash
# Run tests
cargo test --workspace

# Build client for development
cd client && ./build.sh

# Run server
cargo run -p wifi-verify-server
```

## How It Works

1. Client creates WebRTC connection to server
2. Three data channels established:
   - **Probe** (unreliable): High-frequency packets for delay/jitter/loss measurement
   - **Bulk** (unreliable, unordered): Continuous data for realistic throughput measurement
   - **Control** (reliable): Stats reporting
3. Both client and server send probes/bulk data bidirectionally
4. Metrics calculated over 1s, 10s, and 60s windows
5. Dashboard shows real-time view of all connected clients

## License

MIT
```

**Step 2: Build everything**

Run: `cd client && ./build.sh && cd ..`
Run: `cargo build --workspace --release`
Expected: All builds successfully

**Step 3: Run integration test**

Run: `cargo run --release -p wifi-verify-server` (background)
Open browser to `http://localhost:3000/`
Click "Start Measurement"
Open another tab to `http://localhost:3000/dashboard.html`
Expected:
- Client shows metrics updating
- Dashboard shows client connected with metrics

Kill server

**Step 4: Run all tests**

Run: `cargo test --workspace`
Expected: All tests pass

**Step 5: Commit**

```bash
git add README.md
git commit -m "docs: add README with architecture and usage instructions"
```

**Step 6: Final commit**

```bash
git log --oneline
```

Expected: See all commits from Task 1 through Task 14

---

## Plan Complete

**Plan saved to:** `docs/plans/2025-12-05-network-measurement-implementation.md`

**Execution Options:**

1. **Subagent-Driven (this session)** - I dispatch fresh subagent per task, review between tasks, fast iteration

2. **Parallel Session (separate)** - Open new session with executing-plans, batch execution with checkpoints

Which approach would you like?
