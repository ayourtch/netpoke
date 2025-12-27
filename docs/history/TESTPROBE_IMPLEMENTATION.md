# TestProbe Channel Implementation

## Overview

This document describes the implementation of a separate "testprobe" data channel for traceroute packets. The main goal is to prevent traceroute packets with short TTLs from being counted towards packet loss metrics.

## Problem Statement

Previously, traceroute packets (with short TTLs that intentionally expire at intermediate hops) were sent via the regular "probe" channel and counted towards the loss rate calculation. This was incorrect because:

1. Traceroute packets with TTL < destination hops are **expected** to not reach the client
2. They should not contribute to loss rate statistics
3. Each connection should have separate sequence spaces for regular probes and test probes

## Solution

The solution implements a separate "testprobe" data channel with its own per-connection sequence space. When a testprobe reaches the client, it is echoed back, and this triggers a reset of the testprobe sequence number (since there's no need to traceroute further).

## Implementation Details

### 1. Protocol Changes (`common/src/protocol.rs`)

Added a new `TestProbePacket` structure identical to `ProbePacket`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestProbePacket {
    pub seq: u64,
    pub timestamp_ms: u64,
    pub direction: Direction,
    pub send_options: Option<SendOptions>,
}
```

### 2. Server State Changes (`server/src/state.rs`)

#### DataChannels Structure
Added `testprobe` channel to the data channels:

```rust
pub struct DataChannels {
    pub probe: Option<Arc<RTCDataChannel>>,
    pub bulk: Option<Arc<RTCDataChannel>>,
    pub control: Option<Arc<RTCDataChannel>>,
    pub testprobe: Option<Arc<RTCDataChannel>>,  // NEW
}
```

#### MeasurementState Structure
Added separate tracking for testprobes:

```rust
pub struct MeasurementState {
    pub probe_seq: u64,
    pub testprobe_seq: u64,  // NEW: Separate sequence space
    // ... other fields ...
    pub sent_testprobes: VecDeque<SentProbe>,      // NEW
    pub echoed_testprobes: VecDeque<EchoedProbe>,  // NEW
}
```

### 3. Traceroute Sender Changes (`server/src/measurements.rs`)

Modified `start_traceroute_sender()` to:
- Use the `testprobe` channel instead of `probe`
- Use `testprobe_seq` instead of `probe_seq`
- Send `TestProbePacket` instead of `ProbePacket`
- Track sent testprobes in `sent_testprobes` queue

### 4. TestProbe Handler (`server/src/measurements.rs`)

Added `handle_testprobe_packet()` function that:
- Receives echoed testprobes from the client
- Matches them with sent testprobes using sequence numbers
- Calculates round-trip delay
- **Resets testprobe sequence to 0** when a testprobe reaches the client
  - This indicates the path is clear and no further traceroute is needed

```rust
// Reset testprobe sequence number since packet reached the client
tracing::info!("🎯 Test probe reached client! Resetting testprobe sequence number");
state.testprobe_seq = 0;
```

### 5. Client Changes

#### WebRTC Connection Setup (`client/src/webrtc.rs`)
Added testprobe channel creation:

```rust
// Create testprobe channel (unreliable, unordered)
let testprobe_init = RtcDataChannelInit::new();
testprobe_init.set_ordered(false);
testprobe_init.set_max_retransmits(0);
let testprobe_channel = peer.create_data_channel_with_data_channel_dict("testprobe", &testprobe_init);
measurements::setup_testprobe_channel(testprobe_channel);
```

#### TestProbe Handler (`client/src/measurements.rs`)
Added `setup_testprobe_channel()` function that:
- Receives testprobes from server
- Echoes them back with updated timestamp
- Logs the sequence number for debugging

### 6. Data Channel Registration (`server/src/data_channels.rs`)

Added handler for the "testprobe" channel:

```rust
"testprobe" => {
    chans.testprobe = Some(dc.clone());
    tracing::info!("TestProbe channel registered for client {}", session.id);
},
```

## Key Benefits

### 1. Accurate Loss Metrics
Testprobe packets are completely separate from regular probe packets:
- Regular probes: Used for normal loss/delay/jitter measurements
- Testprobes: Used only for traceroute diagnostics
- Loss rate calculations only use regular probes

### 2. Separate Sequence Spaces
Each connection has two independent sequence counters:
- `probe_seq`: For regular S2C probes (never reset)
- `testprobe_seq`: For traceroute test probes (resets when reaching client)

### 3. Smart Traceroute Behavior
When a testprobe reaches the client:
- The sequence number is reset to 0
- Server knows there's no need to continue increasing TTL
- Next traceroute cycle starts from TTL=1 again

### 4. Backward Compatible
- Existing probe/bulk/control channels unchanged
- No impact on existing measurements
- Clients without testprobe support simply won't register the channel

## Testing

### Unit Tests

1. **TestProbePacket Serialization** (`common/src/protocol.rs`)
   - Tests serialization/deserialization of TestProbePacket
   - Verifies send_options are preserved

2. **Separate Sequence Spaces** (`server/src/measurements.rs`)
   - Verifies probe_seq and testprobe_seq are independent
   - Tests that incrementing one doesn't affect the other

### Manual Testing

To verify the implementation:

1. Start the server:
   ```bash
   RUST_LOG=info cargo run -p wifi-verify-server
   ```

2. Connect a client (web browser)

3. Look for these log messages:
   - `TestProbe channel registered for client {id}` - Channel setup
   - `🔵 Sending traceroute test probe via testprobe channel: TTL={n}` - Sending
   - `Received echoed S2C test probe seq {n}` - Echo received
   - `🎯 Test probe reached client! Resetting testprobe sequence number` - Reset

## Files Modified

### Common Library
- `common/src/protocol.rs` - Added TestProbePacket structure

### Server
- `server/src/state.rs` - Added testprobe channel and sequence tracking
- `server/src/data_channels.rs` - Added testprobe channel handler
- `server/src/measurements.rs` - Modified traceroute sender, added testprobe handler

### Client
- `client/src/webrtc.rs` - Added testprobe channel creation
- `client/src/measurements.rs` - Added testprobe echo handler

## Future Enhancements

Potential improvements:
1. Add metrics dashboard for testprobe-specific statistics
2. Implement adaptive TTL range based on detected hop count
3. Add configuration option to enable/disable testprobe sequence reset
4. Expose testprobe RTT measurements via API

## Conclusion

The testprobe channel implementation successfully separates traceroute diagnostic packets from regular measurement packets. This ensures accurate loss metrics while maintaining full traceroute functionality with smart sequence number management.
