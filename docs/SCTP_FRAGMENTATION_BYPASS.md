# SCTP Fragmentation Bypass Feature

**Status**: ✅ Implemented  
**Date**: 2026-01-02  
**Purpose**: Enable MTU probes larger than 1200 bytes by bypassing SCTP fragmentation

## Overview

This document describes the implementation of `bypass_sctp_fragmentation` flag, which complements the existing `bypass_dtls` flag to provide complete control over UDP packet sizes for MTU discovery tests.

## Problem Statement

The original issue reported:

> For the MTU probes, there is a problem: setting bypass_dtls only bypasses the DTLS - but the packet itself is split in two for bigger packets. Is there a way to also to make this option bypass the other layers of webrtc as well, such that the original packet is delivered to UDP, so that we could go in bigger sizes (up to the interface MTU)?

### Root Cause

Even with `bypass_dtls: true`, packets were being split by the SCTP layer:

1. **DTLS bypass** (existing) - Bypasses DTLS encryption ✅
2. **SCTP fragmentation** (problem) - SCTP still splits messages > 1200 bytes into multiple chunks ❌
3. **Result** - Large MTU probe packets were sent as multiple UDP packets, defeating MTU discovery

Example:
```
Application: 1500-byte MTU probe
  ↓
SCTP Layer: Splits into chunks (1200 bytes + 300 bytes)
  ↓
DTLS Layer: Bypassed (with bypass_dtls)
  ↓
UDP: Two separate packets sent ❌
```

## Solution

Added `bypass_sctp_fragmentation` flag to skip SCTP's message fragmentation logic.

### Implementation

#### 1. Protocol Definition

Added field to `SendOptions` in `common/src/protocol.rs`:

```rust
pub struct SendOptions {
    pub ttl: Option<u8>,
    pub df_bit: Option<bool>,
    pub tos: Option<u8>,
    pub flow_label: Option<u32>,
    pub track_for_ms: u32,
    pub bypass_dtls: bool,
    pub bypass_sctp_fragmentation: bool,  // NEW
}
```

#### 2. UDP Options

Added field to `UdpSendOptions` in `vendored/webrtc-util/src/conn/conn_udp.rs`:

```rust
pub struct UdpSendOptions {
    pub ttl: Option<u8>,
    pub tos: Option<u8>,
    pub df_bit: Option<bool>,
    pub conn_id: String,
    pub bypass_dtls: bool,
    pub bypass_sctp_fragmentation: bool,  // NEW
}
```

#### 3. SCTP Stream Layer

Modified `Stream::packetize()` in `vendored/webrtc-sctp/src/stream/mod.rs`:

```rust
// Check if we should bypass SCTP fragmentation for MTU testing
let bypass_fragmentation = udp_send_options
    .as_ref()
    .map(|opts| opts.bypass_sctp_fragmentation)
    .unwrap_or(false);

while remaining != 0 {
    // If bypass_sctp_fragmentation is enabled, send entire payload as one chunk
    // Otherwise, respect max_payload_size
    let fragment_size = if bypass_fragmentation {
        remaining  // Send all remaining data in one chunk
    } else {
        std::cmp::min(self.max_payload_size as usize, remaining)
    };
    
    // ... create chunk with fragment_size ...
}
```

**Key behavior:**
- When `bypass_fragmentation: false` (default): Fragment based on `max_payload_size` (~1200 bytes)
- When `bypass_fragmentation: true`: Send entire payload as single chunk (up to interface MTU)

#### 4. MTU Traceroute Usage

Updated `run_mtu_traceroute_round()` in `server/src/measurements.rs`:

```rust
let send_options = common::SendOptions {
    ttl: Some(current_ttl),
    df_bit: Some(true),
    tos: None,
    flow_label: None,
    track_for_ms: 5000,
    bypass_dtls: true,
    bypass_sctp_fragmentation: true,  // NEW: Enable for MTU testing
};

let options = Some(UdpSendOptions {
    ttl: Some(current_ttl),
    tos: None,
    df_bit: Some(true),
    conn_id: session.conn_id.clone(),
    bypass_dtls: true,
    bypass_sctp_fragmentation: true,  // NEW: Enable for MTU testing
});
```

## Data Flow

### Before: DTLS Bypass Only (Problem)

```
Application: 1500-byte MTU probe
  ↓
RTCDataChannel::send_with_options()
  ↓
Stream::packetize()
  → FRAGMENTS into: Chunk 1 (1200 bytes) + Chunk 2 (300 bytes)  ❌
  ↓
Association (bundles chunks into separate packets)
  ↓
DTLSConn::send_with_options()
  → BYPASSES DTLS (cleartext)  ✅
  ↓
UDP: Sends TWO packets (1200 bytes + 300 bytes)  ❌
```

### After: Both Bypasses (Solution)

