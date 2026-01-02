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

Note: When bypassing, data still goes through SCTP layer, so SCTP framing is still present.

## Usage Examples

### MTU Traceroute (with bypass)

```rust
let send_options = common::SendOptions {
    ttl: Some(ttl_value),
    df_bit: Some(true),
    tos: None,
    flow_label: None,
    track_for_ms: 5000,
    bypass_dtls: true,  // Bypass for MTU testing
};

let options = Some(UdpSendOptions {
    ttl: Some(ttl_value),
    tos: None,
    df_bit: Some(true),
    conn_id: session.conn_id.clone(),
    bypass_dtls: true,
});

testprobe_channel.send_with_options(&json.into(), options).await?;
```

### Regular Traceroute (with encryption)

```rust
let send_options = common::SendOptions {
    ttl: Some(ttl_value),
    df_bit: Some(true),
    tos: None,
    flow_label: None,
    track_for_ms: 5000,
    bypass_dtls: false,  // Keep DTLS encryption
};

let options = Some(UdpSendOptions {
    ttl: Some(ttl_value),
    tos: None,
    df_bit: Some(true),
    conn_id: session.conn_id.clone(),
    bypass_dtls: false,
});

testprobe_channel.send_with_options(&json.into(), options).await?;
```

## Security Considerations

### What's Protected

1. **Default behavior**: `bypass_dtls` defaults to `false`, maintaining DTLS encryption
2. **Explicit opt-in**: Must explicitly set `bypass_dtls: true` to bypass encryption
3. **Per-packet control**: Each packet can independently choose to bypass or not
4. **Backward compatible**: Old code continues to use DTLS encryption

### What's Not Protected

1. **Cleartext transmission**: When bypass is enabled, data is sent in cleartext
2. **No authentication**: Bypassed packets don't have DTLS authentication/integrity
3. **Visible to network**: Packet contents are visible to network observers

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

When bypass is active, you'll see:

```
DEBUG [dtls] 🔵 DTLSConn::send_with_options: BYPASSING DTLS - Sending cleartext 1500 bytes with TTL=Some(5), TOS=None, DF=Some(true)
```

When encryption is active:

```
DEBUG [dtls] 🔵 DTLSConn::send_with_options: Sending encrypted data with TTL=Some(5), TOS=None, DF=Some(true)
```

### Testing

Run MTU traceroute and verify:

1. Packets have exact expected sizes (no DTLS overhead)
2. ICMP "Fragmentation Needed" messages report correct MTU values
3. Debug logs show "BYPASSING DTLS" for MTU test packets
4. Debug logs show "Sending encrypted data" for regular traceroute packets

### Packet Capture

Use tcpdump to verify cleartext:

```bash
# Capture MTU test packets
sudo tcpdump -i any -v -n -X 'udp and port 5004'

# With bypass_dtls=true, you should see readable JSON in packet data
# With bypass_dtls=false, you should see encrypted binary data
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

The DTLS bypass feature provides:

✅ **Precise MTU control** - Send exact packet sizes for accurate MTU discovery  
✅ **Secure by default** - DTLS encryption is the default behavior  
✅ **Explicit opt-in** - Must explicitly request bypass per packet  
✅ **Backward compatible** - Old code continues to work with encryption  
✅ **Well documented** - Clear warnings about when to use bypass  

This feature enables accurate MTU testing while maintaining security for all other traffic.
