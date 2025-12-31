# Network Measurement System Design

Date: 2025-12-05

## Overview

Browser-based network measurement system using WebRTC for continuous monitoring of throughput, delay, jitter, packet loss, and reordering between clients and server.

## System Architecture

### Components

**1. Rust Server (Tokio-based)**
- Serves WASM client (HTML/JS/WASM bundle)
- Handles WebRTC signaling via HTTP endpoints
- Maintains WebRTC data channel connections with each client
- Sends/receives measurement packets bidirectionally
- Calculates network metrics in real-time (1s, 10s, 1min windows)
- Serves dashboard page
- Broadcasts client states to dashboard viewers via WebSocket

**2. WASM Client (Rust → WASM)**
- Runs in browser, establishes WebRTC connection to server
- Opens three data channels:
  - **Probe channel** (unreliable, unordered): Rapid probe packets
  - **Bulk channel** (unreliable, unordered): Realistic throughput measurement traffic
  - **Control channel** (reliable, ordered): Stats reporting
- Measures incoming packets (delay, jitter, reordering, loss)
- Calculates metrics locally and displays in UI
- Reports aggregated stats to server via control channel

**3. Dashboard Page (HTML/JS + WebSocket)**
- Real-time view of all connected clients
- Shows per-client metrics (throughput, delay, jitter, loss, reordering)
- Updates live via WebSocket as measurements flow
- Simple table/grid layout with client ID and metrics

## WebRTC Connection Flow

### Connection Establishment

1. **Client loads page** → Browser fetches HTML/JS/WASM from server
2. **Client initiates signaling:**
   - Creates RTCPeerConnection with STUN server config
   - Creates three data channels (probe, bulk, control)
   - Generates SDP offer
   - POSTs offer to `/api/signaling/start`
