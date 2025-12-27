# Testing ICMP-based Session Cleanup

This document describes how to manually test the ICMP-based session cleanup feature.

## Feature Overview

When a client abruptly disconnects (e.g., browser closed, network interrupted), the server may continue sending WebRTC packets to the client's IP address. This results in ICMP "Port Unreachable" errors being received by the server. The new feature detects these errors and automatically cleans up the orphaned session.

## How It Works

1. **ICMP Error Detection**: Both IPv4 (ICMP) and IPv6 (ICMPv6) error listeners are running
2. **Error Tracking**: Unmatched ICMP errors are tracked per destination IP address
3. **Threshold Trigger**: After 5 consecutive unmatched errors to the same IP, cleanup is triggered
4. **Session Cleanup**: All sessions with that peer IP are closed and removed

## Testing Procedure

### Prerequisites
- Server must be running with root privileges or CAP_NET_RAW capability (for ICMP socket)
- At least one WebRTC client connected to the server

### Test Steps

1. **Start the server**:
   ```bash
   cd /home/runner/work/wifi-verify/wifi-verify
   cargo build --release --package wifi-verify-server
   sudo target/release/wifi-verify-server
   ```

2. **Connect a client** via the web interface

3. **Monitor server logs**:
   Look for these log messages:
   - `"ICMP-based session cleanup callback registered"`
   - `"IPv4 ICMP listener started successfully"`
   - `"IPv6 ICMPv6 listener started successfully"`

4. **Abruptly disconnect the client**:
   - Close the browser tab/window without proper disconnect, OR
   - Kill the browser process, OR
   - Disconnect the client's network

5. **Observe the server behavior**:
   
   **Before the fix**: You would see repeated messages like:
   ```
   DEBUG: Parsed ICMP error successfully:
     ICMP type=3, code=3
     src_port=36292, dest=37.228.235.203:55676
     udp_length=112
     payload_prefix len=0
   DEBUG: Current tracked packets count: 0
   DEBUG: NO MATCH FOUND for dest=37.228.235.203:55676, udp_length=112
   ```

   **After the fix**: You should see:
   ```
   WARN: Unmatched ICMP error for dest=<IP> (count: 1/5)
   WARN: Unmatched ICMP error for dest=<IP> (count: 2/5)
   WARN: Unmatched ICMP error for dest=<IP> (count: 3/5)
   WARN: Unmatched ICMP error for dest=<IP> (count: 4/5)
   WARN: Unmatched ICMP error for dest=<IP> (count: 5/5)
   WARN: ICMP error threshold reached for dest=<IP>, triggering session cleanup
   WARN: Cleaning up N session(s) with peer IP <IP> due to ICMP errors: [<session_ids>]
   INFO: Closed peer connection for <session_id> due to ICMP errors
   ```

6. **Verify cleanup**:
   - Check the dashboard - the disconnected client should be removed
   - No more ICMP error messages for that IP should appear

## Expected Behavior

### IPv4 Test
- ICMP Type 3 (Destination Unreachable) errors should trigger cleanup
- After 5 consecutive errors, session is automatically removed

### IPv6 Test
- ICMPv6 Type 1 (Destination Unreachable) errors should trigger cleanup
- After 5 consecutive errors, session is automatically removed

## Configuration

The error threshold (default: 5) is hardcoded in `PacketTracker::new()`. To adjust:
```rust
error_threshold: 5, // Change this value
```

## Troubleshooting

### ICMP listener not starting
- **Error**: "Failed to create IPv4 ICMP socket"
- **Solution**: Run with `sudo` or grant CAP_NET_RAW capability:
  ```bash
  sudo setcap cap_net_raw+ep target/release/wifi-verify-server
  ```

### No cleanup happening
- Check that peer_address is properly set for the session
- Verify ICMP errors are actually being received (check raw socket with tcpdump)
- Ensure the client IP matches what's stored in peer_address

### False positives
- If legitimate network issues cause ICMP errors, sessions might be cleaned up prematurely
- Consider increasing the threshold or adding a time window check

## Notes

- The feature works for both IPv4 and IPv6
- Error records older than 30 seconds are automatically cleaned up
- Successful packet matches reset the error counter for that IP
- Multiple sessions to the same IP are all cleaned up together
