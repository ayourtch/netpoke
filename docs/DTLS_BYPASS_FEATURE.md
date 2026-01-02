# DTLS Bypass Feature for MTU Tests

**Status**: ✅ Implemented  
**Date**: 2026-01-02  
**Purpose**: Enable direct control of UDP packet sizes for MTU discovery tests

## Overview

This feature allows bypassing DTLS encryption for specific packets, particularly useful for MTU (Maximum Transmission Unit) discovery tests where DTLS framing would interfere with precise packet size control.

## Background

### The Problem

In commit `20db0dcb9b822dbe878ff379744a90cad6f5ecfe`, a fix was implemented to ensure cleartext was not sent directly into UDP without DTLS encryption. This was a security fix to prevent data leakage.

However, for MTU tests, DTLS encryption adds overhead that interferes with accurate MTU discovery:
- DTLS adds framing and encryption overhead (typically 13-29 bytes)
- MTU tests need precise control over UDP packet sizes
- ICMP "Fragmentation Needed" messages report the actual UDP packet size, not the application payload size

### The Solution

A controlled bypass mechanism was added that:
- Allows sending cleartext UDP packets when explicitly requested
- Maintains DTLS encryption by default
- Only used for diagnostic/measurement packets
- Configured per-packet via send options

## Implementation Details

### 1. SendOptions Field

Added `bypass_dtls` field to `SendOptions` struct in `common/src/protocol.rs`:

```rust
/// UDP socket options for packet transmission
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub struct SendOptions {
    pub ttl: Option<u8>,
    pub df_bit: Option<bool>,
    pub tos: Option<u8>,
    pub flow_label: Option<u32>,
    pub track_for_ms: u32,
    
    /// Bypass DTLS encryption and send cleartext directly to UDP
    /// This is useful for MTU tests where DTLS framing would interfere with packet size control
    /// WARNING: Only use for diagnostic packets, not sensitive data
    #[serde(default)]
    pub bypass_dtls: bool,
}
```

Key points:
- `#[serde(default)]` ensures backward compatibility (defaults to `false`)
- Warning comment indicates this should only be used for diagnostic packets

### 2. UdpSendOptions Field

Added corresponding field to `UdpSendOptions` in `vendored/webrtc-util/src/conn/conn_udp.rs`:

```rust
pub struct UdpSendOptions {
    pub ttl: Option<u8>,
    pub tos: Option<u8>,
    pub df_bit: Option<bool>,
    pub conn_id: String,
    
    /// Bypass DTLS encryption and send cleartext directly to UDP
    /// This is useful for MTU tests where DTLS framing would interfere with packet size control
    /// WARNING: Only use for diagnostic packets, not sensitive data
    pub bypass_dtls: bool,
}
```

### 3. DTLSConn Bypass Logic

Modified `DTLSConn::send_with_options()` in `vendored/dtls/src/conn/mod.rs`:

```rust
#[cfg(target_os = "linux")]
async fn send_with_options(
    &self,
    buf: &[u8],
    options: &UdpSendOptions,
) -> UtilResult<usize> {
    // Check if we should bypass DTLS encryption
    if options.bypass_dtls {
        log::debug!("🔵 DTLSConn::send_with_options: BYPASSING DTLS - Sending cleartext {} bytes with TTL={:?}, TOS={:?}, DF={:?}",
            buf.len(), options.ttl, options.tos, options.df_bit);
        // Send cleartext directly to underlying connection
        return self.conn.send_with_options(buf, options).await;
    }
    
    log::debug!("🔵 DTLSConn::send_with_options: Sending encrypted data with TTL={:?}, TOS={:?}, DF={:?}",
        options.ttl, options.tos, options.df_bit);
    self.write_with_options(buf, options, None).await.map_err(util::Error::from_std)
}
```