3. **Server processes signaling:**
   - Creates corresponding RTCPeerConnection
   - Sets remote description (client's offer)
   - Creates answer SDP
   - Returns answer to client
4. **ICE candidate exchange:**
   - Client POSTs ICE candidates to `/api/signaling/ice`
   - Server adds them to peer connection
   - Server's candidates sent in response or via polling
5. **Connection established** → Data channels open, measurement begins

### Data Channel Configuration

- **Probe channel**: `{ordered: false, maxRetransmits: 0}` - unreliable for low-latency probes
- **Bulk channel**: `{ordered: false, maxRetransmits: 0}` - unreliable for realistic throughput measurement under actual network conditions
- **Control channel**: `{ordered: true}` - reliable for stats/control messages

### Rationale

HTTP signaling is stateless and simple. Once WebRTC is established, all measurement traffic flows over data channels, with no dependency on the signaling path.

## Measurement Methodology

### Probe Packets (Delay, Jitter, Reordering, Loss)

- **Frequency**: 20 probes/second (every 50ms) in each direction
- **Packet structure**: `{seq: u64, timestamp_ms: u64, direction: "c2s" | "s2c"}`
- **Client→Server probes**: Client sends with incrementing seq + client timestamp
- **Server→Client probes**: Server sends independently with own seq + server timestamp

**Measurements:**
- **Loss**: Missing sequence numbers in received stream
- **Reordering**: seq(n) arrives after seq(n+1)
- **Delay**: `receive_time - send_time` (one-way if clocks sync'd, or RTT with echo)
- **Jitter**: Standard deviation of delay over time window

### Throughput Packets (Bulk channel)

- **Mode**: Continuous send of 1KB chunks on bulk channel
- **Server→Client**: Server continuously sends data
- **Client→Server**: Client continuously sends data
- **Measurement**: Bytes received / time window = throughput in each direction
- **Adaptive**: Could throttle if not needed continuously

### Time Windows

All metrics calculated over:
- **1 second**: Immediate feedback
- **10 seconds**: Medium-term trend
- **60 seconds**: Long-term average

## Data Structures & State Management

### Server-side State

```rust
struct ClientSession {
    id: String,
    peer_connection: RTCPeerConnection,
    data_channels: DataChannels,
    metrics: ClientMetrics,
    connected_at: Instant,
}

struct ClientMetrics {
    // Calculated over 1s, 10s, 60s windows
    c2s_throughput: [f64; 3],      // bytes/sec
    s2c_throughput: [f64; 3],
    c2s_delay_avg: [f64; 3],       // ms
    s2c_delay_avg: [f64; 3],
    c2s_jitter: [f64; 3],          // ms std dev
    s2c_jitter: [f64; 3],
    c2s_loss_rate: [f64; 3],       // percentage
    s2c_loss_rate: [f64; 3],
    c2s_reorder_rate: [f64; 3],    // percentage
    s2c_reorder_rate: [f64; 3],
}
```

**Server maintains:**
- `HashMap<ClientId, ClientSession>` - all active clients
- `Vec<WebSocket>` - connected dashboard viewers
- Spawns tasks per client for sending probes and bulk data
- Updates metrics in real-time, broadcasts to dashboards

**Client-side State:**
- Mirror of `ClientMetrics` for local display
- Sequence tracking for sent/received probes
- Rolling buffers of recent packets for calculating windows

**Message passing:** Server broadcasts JSON snapshots of all `ClientMetrics` to dashboard WebSockets whenever updated (every 1 second).

## Server Implementation Details

### HTTP Endpoints (using `axum` framework)

- `GET /` - Serves client HTML page
- `GET /client.js` - Serves WASM loader JavaScript
- `GET /client_bg.wasm` - Serves WASM binary
- `POST /api/signaling/start` - Receives SDP offer, returns SDP answer
- `POST /api/signaling/ice` - Receives ICE candidates
- `GET /dashboard` - Serves dashboard HTML page
- `GET /api/dashboard/ws` - WebSocket upgrade for dashboard updates

### WebRTC Stack (`webrtc-rs` crate)

- Create `RTCPeerConnection` per client
- Configure STUN servers for NAT traversal
- Handle data channel events (open, message, close)
- Send probes on timer (tokio interval 50ms)
- Send bulk data continuously on bulk channel

### Dashboard Broadcasting

- Spawn task that runs every 1 second
- Collects all client metrics into JSON array
- Broadcasts to all connected dashboard WebSockets
- Message format: `{clients: [{id, metrics}, ...]}`

### Concurrency

- Main Tokio runtime handles HTTP/WebSocket
- Per-client tasks for sending probes and bulk data
- Shared state via `Arc<RwLock<HashMap<ClientId, ClientSession>>>`

## WASM Client Implementation Details

### Client Framework

- Uses `wasm-bindgen` + `web-sys` for browser APIs
- Entry point initializes UI and WebRTC connection
- Uses browser's native `RTCPeerConnection` via `web-sys` bindings
- Three data channels created and event handlers attached

### Client Workflow

1. **Initialize**: Create peer connection, add STUN servers
2. **Create channels**: Probe (unreliable), bulk (reliable), control (reliable)
3. **Signaling**: Generate offer, POST to `/api/signaling/start`, set answer
4. **ICE handling**: Gather candidates, POST to server
5. **On channels open**: Start measurement loops

### Measurement Tasks

Using `wasm-bindgen-futures`:
- **Probe sender**: `setInterval` at 50ms → send probe packet on probe channel
- **Probe receiver**: On message from probe channel → calculate delay, update metrics
- **Bulk receiver**: On message from bulk channel → count bytes, calculate throughput
- **Bulk sender**: Continuously send 1KB chunks on bulk channel
- **Stats reporter**: Every 1 second → send aggregated metrics on control channel

### UI Update

- Display local metrics in HTML table (client-side view)
- Update every 100ms for responsive feel
- Show all three time windows (1s, 10s, 60s) for each metric

## Error Handling & Edge Cases

### Connection Failures
- Client retry logic if signaling fails (exponential backoff)
- Server cleans up client state on data channel close
- Dashboard shows "disconnected" status for dropped clients

### Clock Skew
- One-way delay measurements unreliable if clocks differ
- Can fall back to RTT (echo packets) or show "clock skew detected"
- Jitter/reordering work fine without synchronized clocks

### Network Congestion
- Bulk channel may compete with probe channel
- Use separate channels to minimize interference
- Server adapts probe/bulk send rates if needed (future enhancement)

### Browser Limitations
- Some browsers limit data channel message size
- Keep packets < 16KB to avoid issues
- Handle WASM memory limits gracefully

### Dashboard Scaling
- Broadcast all clients to all dashboards works for <100 clients
- For larger deployments, could add filtering/pagination later

## Testing Strategy

- Manual testing with Chrome/Firefox
- Artificial delay/jitter/loss using `tc` on Linux server
- Multiple clients from different networks

## Technology Stack

### Server
- **Rust** (stable)
- **tokio** - async runtime
- **axum** - HTTP framework
- **webrtc-rs** - WebRTC implementation
- **tokio-tungstenite** - WebSocket support
- **serde/serde_json** - JSON serialization

### Client
- **Rust** → WASM via `wasm-bindgen`
- **web-sys** - Browser API bindings
- **wasm-bindgen-futures** - Async support in WASM
- **serde-wasm-bindgen** - JSON serialization

### Deployment
- Single binary server
- Static files (HTML/JS/WASM) embedded or served from disk
- STUN server configured (public or self-hosted)

## Data Persistence

**Approach**: In-memory only for v1
- Server keeps current client states in memory
- When client disconnects, history is lost
- Simple, fast, no database needed
- Focus on live monitoring
- Can add persistence later if historical analysis needed
