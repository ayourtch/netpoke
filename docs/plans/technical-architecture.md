# NetPoke: Technical Architecture

## System Overview

NetPoke is a distributed system with a Rust-based server and WebAssembly client communicating via WebRTC data channels.

```
┌──────────────────────────────────────────────────────────────────────────┐
│                         Browser (WASM Client)                             │
│  ┌─────────────────┐  ┌─────────────────┐  ┌──────────────────────────┐  │
│  │  IPv4 Conn(s)   │  │  IPv6 Conn(s)   │  │     UI Components        │  │
│  │  - probe ch     │  │  - probe ch     │  │  - Metrics Display       │  │
│  │  - bulk ch      │  │  - bulk ch      │  │  - Charts (Chart.js)     │  │
│  │  - control ch   │  │  - control ch   │  │  - Traceroute Viz        │  │
│  │  - testprobe ch │  │  - testprobe ch │  │  - Survey Capture        │  │
│  └────────┬────────┘  └────────┬────────┘  └──────────────────────────┘  │
│           └────────────────────┼─────────────────────────────────────────│
│                                │ WebRTC Data Channels                    │
└────────────────────────────────┼─────────────────────────────────────────┘
                                 │
                                 ▼
┌──────────────────────────────────────────────────────────────────────────┐
│                        Rust Server (Tokio)                                │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │                      WebRTC Manager                                 │  │
│  │  - Peer connection lifecycle    - ICE candidate handling           │  │
│  │  - Data channel management      - DTLS transport                   │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │                    Measurement Engine                               │  │
│  │  - Probe packet sender          - Metrics calculation              │  │
│  │  - Bulk data sender             - Statistics aggregation           │  │
│  │  - Traceroute logic             - ICMP listener                    │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │                     Packet Tracker                                  │  │
│  │  - UDP packet tracking          - ICMP correlation                 │  │
│  │  - TTL management               - MTU discovery                    │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │                   Additional Services                               │  │
│  │  - iperf3 server                - Packet capture (PCAP)            │  │
│  │  - Authentication               - Tracing/logging buffer           │  │
│  │  - Dashboard WebSocket          - Survey data storage              │  │
│  └────────────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## Component Architecture

### Crate Structure

```
netpoke/
├── server/              # Main server application
│   ├── src/
│   │   ├── main.rs              # Entry point, Axum router setup
│   │   ├── signaling.rs         # WebRTC signaling (SDP/ICE exchange)
│   │   ├── webrtc_manager.rs    # Peer connection lifecycle
│   │   ├── data_channels.rs     # Data channel message handling
│   │   ├── measurements.rs      # Probe sending, traceroute, MTU
│   │   ├── packet_tracker.rs    # ICMP correlation, packet tracking
│   │   ├── state.rs             # Session state management
│   │   ├── dashboard.rs         # WebSocket dashboard
│   │   ├── survey_middleware.rs # Magic Key authentication
│   │   └── iperf3_server.rs     # Built-in iperf3
│   └── static/                  # Static web assets
│       ├── nettest.html         # Main test interface
│       ├── camera-tracker.html  # Survey capture prototype
│       └── dashboard.html       # Real-time dashboard
│
├── client/              # WebAssembly client
│   └── src/
│       ├── lib.rs               # WASM entry point
│       ├── webrtc.rs            # WebRTC connection management
│       ├── metrics.rs           # Client-side metric calculation
│       └── ui.rs                # DOM interaction
│
├── common/              # Shared types
│   └── src/
│       ├── lib.rs
│       └── protocol.rs          # Message definitions
│
├── netpoke-auth/    # Authentication library
│   └── src/
│       ├── lib.rs               # Auth state, session management
│       ├── config.rs            # Auth configuration
│       ├── oauth/               # OAuth2 providers
│       └── bluesky.rs           # Bluesky DPoP authentication
│
└── vendored/            # Modified WebRTC crates
    ├── webrtc/
    ├── webrtc-data/
    ├── webrtc-sctp/
    ├── webrtc-util/
    ├── dtls/
    └── webrtc-ice/
```

---

## WebRTC Data Channels

### Channel Configuration

| Channel | Label | Reliability | Ordering | Purpose |
|---------|-------|-------------|----------|---------|
| **Probe** | `probe` | Unreliable | Unordered | High-frequency probes (100 pps) for latency/jitter/loss |
| **Bulk** | `bulk` | Unreliable | Unordered | Throughput measurement (1KB chunks) |
| **Control** | `control` | Reliable | Ordered | Control messages and statistics |
| **TestProbe** | `testprobe` | Unreliable | Unordered | Traceroute and MTU discovery probes |

### Channel Separation Rationale

- **Probe channel**: Unreliable for accurate loss measurement (no retransmission)
- **Bulk channel**: Separate to avoid probe timing interference during throughput tests
- **Control channel**: Reliable for commands that must arrive (start/stop, config)
- **TestProbe channel**: Separate for traceroute packets with varying TTL

---

## Protocol Messages

### Control Channel Messages

Defined in `common/src/protocol.rs`:

```rust
pub enum ControlMessage {
    // Session management
    StartSurveySession(StartSurveySessionMessage),
    