```
Application: 1500-byte MTU probe
  ↓
RTCDataChannel::send_with_options()
  ↓
Stream::packetize()
  → BYPASS FRAGMENTATION: Single chunk (1500 bytes)  ✅
  ↓
Association (single packet with single chunk)
  ↓
DTLSConn::send_with_options()
  → BYPASSES DTLS (cleartext)  ✅
  ↓
UDP: Sends ONE packet (1500 bytes)  ✅
```

## Usage Guidelines

### When to Enable Each Bypass

| Scenario | bypass_dtls | bypass_sctp_fragmentation | Use Case |
|----------|-------------|---------------------------|----------|
| **Default** | `false` | `false` | Regular encrypted traffic |
| **Small probes** | `true` | `false` | Traceroute probes < 1200 bytes |
| **MTU testing** | `true` | `true` | MTU discovery > 1200 bytes |
| **Not recommended** | `false` | `true` | Encrypted large messages (inefficient) |

### Best Practices

1. **Always use both bypasses for MTU testing**
   ```rust
   bypass_dtls: true,
   bypass_sctp_fragmentation: true,
   ```

2. **Keep defaults for production traffic**
   ```rust
   bypass_dtls: false,
   bypass_sctp_fragmentation: false,
   ```

3. **Consider packet size limits**
   - Interface MTU: Typically 1500 bytes (Ethernet)
   - Path MTU: May be smaller on some networks
   - Packets > path MTU will be dropped or fragmented by routers

## Debug Logging

When both bypasses are enabled, you'll see:

```
DEBUG [webrtc_sctp::stream] 🔵 Stream::packetize: Created chunk 1500 bytes (bypass_sctp_frag=true, TTL=Some(5), TOS=None, DF=Some(true))
DEBUG [dtls] 🔵 DTLSConn::send_with_options: BYPASSING DTLS - Sending cleartext 1500 bytes with TTL=Some(5), TOS=None, DF=Some(true)
```

## Security Considerations

### SCTP Fragmentation Bypass

- ✅ **Safe for diagnostics**: MTU probes, network testing
- ⚠️ **Use with caution**: Very large packets may exceed path MTU
- ❌ **Not for production**: Keep SCTP fragmentation for reliability

### Combined with DTLS Bypass

When both bypasses are enabled:
- ⚠️ **Cleartext transmission**: No encryption or authentication
- ⚠️ **Large packets**: Risk of being dropped or fragmented
- ✅ **Precise control**: Full control over UDP packet size
- ✅ **MTU discovery**: Ideal for path MTU detection

## Testing

### Verification

1. **Build and test**
   ```bash
   cargo check --all
   cargo test --lib -p common
   ```

2. **Run MTU traceroute**
   ```bash
   # Server side (with RUST_LOG=debug)
   cargo run --bin wifi-verify-server
   ```

3. **Packet capture**
   ```bash
   # Observe packet sizes
   sudo tcpdump -i any -v -n 'udp' | grep -E 'length [0-9]+'
   
   # With both bypasses: packets > 1200 bytes
   # Without SCTP bypass: packets ≤ 1200 bytes
   ```

### Expected Results

- ✅ Single UDP packet for each MTU probe (even > 1200 bytes)
- ✅ Exact packet size matches application payload
- ✅ ICMP "Fragmentation Needed" reports accurate MTU values
- ✅ Debug logs show bypass flags in effect

## Limitations

1. **Linux only**: Per-packet options work only on Linux
2. **Path MTU**: Packets larger than path MTU will be dropped
3. **SCTP overhead**: SCTP headers still present (~16 bytes)
4. **No authentication**: Bypassed packets have no integrity checking

## Files Modified

- `common/src/protocol.rs` - Added `bypass_sctp_fragmentation` to SendOptions
- `vendored/webrtc-util/src/conn/conn_udp.rs` - Added to UdpSendOptions
- `vendored/webrtc-sctp/src/stream/mod.rs` - Implemented bypass logic in packetize()
- `server/src/measurements.rs` - Enabled for MTU traceroute
- `server/src/main.rs` - Updated SendOptions constructor
- `server/src/packet_tracker.rs` - Updated test constructors
- `docs/DTLS_BYPASS_FEATURE.md` - Updated documentation

## Related Documents

- [DTLS_BYPASS_FEATURE.md](DTLS_BYPASS_FEATURE.md) - DTLS and SCTP bypass feature documentation
- [UDP_PACKET_OPTIONS.md](UDP_PACKET_OPTIONS.md) - Per-packet UDP options implementation

## Summary

The `bypass_sctp_fragmentation` feature successfully addresses the problem statement by:

✅ **Enabling large MTU probes** - Packets > 1200 bytes sent as single UDP packets  
✅ **Precise size control** - Application payload → UDP packet (no fragmentation)  
✅ **Secure by default** - Flag defaults to `false`, requires explicit opt-in  
✅ **Backward compatible** - No changes to existing behavior  
✅ **Well integrated** - Works seamlessly with existing `bypass_dtls` flag  

This feature enables accurate MTU discovery tests up to the interface MTU while maintaining security for all other traffic.
