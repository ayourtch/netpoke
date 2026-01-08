# WiFi-Verify: Product Overview

## Executive Summary

WiFi-Verify is a browser-based network measurement and survey platform that measures **end-to-end application-layer network performance**. Unlike traditional WiFi survey tools that focus on radio frequency (RF) metrics, WiFi-Verify measures what applications actually experience: latency, jitter, packet loss, path characteristics, and throughput.

**Positioning**: WiFi-Verify is complementary to RF survey tools like Ekahau. While Ekahau answers "How strong is the WiFi signal?", WiFi-Verify answers "How well do applications actually perform over this network?"

## Core Value Proposition

> "Measure what your applications actually experience, not just what the radio sees."

### The Problem We Solve

1. **RF metrics don't tell the whole story** - A strong WiFi signal doesn't guarantee good application performance
2. **End-to-end path matters** - Problems may exist beyond the access point (backhaul, internet, DNS, routing)
3. **Traditional tools require installation** - Native apps, agents, or specialized hardware
4. **Survey tools are expensive** - Ekahau + Sidekick costs $8,000+
5. **Remote diagnostics are difficult** - Can't easily test someone else's network

### Our Solution

A zero-install, browser-based platform that:
- Measures true end-to-end network quality via WebRTC
- Performs traceroute and MTU discovery from the browser
- Captures sensor data (GPS, accelerometer, compass) for location context
- Records video for visual documentation
- Correlates all data for comprehensive network surveys
- Enables remote testing via shareable Magic Key links

---

## Core Functionality

### 1. Real-Time Network Measurement

WiFi-Verify measures key network quality metrics continuously:

| Metric | Description | Measurement Method |
|--------|-------------|-------------------|
| **Latency (RTT)** | Round-trip time to server | WebRTC probe packets at 100 pps |
| **One-way Delay** | Directional latency measurement | NTP-synchronized timestamps |
| **Jitter** | Delay variation between packets | Inter-packet arrival analysis |
| **Packet Loss** | Percentage of lost packets | Sequence number gap detection |
| **Reordering** | Out-of-sequence packet delivery | Sequence analysis |
| **Throughput** | Achievable data rate | Bulk data channel flooding |

**Statistical Windows**: Metrics calculated over 1s, 10s, and 60s windows with percentiles (min, 50th, 99th, max).

### 2. Network Path Analysis

#### Traceroute via WebRTC
WiFi-Verify performs browser-based traceroute by:
1. Sending probe packets with incrementing TTL values
2. Server-side ICMP listener captures "Time Exceeded" responses
3. Correlating ICMP responses with tracked UDP packets
4. Returning hop-by-hop path information to browser

**Unique capability**: No other browser-based tool can perform real traceroute.