The logic:
1. Check `options.bypass_dtls` flag
2. If `true`: send cleartext directly via `self.conn.send_with_options()`, bypassing DTLS
3. If `false`: use normal DTLS encryption via `self.write_with_options()`
4. Debug logging distinguishes between bypass and encrypted sends

### 4. MTU Traceroute Configuration

Updated `run_mtu_traceroute_round()` in `server/src/measurements.rs`:

```rust
// For SendOptions (in TestProbePacket)
let send_options = common::SendOptions {
    ttl: Some(current_ttl),
    df_bit: Some(true),  // DF bit is essential for MTU discovery
    tos: None,
    flow_label: None,
    track_for_ms: 5000,
    bypass_dtls: true,  // Bypass DTLS for MTU tests to control exact packet sizes
};

// For UdpSendOptions (actual transmission)
let options = Some(UdpSendOptions {
    ttl: Some(current_ttl),
    tos: None,
    df_bit: Some(true),
    conn_id: session.conn_id.clone(),
    bypass_dtls: true,  // Bypass DTLS for MTU tests to control exact packet sizes
});
```

Regular traceroute continues to use `bypass_dtls: false` to maintain encryption.

## Data Flow with Bypass

### Normal Flow (bypass_dtls = false)

```
Application Data
  → RTCDataChannel::send_with_options()
  → DataChannel::write_data_channel_with_options()
  → Stream::write_sctp_with_options()
  → Association (SCTP layer)
  → DTLSConn::send_with_options()
    → write_with_options() [ENCRYPTS DATA]
    → DTLSConn::write_packets_with_options()
    → Endpoint::send_with_options()
    → UdpSocket::send_with_options()
    → sendmsg() with control messages (TTL, DF, TOS)
```

### Bypass Flow (bypass_dtls = true)

```
Application Data
  → RTCDataChannel::send_with_options()
  → DataChannel::write_data_channel_with_options()
  → Stream::write_sctp_with_options()
  → Association (SCTP layer)
  → DTLSConn::send_with_options()
    → BYPASS: self.conn.send_with_options() [CLEARTEXT]
    → Endpoint::send_with_options()
    → UdpSocket::send_with_options()
    → sendmsg() with control messages (TTL, DF, TOS)
```

Note: When bypassing DTLS only, data still goes through SCTP layer, so SCTP framing and 
potential fragmentation are still present (packets may be split if larger than max_payload_size).

## SCTP Fragmentation Bypass (Added 2026-01-02)

In addition to DTLS bypass, a second bypass flag was added to address SCTP fragmentation:

### The Problem

Even with `bypass_dtls: true`, the SCTP layer still fragments large messages:
- SCTP splits messages larger than `max_payload_size` (~1200 bytes) into multiple chunks
- Each chunk becomes a separate packet after bundling
- This defeats MTU discovery tests that need to send packets larger than 1200 bytes

### The Solution

Added `bypass_sctp_fragmentation` flag to `SendOptions` and `UdpSendOptions`:

```rust
pub struct SendOptions {
    pub ttl: Option<u8>,
    pub df_bit: Option<bool>,
    pub tos: Option<u8>,
    pub flow_label: Option<u32>,
    pub track_for_ms: u32,
    pub bypass_dtls: bool,
    pub bypass_sctp_fragmentation: bool,  // NEW: Bypass SCTP fragmentation
}
```

When `bypass_sctp_fragmentation: true`:
- Stream::packetize() sends the entire payload as a single SCTP chunk
- No fragmentation based on max_payload_size
- Allows sending packets up to interface MTU (e.g., 1500 bytes or more)

### Data Flow with Both Bypasses

```
Application Data
  → RTCDataChannel::send_with_options()
  → DataChannel::write_data_channel_with_options()
  → Stream::write_sctp_with_options()
    → BYPASS SCTP FRAG: Send as single chunk (no fragmentation)
  → Association (SCTP layer - single chunk)
  → DTLSConn::send_with_options()
    → BYPASS DTLS: self.conn.send_with_options() [CLEARTEXT]
    → Endpoint::send_with_options()
    → UdpSocket::send_with_options()
    → sendmsg() with control messages (TTL, DF, TOS)
```

