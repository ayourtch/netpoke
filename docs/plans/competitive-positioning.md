# NetPoke: Competitive Positioning

## Market Positioning

NetPoke occupies a unique position in the network measurement landscape, sitting at the intersection of several categories without directly competing with any of them.

```
                        Complexity / Cost
                              ▲
                              │
        Enterprise NPM        │        NetPoke
        (ThousandEyes,        │        sits HERE
         Catchpoint)          │            ↓
        $20K-100K/yr          │        ┌─────────┐
                              │        │ WiFi-   │
                              │        │ Verify  │
                              │        └─────────┘
                              │
        RF Survey Tools       │
        (Ekahau, NetSpot)     │
        $500-15K              │
                              │
                              │
        Consumer Speed Tests  │
        (Speedtest, Fast.com) │
        Free                  │
                              │
        ──────────────────────┼──────────────────────────►
              Signal/Speed    │    Application-Layer
              Metrics Only    │    Full Path Analysis
                              │
                        Measurement Depth
```

---

## Competitive Landscape

### Category 1: Consumer Speed Tests

#### Competitors
- **Speedtest by Ookla** - Market leader, 65B+ tests, brand recognition
- **Fast.com by Netflix** - Simple, fast, trusted
- **Cloudflare Speed Test** - Privacy-focused, edge network
- **Meter.net** - European focus, embeddable

#### How They Compare

| Capability | Speedtest | Fast.com | Cloudflare | NetPoke |
|------------|-----------|----------|------------|-------------|
| Download speed | ✅ | ✅ | ✅ | ✅ |
| Upload speed | ✅ | ✅ | ✅ | ✅ |
| Ping/Latency | ✅ | ✅ | ✅ | ✅ |
| Jitter | ❌ | ❌ | ✅ | ✅ |
| Packet loss | ❌ | ❌ | ✅ | ✅ |
| Traceroute | ❌ | ❌ | ❌ | ✅ |
| MTU discovery | ❌ | ❌ | ❌ | ✅ |
| ECMP detection | ❌ | ❌ | ❌ | ✅ |
| Dual-stack (IPv4/IPv6) | Limited | ❌ | ✅ | ✅ |
| Video + sensor capture | ❌ | ❌ | ❌ | ✅ |
| Walk-through survey | ❌ | ❌ | ❌ | ✅ |

#### Positioning vs Consumer Speed Tests

> "Speedtest tells you how fast. NetPoke tells you why."

**Key Differentiators**:
1. Traceroute shows the full path, not just endpoint speed
2. Jitter and packet loss matter more than speed for real-time apps
3. MTU discovery finds fragmentation issues invisible to speed tests
4. Survey mode enables systematic assessment, not just point-in-time

**When to Use Each**:
- **Speedtest**: Quick "is my internet working?" check
- **NetPoke**: Diagnosing why video calls drop, finding network problems

---

### Category 2: RF Survey Tools

#### Competitors
- **Ekahau Pro + Sidekick** - Industry standard, $5K software + $3K hardware
- **NetSpot** - Mac/Windows, $500-1,500, consumer-friendly
- **iBwave** - Enterprise, predictive modeling
- **AirMagnet** - NetAlly (formerly Fluke), hardware-based

#### How They Compare

| Capability | Ekahau | NetSpot | NetPoke |
|------------|--------|---------|-------------|
| **Primary Measurement** | RF signal | RF signal | Application performance |
| Signal strength heatmap | ✅ | ✅ | ❌ |
| Channel utilization | ✅ | ✅ | ❌ |
| Interference detection | ✅ | ✅ | ❌ |
| AP placement planning | ✅ | ✅ | ❌ |
| End-to-end latency | ❌ | ❌ | ✅ |
| Packet loss | ❌ | ❌ | ✅ |
| Traceroute/path analysis | ❌ | ❌ | ✅ |
| Backhaul/internet issues | ❌ | ❌ | ✅ |
| Hardware required | Yes ($3K+) | Optional | No |
| Mobile device support | Limited | ❌ | ✅ (any browser) |
| Price | $5K-15K | $500-1.5K | $49-499/mo |

#### Positioning vs RF Survey Tools

> "Ekahau measures radio. NetPoke measures reality."

**NetPoke is COMPLEMENTARY, not competitive**:

RF survey tools answer:
- "Is there WiFi signal here?"
- "What channel should this AP use?"
- "Where should I place access points?"