    // Traceroute
    StartTraceroute(StartTracerouteMessage),
    StopTraceroute(StopTracerouteMessage),
    TracerouteHop(TracerouteHopMessage),
    TracerouteComplete(TracerouteCompleteMessage),
    
    // MTU Discovery
    StartMtuTraceroute(StartMtuTracerouteMessage),
    MtuHop(MtuHopMessage),
    MtuComplete(MtuCompleteMessage),
    
    // Traffic generation
    StartServerTraffic(StartServerTrafficMessage),
    StopServerTraffic(StopServerTrafficMessage),
    
    // Probe streams
    StartProbeStreams(StartProbeStreamsMessage),
    StopProbeStreams(StopProbeStreamsMessage),
    
    // Metrics
    GetMeasuringTime(GetMeasuringTimeMessage),
    MeasuringTime(MeasuringTimeMessage),
    ProbeStats(ProbeStatsMessage),
}
```

### Survey Session ID

All messages include `survey_session_id` for cross-correlation:
- Links all measurements to a single survey session
- Enables correlation with camera/sensor data captured client-side
- Used for organizing data under projects

---

## Measurement Engine

### Probe Packet Flow

```
Client                          Server
  │                               │
  │  ──── Probe Packet ────────>  │  (sequence #, timestamp)
  │                               │
  │  <──── Probe Response ─────   │  (echo sequence, server timestamp)
  │                               │
  │  Calculate RTT, one-way delay │
  │  Detect loss via sequence gaps│
  │  Calculate jitter from deltas │
```

### Traceroute Flow

```
Client                          Server                      Network
  │                               │                            │
  │  StartTraceroute ──────────>  │                            │
  │                               │                            │
  │                               │  Send probe TTL=1 ──────>  │
  │                               │                            │
  │                               │  <── ICMP Time Exceeded    │
  │                               │      (from hop 1)          │
  │                               │                            │
  │  <──── TracerouteHop ───────  │  (hop 1 info)              │
  │                               │                            │
  │                               │  Send probe TTL=2 ──────>  │
  │                               │                            │
  │                               │  <── ICMP Time Exceeded    │
  │                               │      (from hop 2)          │
  │                               │                            │
  │  <──── TracerouteHop ───────  │  (hop 2 info)              │
  │                               │                            │
  │            ...                │           ...              │
  │                               │                            │
  │  <──── TracerouteComplete ──  │  (all hops discovered)     │
```

### Per-Packet Socket Options

The vendored WebRTC stack enables setting options on each UDP message:

```rust
pub struct UdpSendOptions {
    pub ttl: Option<u8>,           // Time-to-live / hop limit
    pub dont_fragment: Option<bool>, // DF bit for MTU discovery
    pub tos: Option<u8>,           // Type of Service / DSCP
    pub flow_label: Option<u32>,   // IPv6 flow label
}
```

Implementation uses Linux `sendmsg()` with control messages (ancillary data).

---

## Session State

### ClientSession Structure

```rust
pub struct ClientSession {
    pub id: String,                              // Unique session ID
    pub conn_id: String,                         // Connection identifier
    pub survey_session_id: Arc<RwLock<String>>,  // Current survey session
    pub peer_connection: Arc<RTCPeerConnection>,
    pub data_channels: DataChannels,
    pub metrics: Arc<RwLock<SessionMetrics>>,
    pub packet_tracker: Arc<PacketTracker>,
    // ... additional fields
}
```

### Session Lifecycle

1. **Signaling**: Client initiates via `/api/signaling/start`
2. **ICE Exchange**: Candidates exchanged via `/api/signaling/ice`
3. **Connection**: WebRTC peer connection established
4. **Channels Open**: Data channels become available
5. **Measurement**: Client sends commands, server responds
6. **Cleanup**: Session removed on disconnect or timeout

---

## Authentication Architecture

### Middleware Stack

```
Request
   │
   ▼
┌─────────────────────────────────┐
│  survey_middleware              │  ← Checks Magic Key OR regular auth
│  (require_auth_or_survey_session)│
└─────────────────────────────────┘
   │
   ▼
┌─────────────────────────────────┐
│  netpoke-auth               │  ← Session validation
│  (AuthState)                    │
└─────────────────────────────────┘
   │
   ▼
┌─────────────────────────────────┐
│  Route Handler                  │
└─────────────────────────────────┘
```

### Magic Key Session Format

```
survey_{magic_key}_{timestamp}_{uuid}
```

- `magic_key`: The key from configuration (may contain hyphens)
- `timestamp`: Unix timestamp of session creation
- `uuid`: Random UUID for uniqueness

Validation checks:
1. Format is correct
2. Magic key is in allowed list
3. Session hasn't expired (configurable timeout)

---

## Data Flow: Survey Mode

### Current Prototype (camera-tracker.html)

```
┌────────────────────────────────────────────────────────────────┐
│                    Browser (Survey Mode)                        │
│                                                                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │
│  │   Camera    │  │   Sensors   │  │   WebRTC Measurements   │ │
│  │   Stream    │  │   (GPS,     │  │   (latency, jitter,     │ │
│  │             │  │   Accel,    │  │    loss, traceroute)    │ │
│  │             │  │   Compass)  │  │                         │ │
│  └──────┬──────┘  └──────┬──────┘  └────────────┬────────────┘ │
│         │                │                      │               │
│         └────────────────┼──────────────────────┘               │
│                          │                                      │
│                          ▼                                      │
│              ┌───────────────────────┐                          │
│              │  IndexedDB Storage    │                          │
│              │  (local, offline)     │                          │
│              └───────────┬───────────┘                          │
│                          │                                      │
│                          ▼                                      │
│              ┌───────────────────────┐                          │
│              │  Upload to Server     │  ← Future: automatic    │
│              │  (manual download now)│                          │
│              └───────────────────────┘                          │
└────────────────────────────────────────────────────────────────┘
```

### Planned Architecture

See [Survey Feature Spec](survey-feature-spec.md) for full details.

---

## Server Configuration

### Main Configuration File (`server_config.toml`)

```toml
[server]
enable_http = true
http_port = 3000
enable_https = true
https_port = 3443

[auth]
enable_auth = true
allowed_users = ["admin@example.com"]

[auth.oauth]
enable_github = true
enable_google = true
enable_bluesky = true

[auth.magic_keys]
enabled = true
survey_cookie_name = "survey_session"
survey_timeout_seconds = 86400  # 24 hours
magic_keys = ["field-survey-2024", "remote-diag-key"]

[iperf3]
enabled = true
port = 5201
require_auth = true
max_sessions = 10
max_duration_seconds = 300

[capture]
enabled = true
max_packets = 100000

[tracing]
enabled = true
max_log_entries = 10000

[client]
webrtc_connection_delay_ms = 50
```

---

## Deployment Architecture

### Single Server

```
┌─────────────────────────────────────────────┐
│              Linux Server                    │
│                                             │
│  ┌─────────────────────────────────────┐   │
│  │       NetPoke Server            │   │
│  │  - HTTP/HTTPS (Axum)                │   │
│  │  - WebRTC (webrtc-rs)               │   │
│  │  - ICMP Listener (raw socket)       │   │
│  │  - iperf3 Server                    │   │
│  └─────────────────────────────────────┘   │
│                                             │
│  Requirements:                              │
│  - CAP_NET_RAW for ICMP                    │
│  - Linux kernel (sendmsg with cmsg)        │
└─────────────────────────────────────────────┘
```

### Future: Distributed Architecture

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Edge Node  │     │  Edge Node  │     │  Edge Node  │
│  (Region A) │     │  (Region B) │     │  (Region C) │
└──────┬──────┘     └──────┬──────┘     └──────┬──────┘
       │                   │                   │
       └───────────────────┼───────────────────┘
                           │
                           ▼
               ┌───────────────────────┐
               │   Central Platform    │
               │   - User management   │
               │   - Survey storage    │
               │   - Analytics         │
               └───────────────────────┘
```

---

## Security Considerations

### Network Security
- HTTPS for all API endpoints
- DTLS for WebRTC data channels
- Private cookies for session data

### Authentication Security
- bcrypt for password hashing
- OAuth2 with PKCE for third-party auth
- DPoP tokens for Bluesky
- Time-limited Magic Keys

### ICMP Listener
- Requires CAP_NET_RAW or root
- Only listens for ICMP responses to our packets
- Correlates via tracked packet database

### Data Privacy
- Survey data associated with sessions
- Configurable retention periods
- No PII in network measurements

---

## Performance Characteristics

### Probe Rate
- Default: 100 packets/second
- Configurable per session
- Low bandwidth overhead (~10 KB/s)

### Traceroute
- 5 rounds per TTL value
- Timeout: configurable (default 2s per hop)
- Typically completes in 10-30 seconds

### Memory Usage
- Per-session overhead: ~1-2 MB
- Packet tracker: ring buffer, bounded
- Capture buffer: configurable max packets

### Concurrency
- Tokio async runtime
- Multiple concurrent sessions supported
- WebSocket dashboard scales to many viewers