Result: Original data sent as single UDP packet with precise size control.

## Usage Examples

### MTU Traceroute (with both bypasses)

For MTU testing, use both `bypass_dtls` and `bypass_sctp_fragmentation`:

```rust
let send_options = common::SendOptions {
    ttl: Some(ttl_value),
    df_bit: Some(true),
    tos: None,
    flow_label: None,
    track_for_ms: 5000,
    bypass_dtls: true,  // Bypass DTLS for MTU testing
    bypass_sctp_fragmentation: true,  // Bypass SCTP fragmentation for large packets
};

let options = Some(UdpSendOptions {
    ttl: Some(ttl_value),
    tos: None,
    df_bit: Some(true),
    conn_id: session.conn_id.clone(),
    bypass_dtls: true,
    bypass_sctp_fragmentation: true,
});

testprobe_channel.send_with_options(&json.into(), options).await?;
```

This allows sending packets larger than 1200 bytes (up to interface MTU) with precise size control.

### Regular Traceroute (with encryption and normal fragmentation)

```rust
let send_options = common::SendOptions {
    ttl: Some(ttl_value),
    df_bit: Some(true),
    tos: None,
    flow_label: None,
    track_for_ms: 5000,
    bypass_dtls: false,  // Keep DTLS encryption
    bypass_sctp_fragmentation: false,  // Use normal SCTP fragmentation
};

let options = Some(UdpSendOptions {
    ttl: Some(ttl_value),
    tos: None,
    df_bit: Some(true),
    conn_id: session.conn_id.clone(),
    bypass_dtls: false,
    bypass_sctp_fragmentation: false,
});

testprobe_channel.send_with_options(&json.into(), options).await?;
```

## When to Use Each Bypass

### bypass_dtls only (bypass_sctp_fragmentation: false)

Use when you need to bypass DTLS encryption but packets are small enough to fit in SCTP chunks (~1200 bytes):

- ✅ Small diagnostic packets
- ✅ Traceroute probes (typically < 100 bytes)
- ✅ Regular network measurements

### Both bypasses (bypass_dtls: true, bypass_sctp_fragmentation: true)

Use when you need to send large packets for MTU testing:

- ✅ MTU discovery (packets > 1200 bytes)
- ✅ Path MTU testing (up to interface MTU)
- ✅ Fragmentation behavior analysis

**WARNING**: Very large packets (> path MTU) will be fragmented or dropped by routers.

### Neither bypass (both: false) - RECOMMENDED DEFAULT

Use for all regular traffic that needs security:

- ✅ User data
- ✅ Application messages
- ✅ Any sensitive information
- ✅ Production traffic

## Security Considerations

### What's Protected

1. **Default behavior**: Both `bypass_dtls` and `bypass_sctp_fragmentation` default to `false`
2. **Explicit opt-in**: Must explicitly enable each bypass per packet
3. **Per-packet control**: Each packet can independently choose to bypass or not
4. **Backward compatible**: Old code continues to use DTLS encryption and normal fragmentation

### What's Not Protected

1. **Cleartext transmission**: When DTLS bypass is enabled, data is sent in cleartext
2. **No authentication**: Bypassed packets don't have DTLS authentication/integrity
3. **Visible to network**: Packet contents are visible to network observers
4. **Path MTU risks**: Large unfragmented packets may be dropped or fragmented by routers

### When to Use Bypass

✅ **Safe to use**:
- MTU discovery probes
- Network diagnostics
- Path measurement packets
- Non-sensitive test data

❌ **DO NOT use**:
- User data
- Authentication tokens
- Passwords or credentials
- Any sensitive information

## Verification

### Debug Logging