NetPoke answers:
- "Will video calls work here?"
- "Is the problem WiFi or backhaul?"
- "What's the actual end-to-end performance?"

**Combined Workflow**:
1. **RF Survey (Ekahau)**: Design and validate coverage
2. **Application Survey (NetPoke)**: Validate end-to-end performance

**Future Integration**: Screen capture of Ekahau during NetPoke survey for comprehensive documentation.

---

### Category 3: Enterprise Network Performance Monitoring (NPM)

#### Competitors
- **ThousandEyes (Cisco)** - Path visualization, internet intelligence, $20K+/yr
- **Catchpoint** - Synthetic monitoring, DEM, $50K+/yr
- **Kentik** - Flow analytics, cloud-native, $30K+/yr
- **SolarWinds NPM** - Traditional NPM, $3K-30K
- **PRTG** - SMB-focused, sensor-based, $1.5K-15K

#### How They Compare

| Capability | ThousandEyes | Catchpoint | NetPoke |
|------------|--------------|------------|-------------|
| Path visualization | ✅ | ✅ | ✅ |
| Internet outage detection | ✅ | ✅ | ❌ |
| Synthetic monitoring | ✅ | ✅ | ✅ |
| Browser-based (no agent) | ❌ | ❌ | ✅ |
| Zero deployment | ❌ | ❌ | ✅ |
| Walk-through survey | ❌ | ❌ | ✅ |
| Shareable test links | ❌ | ❌ | ✅ (Magic Keys) |
| Video + sensor capture | ❌ | ❌ | ✅ |
| Price | $20K+/yr | $50K+/yr | $600-6K/yr |

#### Positioning vs Enterprise NPM

> "Enterprise insights without enterprise complexity."

**Key Differentiators**:
1. **Zero deployment**: No agents, no probes, no infrastructure
2. **Instant access**: Send a link, get results in 60 seconds
3. **Anyone can use it**: No training required for end users
4. **10-100x cheaper**: Accessible to SMB and MSPs

**When to Use Each**:
- **ThousandEyes/Catchpoint**: Continuous monitoring of production infrastructure
- **NetPoke**: Ad-hoc diagnostics, surveys, remote troubleshooting

**Not Competing For**:
- 24/7 infrastructure monitoring
- Alerting and incident response
- Historical trend analysis at scale
- Internet backbone visibility

---

### Category 4: WiFi Troubleshooting Apps

#### Competitors
- **WiFi Analyzer** (Android) - Channel visualization, free
- **Network Analyzer** (iOS) - Basic diagnostics, free/$5
- **Fing** - Device discovery, network scanner
- **PingPlotter** - Visual traceroute, $15-50/mo

#### How They Compare

| Capability | WiFi Analyzer | PingPlotter | NetPoke |
|------------|---------------|-------------|-------------|
| Channel info | ✅ | ❌ | ❌ |
| Signal strength | ✅ | ❌ | ❌ |
| Visual traceroute | ❌ | ✅ | ✅ |
| Continuous monitoring | ❌ | ✅ | ✅ |
| Browser-based | ❌ | ❌ | ✅ |
| Video + sensor survey | ❌ | ❌ | ✅ |
| Shareable links | ❌ | Limited | ✅ |
| MTU discovery | ❌ | ❌ | ✅ |
| ECMP detection | ❌ | ❌ | ✅ |

#### Positioning

> "Professional network diagnostics, no app install required."

---

## Unique Competitive Advantages

### Technical Moat

These capabilities are unique to NetPoke and would require significant engineering to replicate:

| Capability | Difficulty to Replicate | Why |
|------------|------------------------|-----|
| Browser-based traceroute | Very High | Requires modified WebRTC stack with per-packet TTL control |
| Per-packet socket options | Very High | Vendored and modified 6 WebRTC crates |
| MTU discovery via browser | Very High | Requires DTLS bypass and ICMP correlation |
| ECMP path detection | High | Multi-connection WebRTC with path comparison |
| Video + sensor + network correlation | Medium | Novel combination, not technically hard |
| Magic Key shareable surveys | Medium | Product design, not technical barrier |

### Modified WebRTC Stack

NetPoke maintains forked versions of:
- `webrtc` v0.14.0
- `webrtc-data` v0.12.0
- `webrtc-sctp` v0.13.0
- `webrtc-util` v0.12.0
- `dtls` v0.13.0
- `webrtc-ice` v0.14.0

