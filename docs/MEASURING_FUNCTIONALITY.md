# Network Measurement Functionality

This document provides detailed technical documentation for the wifi-verify network measurement system.

## Table of Contents

1. [Overview](#overview)
2. [System Architecture](#system-architecture)
3. [Measurement Phases](#measurement-phases)
4. [Data Channels](#data-channels)
5. [Probe Streams and Metrics](#probe-streams-and-metrics)
6. [Control Messages](#control-messages)
7. [Statistics and Metrics](#statistics-and-metrics)
8. [Client API Reference](#client-api-reference)
9. [Configuration](#configuration)
10. [Troubleshooting](#troubleshooting)

---

## Overview

The wifi-verify measurement system provides comprehensive network path analysis and continuous performance monitoring using WebRTC data channels. It measures key network quality metrics including:

- **Delay/Latency**: One-way and round-trip delay measurements
- **Jitter**: Delay variation between consecutive packets
- **Packet Loss**: Percentage of packets lost in transit
- **Packet Reordering**: Percentage of packets arriving out of order
- **Throughput**: Data transfer rate (when bulk channels are enabled)
- **Path MTU**: Maximum Transmission Unit along the network path
- **Traceroute**: Network hop-by-hop path discovery

The system supports simultaneous dual-stack (IPv4 and IPv6) measurements with multiple connections per address family for ECMP (Equal-Cost Multi-Path) testing.

---

## System Architecture

### Components

```
┌──────────────────────────────────────────────────────────────────┐
│                        Browser (WASM Client)                       │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐   │
│  │  IPv4 Conn(s)   │  │  IPv6 Conn(s)   │  │   UI Updates    │   │
│  │  - probe ch     │  │  - probe ch     │  │   - Metrics     │   │
│  │  - bulk ch      │  │  - bulk ch      │  │   - Charts      │   │
│  │  - control ch   │  │  - control ch   │  │   - Traceroute  │   │
│  │  - testprobe ch │  │  - testprobe ch │  │                 │   │
│  └────────┬────────┘  └────────┬────────┘  └────────┬────────┘   │
│           │                    │                    │             │
│           └────────────────────┼────────────────────┘             │
│                                │ WebRTC                           │
└────────────────────────────────┼──────────────────────────────────┘
                                 │
                                 ▼
┌────────────────────────────────────────────────────────────────────┐
│                          Rust Server                                │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    WebRTC Manager                            │   │
│  │  - Peer connections       - Data channel handlers           │   │
│  │  - ICE candidate handling - DTLS transport                  │   │
│  └─────────────────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                   Measurement Engine                         │   │
│  │  - Probe senders          - Metrics calculation             │   │
│  │  - Bulk data senders      - Statistics reporting            │   │
│  │  - Traceroute logic       - ICMP listener                   │   │
│  └─────────────────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    Packet Tracker                            │   │
│  │  - UDP packet tracking    - ICMP correlation                │   │
│  │  - TTL management         - MTU discovery                   │   │
│  └─────────────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────────┘
```

### Connection Model

Each measurement session consists of one or more WebRTC peer connections, each containing four data channels:

| Channel | Label | Reliability | Ordering | Purpose |
|---------|-------|-------------|----------|---------|
| Probe | `probe` | Unreliable | Unordered | High-frequency probe packets for latency/jitter/loss |
| Bulk | `bulk` | Unreliable | Unordered | Throughput measurement traffic |
| Control | `control` | Reliable | Ordered | Control messages and statistics |
| TestProbe | `testprobe` | Unreliable | Unordered | Traceroute and MTU discovery probes |

---

## Measurement Phases

The `analyze_network_with_count()` function orchestrates a multi-phase network analysis:

### Phase 0: Connection Establishment

1. Generate a unique survey session UUID for cross-correlation
2. Establish IPv4 and IPv6 WebRTC connections
3. Wait for all data channels to become ready
4. Send `StartSurveySession` message to server
5. Wait for `ServerSideReady` acknowledgment from all connections

**Timeout**: 30 seconds for all connections to become ready

### Phase 1: Traceroute (5 Rounds)

For each connection, the client triggers traceroute probes:

1. Send `StartTraceroute` control message
2. Server sends test probes with incrementing TTL (1 to 16)
3. ICMP Time Exceeded responses are captured and correlated
4. `TraceHop` messages are sent back to client with hop information
5. `TracerouteCompleted` signals end of round

**Configuration**:
- Maximum TTL: 16
- Probe interval: 50ms between TTL values
- Drain interval: 500ms for ICMP responses
- Stagger delay: 1000ms between connections

### Phase 2: MTU Traceroute (9 Rounds)

MTU discovery using the Don't Fragment (DF) bit:

1. Send `StartMtuTraceroute` with target packet size
2. Server sends large probes with DF bit set
3. ICMP Fragmentation Needed responses indicate MTU limits
4. `MtuHop` messages report discovered MTU values

**Packet Sizes Tested**: 576, 1280, 1350, 1400, 1450, 1472, 1490, 1500, 1500 bytes

**Note**: MTU probes bypass DTLS encryption and SCTP fragmentation for precise packet size control.

### Phase 3: Probe Stream Measurement

Continuous bidirectional probe streams for baseline measurement:

1. Clear previous metrics
2. Send `StartProbeStreams` to all connections
3. Start client-side probe sender (100 pps)
4. Start per-second statistics reporter
5. Begin chart data collection after 10-second delay

**Probe Rate**: 100 packets per second (10ms interval)

---

## Data Channels

### Probe Channel

Used for high-frequency measurement probes in two modes:

#### Regular Probe Mode
```rust
struct ProbePacket {
    seq: u64,           // Sequence number
    timestamp_ms: u64,  // Send timestamp (ms since epoch)
    direction: Direction,
    send_options: Option<SendOptions>,
    conn_id: String,    // Connection UUID
}
```

Probes are echoed back by the client to enable server-side RTT calculation.

#### Measurement Probe Mode
When probe streams are active, uses `MeasurementProbePacket`:
```rust
struct MeasurementProbePacket {
    seq: u64,
    sent_at_ms: u64,
    direction: Direction,
    conn_id: String,
    feedback: ProbeFeedback,  // Feedback about reverse direction
}

struct ProbeFeedback {
    highest_seq: u64,              // Highest sequence received
    highest_seq_received_at_ms: u64,
    recent_count: u32,             // Probes in last second
    recent_reorders: u32,          // Reorders in last second
}
```

### Bulk Channel

Used for throughput measurement (when enabled):

```rust
struct BulkPacket {
    data: Vec<u8>,      // Payload (typically 1024 bytes)
    send_options: Option<SendOptions>,
}
```

**Send Rate**: 100 Hz (10ms intervals)

### Control Channel

Reliable ordered channel for control messages. See [Control Messages](#control-messages) for the complete message catalog.

### TestProbe Channel

Used for traceroute and MTU discovery with special per-packet options:

```rust
struct TestProbePacket {
    test_seq: u64,
    timestamp_ms: u64,
    direction: Direction,
    send_options: Option<SendOptions>,
    conn_id: String,
}

struct SendOptions {
    ttl: Option<u8>,       // IP TTL/Hop Limit
    df_bit: Option<bool>,  // Don't Fragment (IPv4)
    tos: Option<u8>,       // Type of Service
    flow_label: Option<u32>, // IPv6 Flow Label
    track_for_ms: u32,     // ICMP tracking duration
    bypass_dtls: bool,     // Skip encryption (MTU tests)
    bypass_sctp_fragmentation: bool,
}
```

---

## Probe Streams and Metrics

### Probe Stream Constants

Defined in `common/src/protocol.rs`:

```rust
pub const PROBE_STREAM_PPS: u32 = 100;        // Packets per second
pub const PROBE_INTERVAL_MS: u32 = 10;        // 1000 / PPS
pub const BASELINE_MIN_SAMPLES: u64 = 10;     // Min samples for baseline
pub const BASELINE_OUTLIER_MULTIPLIER: f64 = 3.0;
pub const PROBE_STATS_WINDOW_MS: u64 = 2000;  // Stats calculation window
pub const PROBE_FEEDBACK_WINDOW_MS: u64 = 1000; // Feedback window
```

### Baseline Delay Calculation

The system calculates a baseline delay by:
1. Accumulating delay measurements
2. Excluding outliers > 3x the current baseline
3. Using exponential averaging

```rust
let baseline = baseline_delay_sum / baseline_delay_count;
if delay < baseline * BASELINE_OUTLIER_MULTIPLIER {
    baseline_delay_sum += delay;
    baseline_delay_count += 1;
}
```

### Direction Statistics

Each direction (C2S, S2C) reports:

```rust
struct DirectionStats {
    delay_deviation_ms: [f64; 4],  // [p50, p99, min, max]
    rtt_ms: [f64; 4],              // [p50, p99, min, max]
    jitter_ms: [f64; 4],           // [p50, p99, min, max]
    loss_rate: f64,                // Percentage
    reorder_rate: f64,             // Percentage
    probe_count: u32,
    baseline_delay_ms: f64,
}
```

**Delay Deviation**: Calculated as `observed_delay - baseline_delay`

**Jitter**: Absolute difference between consecutive probe delays

**Loss Rate**: `(expected - received) / expected * 100`

**Reorder Rate**: Packets arriving with sequence less than max seen

---

## Control Messages

All control messages use a tagged enum with `"type"` field for disambiguation:

```rust
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlMessage {
    // Client -> Server
    StartTraceroute(StartTracerouteMessage),
    StopTraceroute(StopTracerouteMessage),
    StartSurveySession(StartSurveySessionMessage),
    StartMtuTraceroute(StartMtuTracerouteMessage),
    GetMeasuringTime(GetMeasuringTimeMessage),
    StartServerTraffic(StartServerTrafficMessage),
    StopServerTraffic(StopServerTrafficMessage),
    StartProbeStreams(StartProbeStreamsMessage),
    StopProbeStreams(StopProbeStreamsMessage),
    TestProbeMessageEcho(TestProbePacket),
    ProbeStats(ProbeStatsReport),
    
    // Server -> Client
    ServerSideReady(ServerSideReadyMessage),
    TracerouteCompleted(TracerouteCompletedMessage),
    MtuTracerouteCompleted(MtuTracerouteCompletedMessage),
    TraceHop(TraceHopMessage),
    MtuHop(MtuHopMessage),
    MeasuringTimeResponse(MeasuringTimeResponseMessage),
}
```

### Message Details

#### StartSurveySession
```json
{
  "type": "start_survey_session",
  "survey_session_id": "uuid-string",
  "conn_id": "uuid-string"
}
```
Initiates a survey session for cross-correlation across connections.

#### StartTraceroute
```json
{
  "type": "start_traceroute",
  "conn_id": "uuid-string",
  "survey_session_id": "uuid-string"
}
```
Triggers a single round of traceroute probes.

#### StartMtuTraceroute
```json
{
  "type": "start_mtu_traceroute",
  "conn_id": "uuid-string",
  "survey_session_id": "uuid-string",
  "packet_size": 1500,
  "path_ttl": 16,
  "collect_timeout_ms": 3000
}
```
Triggers MTU discovery with specified packet size.

#### StartProbeStreams
```json
{
  "type": "start_probe_streams",
  "conn_id": "uuid-string",
  "survey_session_id": "uuid-string"
}
```
Starts bidirectional probe streams at 100 pps.

#### ProbeStatsReport
```json
{
  "type": "probe_stats",
  "conn_id": "uuid-string",
  "survey_session_id": "uuid-string",
  "timestamp_ms": 1234567890,
  "c2s_stats": { /* DirectionStats */ },
  "s2c_stats": { /* DirectionStats */ }
}
```
Per-second statistics report from both endpoints.

#### TraceHop
```json
{
  "type": "trace_hop",
  "hop": 5,
  "ip_address": "192.168.1.1",
  "rtt_ms": 12.5,
  "message": "Hop 5 via 192.168.1.1 (12.50ms)",
  "conn_id": "uuid-string",
  "survey_session_id": "uuid-string",
  "original_src_port": 12345,
  "original_dest_addr": "10.0.0.1:443"
}
```
Reports a discovered network hop.

#### MtuHop
```json
{
  "type": "mtu_hop",
  "hop": 3,
  "ip_address": "10.0.0.1",
  "rtt_ms": 8.2,
  "mtu": 1480,
  "message": "MTU probe hop 3 (size 1500)",
  "conn_id": "uuid-string",
  "survey_session_id": "uuid-string",
  "packet_size": 1500
}
```
Reports MTU discovery result with optional MTU value from ICMP.

---

## Statistics and Metrics

### ClientMetrics Structure

```rust
struct ClientMetrics {
    // Throughput (bytes/sec) for [1s, 10s, 60s] windows
    c2s_throughput: [f64; 3],
    s2c_throughput: [f64; 3],
    
    // Average delay (ms)
    c2s_delay_avg: [f64; 3],
    s2c_delay_avg: [f64; 3],
    
    // Jitter - std dev of delay (ms)
    c2s_jitter: [f64; 3],
    s2c_jitter: [f64; 3],
    
    // Loss rate (percentage)
    c2s_loss_rate: [f64; 3],
    s2c_loss_rate: [f64; 3],
    
    // Reordering rate (percentage)
    c2s_reorder_rate: [f64; 3],
    s2c_reorder_rate: [f64; 3],
}
```

### Metric Calculation

Metrics are calculated over three time windows:

| Window | Duration | Use Case |
|--------|----------|----------|
| 1s | 1,000ms | Real-time feedback |
| 10s | 10,000ms | Short-term trends |
| 60s | 60,000ms | Long-term averages |

#### Delay Calculation
For client-to-server:
```rust
delay = received_at_ms - sent_at_ms
```

For server-to-client (echo-based):
```rust
delay = echoed_at_ms - sent_at_ms
```

**Note**: One-way delay accuracy depends on clock synchronization between client and server.

#### Jitter Calculation
Standard deviation of delay values:
```rust
avg_delay = sum(delays) / count
variance = sum((d - avg_delay)^2) / count
jitter = sqrt(variance)
```

#### Loss Calculation
Based on sequence number gaps:
```rust
expected = max_seq - min_seq + 1
loss_rate = (expected - received) / expected * 100
```

#### Reorder Detection
A packet is reordered if its sequence number is less than the maximum seen:
```rust
if packet.seq < max_seq_seen {
    reorders += 1
}
max_seq_seen = max(max_seq_seen, packet.seq)
```

---

## Client API Reference

### JavaScript Entry Points

The WASM client exports these functions:

#### `start_measurement()`
```javascript
await wasm.start_measurement();
```
Starts dual-stack measurement with 1 connection per address family.

#### `start_measurement_with_count(conn_count)`
```javascript
await wasm.start_measurement_with_count(4);
```
Starts measurement with multiple connections (1-16 per address family).

#### `analyze_network()`
```javascript
await wasm.analyze_network();
```
Full network analysis: connection → traceroute → MTU → measurement.

#### `analyze_network_with_count(conn_count)`
```javascript
await wasm.analyze_network_with_count(4);
```
Full analysis with multiple connections for ECMP testing.

#### `stop_testing()`
```javascript
wasm.stop_testing();
```
Stops all active testing and closes connections.

#### `is_testing_active()`
```javascript
if (wasm.is_testing_active()) {
    // Testing is running
}
```

### JavaScript Callbacks

The client calls these JavaScript functions for UI updates:

#### `registerPeerConnection(ipVersion, connIndex, connId, localAddr, remoteAddr)`
Called when a new connection is established.

#### `updateConnectionMetrics(ipVersion, connIndex, metricsObject)`
Called every 500ms with per-connection metrics.

#### `addMetricsData(ipv4Metrics, ipv6Metrics)`
Called for chart data updates.

#### `addTracerouteHop(hopData)`
Called when a traceroute hop is discovered.

#### `addMtuHop(mtuData)`
Called when MTU information is discovered.

#### `updateProbeStats(statsReport)`
Called with per-second probe statistics.

---

## Configuration

### Server Configuration

The server reads configuration from `server_config.toml`:

```toml
[client]
# Delay between WebRTC connection attempts (ms)
webrtc_connection_delay_ms = 50
```

The client fetches configuration via `GET /api/config/client`.

### Client Constants

In `client/src/lib.rs`:

```rust
// Traceroute configuration
const TRACEROUTE_ROUNDS: u32 = 3;
const TRACEROUTE_ROUND_MIN_WAIT_MS: u32 = 3000;
const DEFAULT_TRACEROUTE_STAGGER_DELAY_MS: u32 = 1000;

// MTU Traceroute configuration
const MTU_TRACEROUTE_ROUNDS: u32 = 9;
const MTU_TRACEROUTE_ROUND_MIN_WAIT_MS: u32 = 500;
const MTU_SIZES: [u32; 9] = [576, 1280, 1350, 1400, 1450, 1472, 1490, 1500, 1500];

// Measurement configuration
const CHART_COLLECTION_DELAY_MS: u64 = 10000;
const CONTROL_CHANNEL_READY_TIMEOUT_MS: u32 = 2000;
```

### Server Constants

In `server/src/data_channels.rs`:

```rust
const DEFAULT_MEASURING_TIME_MS: u64 = 10_000_000;  // ~2.7 hours
```

In `server/src/measurements.rs`:

```rust
const MAX_TTL: u8 = 16;
const TRC_SEND_INTERVAL_MS: u64 = 50;
const TRC_DRAIN_INTERVAL_MS: u64 = 500;
```

---

## Troubleshooting

### Common Issues

#### No Metrics Data
1. Check that all data channels are open (probe, bulk, control, testprobe)
2. Verify `ServerSideReady` message was received
3. Check browser console for WebRTC errors
4. Verify STUN server connectivity

#### High Packet Loss During Traceroute
This is expected behavior. Traceroute intentionally uses low TTL values that cause packets to expire at intermediate routers.

#### MTU Discovery Not Working
1. Verify the server is running on Linux (required for per-packet socket options)
2. Check that ICMP "Fragmentation Needed" messages are not blocked by firewalls
3. Review server logs for ICMP listener errors

#### IPv6 Connections Failing
1. Verify IPv6 connectivity to the server
2. Check that the server binds to IPv6 addresses
3. Some networks may not support IPv6

#### Clock Skew Affecting Delay Measurements
One-way delay measurements require synchronized clocks. Large skew will show as:
- Negative delay values (client clock ahead of server)
- Unrealistically high delay values (server clock ahead of client)

The system uses RTT-based measurements where possible to avoid clock skew issues.

### Debug Logging

Enable debug logging in the browser:
```javascript
wasm_logger::init(wasm_logger::Config::default());
```

Server-side logging uses the `tracing` crate:
```rust
RUST_LOG=debug cargo run -p wifi-verify-server
```

### Diagnostics Endpoint

The server provides diagnostics at `GET /api/diagnostics`:
- Connection states
- ICE candidate information
- Data channel status
- ICMP error counts

See [DIAGNOSTICS.md](DIAGNOSTICS.md) for details.

---

## See Also

- [UDP_PACKET_OPTIONS.md](UDP_PACKET_OPTIONS.md) - Per-packet socket options documentation
- [DIAGNOSTICS.md](DIAGNOSTICS.md) - Server diagnostics endpoint
- [DTLS_BYPASS_FEATURE.md](DTLS_BYPASS_FEATURE.md) - DTLS bypass for MTU tests
- [SCTP_FRAGMENTATION_BYPASS.md](SCTP_FRAGMENTATION_BYPASS.md) - SCTP fragmentation bypass
- [VERIFICATION_GUIDE.md](VERIFICATION_GUIDE.md) - Verifying traceroute functionality