When DTLS bypass is active, you'll see:

```
DEBUG [dtls] 🔵 DTLSConn::send_with_options: BYPASSING DTLS - Sending cleartext 1500 bytes with TTL=Some(5), TOS=None, DF=Some(true)
```

When SCTP fragmentation bypass is active, you'll see:

```
DEBUG [webrtc_sctp::stream] 🔵 Stream::packetize: BYPASSING SCTP FRAGMENTATION - Sending 1500 bytes as single chunk
DEBUG [webrtc_sctp::stream] 🔵 Stream::packetize: Set UDP options on chunk: TTL=Some(5), TOS=None, DF=Some(true), bypass_sctp_frag=true
```

When encryption is active:

```
DEBUG [dtls] 🔵 DTLSConn::send_with_options: Sending encrypted data with TTL=Some(5), TOS=None, DF=Some(true)
```

### Testing

Run MTU traceroute and verify:

1. Packets have exact expected sizes (no DTLS overhead, no SCTP fragmentation)
2. Large packets (> 1200 bytes) are sent as single UDP packets
3. ICMP "Fragmentation Needed" messages report correct MTU values
4. Debug logs show "BYPASSING DTLS" for MTU test packets
5. Debug logs show "BYPASSING SCTP FRAGMENTATION" for large packets
6. Debug logs show "Sending encrypted data" for regular traceroute packets

### Packet Capture

Use tcpdump to verify both bypasses:

```bash
# Capture MTU test packets
sudo tcpdump -i any -v -n -X 'udp and port 5004'

# With bypass_dtls=true, you should see readable JSON in packet data
# With bypass_dtls=false, you should see encrypted binary data

# Check packet sizes
sudo tcpdump -i any -v -n 'udp and port 5004' | grep -E 'length [0-9]+'

# With bypass_sctp_fragmentation=true, packets can be > 1200 bytes
# With bypass_sctp_fragmentation=false, packets are typically <= 1200 bytes (fragmented by SCTP)
```

## Future Considerations

1. **Additional use cases**: Consider if other diagnostic features need bypass
2. **Statistics**: Track how often bypass is used
3. **Audit logging**: Log when bypass is activated for security auditing
4. **Per-channel control**: Consider allowing bypass to be configured at data channel level

## Related Documents

- [UDP_PACKET_OPTIONS.md](UDP_PACKET_OPTIONS.md) - Per-packet UDP options implementation
- [DTLS_FORWARDING_FIX.md](history/DTLS_FORWARDING_FIX.md) - DTLS send_with_options forwarding
- [TRACEROUTE_INVESTIGATION_SUMMARY.md](../TRACEROUTE_INVESTIGATION_SUMMARY.md) - Traceroute implementation details

## Summary

The dual bypass feature (DTLS + SCTP fragmentation) provides:

✅ **Complete packet size control** - Bypass both DTLS encryption and SCTP fragmentation  
✅ **Large packet support** - Send packets larger than 1200 bytes (up to interface MTU)  
✅ **Precise MTU discovery** - No overhead from DTLS or SCTP chunking  
✅ **Secure by default** - Both bypasses default to `false`  
✅ **Explicit opt-in** - Must explicitly enable each bypass per packet  
✅ **Independent control** - Can bypass DTLS without bypassing SCTP, or vice versa  
✅ **Backward compatible** - Old code continues to work with encryption and normal fragmentation  
✅ **Well documented** - Clear warnings about when to use each bypass  

This feature enables accurate MTU testing with full control over UDP packet sizes, while maintaining security for all other traffic.

### Implementation Summary

1. **DTLS bypass** - Skips DTLS encryption layer (implemented 2026-01-02)
2. **SCTP fragmentation bypass** - Prevents SCTP from splitting large messages (added 2026-01-02)
3. **Combined effect** - Original application data → single UDP packet with exact size

Both bypasses work independently and can be enabled/disabled separately as needed.