Changes enable:
- Per-packet TTL/DF/TOS/Flow Label
- DTLS bypass for MTU testing
- SCTP fragmentation bypass
- Type-safe option passing through stack

**This represents 6-12 months of specialized engineering that competitors would need to replicate.**

---

## Competitive Responses

### If Ookla/Speedtest Adds Features

**Risk**: Medium - Ookla could add traceroute, jitter, loss
**Mitigation**:
- Survey mode with video/sensors is differentiated
- Magic Key sharing model is unique
- Vertical focus (MSPs, WiFi professionals) vs. consumer focus
- Technical depth (MTU, ECMP) beyond typical speed test

### If Ekahau Adds Application Metrics

**Risk**: Low - Would require significant product pivot
**Response**:
- Position as "application-layer companion" rather than competitor
- Ekahau's business model is hardware + software licenses
- Screen capture feature makes us complementary, not competitive
- Different buyer (often same person wears both hats)

### If Enterprise NPM Vendors Go Downmarket

**Risk**: Low - ThousandEyes unlikely to offer $50/mo tier
**Response**:
- Zero-deployment model is fundamentally different
- Magic Key model doesn't fit enterprise sales motion
- Survey feature is consumer-grade UX, not enterprise

### If Open Source Alternative Emerges

**Risk**: Medium - Technical approach could be replicated
**Response**:
- Consider open-sourcing client library (keep server proprietary)
- Build network effects via organization/project model
- Survey storage and playback as value-add
- Speed of execution and product polish

---

## Messaging by Audience

### To RF Survey Tool Users

**Don't Say**: "Replace your Ekahau"
**Do Say**: "Complete your survey with application-layer data"

> "Your RF survey shows excellent coverage. Now prove that apps will actually perform. NetPoke measures end-to-end latency, packet loss, and path quality—everything RF can't see."

### To IT Troubleshooters

**Don't Say**: "Better than Speedtest"
**Do Say**: "When Speedtest isn't enough"

> "The user says WiFi is slow but Speedtest shows 200 Mbps. NetPoke shows 200ms jitter and 2% packet loss—that's why their Zoom calls drop. Now you can prove it."

### To MSPs

**Don't Say**: "Expensive enterprise tool"
**Do Say**: "Enterprise insights, MSP pricing"

> "Your clients expect you to diagnose network issues. Send them a Magic Key link, get full diagnostics without a site visit. Include network assessments in every engagement."

### To Enterprise IT

**Don't Say**: "Replace ThousandEyes"
**Do Say**: "Diagnose what agents can't reach"

> "Your NPM monitors the data center. But what about the VP's home office? NetPoke works anywhere with a browser—no agents, no VPN, no deployment."

---

## Competitive Comparison Page (Website Content)

### NetPoke vs. Speedtest

| | Speedtest | NetPoke |
|---|---|---|
| Speed test | ✅ | ✅ |
| Latency | ✅ | ✅ |
| Jitter | ❌ | ✅ |
| Packet loss | ❌ | ✅ |
| Traceroute | ❌ | ✅ |
| MTU discovery | ❌ | ✅ |
| Walk-through survey | ❌ | ✅ |
| Best for | Quick speed check | Diagnosing real problems |

### NetPoke vs. Ekahau

| | Ekahau | NetPoke |
|---|---|---|
| RF signal mapping | ✅ | ❌ |
| Channel planning | ✅ | ❌ |
| Application latency | ❌ | ✅ |
| End-to-end path | ❌ | ✅ |
| Backhaul testing | ❌ | ✅ |
| Hardware required | $3,000+ | None |
| Works on mobile | Limited | Full |
| Best for | RF coverage design | Application performance validation |

**Use Together**: RF survey for coverage + NetPoke for performance = complete picture.

### NetPoke vs. ThousandEyes

| | ThousandEyes | NetPoke |
|---|---|---|
| Path visualization | ✅ | ✅ |
| Continuous monitoring | ✅ | ✅ |
| Zero deployment | ❌ | ✅ |
| Shareable test links | ❌ | ✅ |
| Walk-through survey | ❌ | ✅ |
| Price | $20,000+/yr | $600-6,000/yr |
| Best for | Enterprise infrastructure | Ad-hoc diagnostics, surveys |