#### Path MTU Discovery
Discovers the Maximum Transmission Unit along the network path:
1. Sends packets of varying sizes (576-1500 bytes) with DF (Don't Fragment) bit set
2. Captures ICMP "Fragmentation Needed" responses
3. Determines MTU at each hop
4. Identifies MTU bottlenecks causing fragmentation

#### ECMP Path Detection
Detects Equal-Cost Multi-Path routing by:
1. Establishing 1-16 concurrent WebRTC connections
2. Each connection may take a different path through load balancers
3. Comparing traceroute results across connections
4. Highlighting path divergence points

### 3. Dual-Stack Testing

Simultaneous IPv4 and IPv6 measurement:
- Separate connections for each address family
- Side-by-side metric comparison
- Identifies protocol-specific issues
- Tests both paths through dual-stack infrastructure

### 4. Integrated iperf3 Server

Built-in iperf3-compatible server for traditional bandwidth testing:
- Authentication-gated access
- Session management with limits
- Complements WebRTC measurements with TCP/UDP throughput tests

### 5. Dashboard & Visualization

Real-time monitoring interface:
- WebSocket-based live updates
- Chart.js time-series graphs (zoomable/pannable)
- Traceroute path visualization
- Server diagnostics endpoint
- Packet capture (PCAP) download
- Tracing log download

---

## Technical Differentiators

### Modified WebRTC Stack

WiFi-Verify uses a **vendored and modified WebRTC implementation** (6 crates) to enable capabilities impossible with standard WebRTC:

#### Per-Packet Socket Options
Control UDP packet attributes on every message:
- **TTL/Hop Limit**: Set Time-To-Live for traceroute
- **DF (Don't Fragment)**: Enable for MTU discovery
- **TOS/DSCP**: Set QoS markers
- **IPv6 Flow Label**: Control routing behavior

#### DTLS Bypass
For MTU testing, bypass DTLS encryption overhead (13-29 bytes) to send exact packet sizes.

#### SCTP Fragmentation Bypass
SCTP normally fragments messages >1200 bytes. Bypass allows sending full MTU-sized packets for accurate testing.

### Browser-Native Implementation

- **Rust + WebAssembly**: High-performance client compiled to WASM
- **Zero installation**: Works in any modern browser
- **Cross-platform**: Desktop, mobile, tablet
- **No extensions**: Pure web standards (WebRTC, IndexedDB, Sensors API)

---

## Authentication & Access Control

### Multi-Provider Authentication

| Method | Use Case |
|--------|----------|
| **Plain Login** | Username/password with bcrypt |
| **OAuth2 (GitHub)** | Developer-friendly login |
| **OAuth2 (Google)** | Enterprise/consumer login |
| **OAuth2 (LinkedIn)** | Professional identity |
| **Bluesky OAuth** | Decentralized identity with DPoP |

### Magic Key System

Time-limited access for field surveys without full accounts:
- Generate shareable links with embedded Magic Key
- Configurable expiration (hours/days)
- Access limited to network testing and survey upload
- Ideal for:
  - Field technicians performing surveys
  - Remote users diagnosing their own network
  - Customers providing network data to support

---

## Deployment Requirements

### Server
- Linux (required for per-packet UDP options via `sendmsg()`)
- Rust stable toolchain
- Root or CAP_NET_RAW for ICMP listening
- Optional: SSL certificates for HTTPS

### Client
- Modern browser with WebRTC support
- No installation required
- Works on mobile devices

### Configuration
See `server_config.toml` for full options including:
- HTTP/HTTPS ports
- Authentication providers
- iperf3 settings
- Capture buffer sizes
- Client configuration

---

## API Endpoints

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/api/signaling/start` | POST | WebRTC signaling - SDP exchange |
| `/api/signaling/ice` | POST | ICE candidate exchange |
| `/api/dashboard/ws` | GET | WebSocket for live dashboard |
| `/api/diagnostics` | GET | Server and session diagnostics |
| `/api/capture/download` | GET | Download PCAP file |
| `/api/tracing/download` | GET | Download tracing logs |
| `/api/config/client` | GET | Client configuration |
| `/auth/*` | Various | Authentication endpoints |

---

## Use Cases

### 1. Network Quality Assessment
Evaluate WiFi and wired network performance for real application behavior, not just RF metrics.

### 2. Path Troubleshooting
Identify network hops causing latency, loss, or MTU issues using browser-based traceroute.

### 3. Remote Diagnostics
Send a Magic Key link to users experiencing issues; get full diagnostics without site visits.

### 4. WiFi Survey (Walk-through)
Capture video, sensor data, and network metrics simultaneously while walking through a space. See [Survey Feature Spec](survey-feature-spec.md).

### 5. Dual-Stack Validation
Compare IPv4 vs IPv6 performance for network infrastructure validation.

### 6. WebRTC Application Testing
Developers testing video conferencing or real-time apps get actual network conditions, not simulated.

### 7. Continuous Monitoring
Long-running measurements with real-time visualization for ongoing network quality tracking.

---

## Relationship to RF Survey Tools

WiFi-Verify is **complementary** to RF survey tools, not a replacement:

| Aspect | RF Tools (Ekahau, NetSpot) | WiFi-Verify |
|--------|---------------------------|-------------|
| **Measures** | Radio signal strength, noise, interference | Application-layer performance |
| **Answers** | "Is there WiFi coverage?" | "Do apps work well here?" |
| **Layer** | Physical/Link (L1/L2) | Transport/Application (L4/L7) |
| **Scope** | Access point to client | End-to-end (client to server/internet) |
| **Hardware** | Specialized adapters often required | Any device with browser |
| **Cost** | $500-$15,000+ | $49-499/month |

### Combined Workflow

1. **RF Survey (Ekahau)**: Map signal coverage, identify dead spots, plan AP placement
2. **Application Survey (WiFi-Verify)**: Validate actual performance, test backhaul, verify end-to-end quality

### Future Integration
Screen capture of Ekahau survey window alongside WiFi-Verify camera view for comprehensive documentation. See [Survey Feature Spec](survey-feature-spec.md).

---

## Summary

WiFi-Verify fills a critical gap in network diagnostics:
- **More than consumer speed tests** (traceroute, MTU, ECMP, jitter, loss)
- **Less complex than enterprise NPM** (no agents, no infrastructure)
- **Complementary to RF tools** (application-layer vs radio-layer)
- **Zero installation** (browser-based, shareable links)
- **Unique technical capabilities** (modified WebRTC stack)

The platform enables anyone to measure what their network actually delivers to applications, from anywhere, without specialized tools or training.
