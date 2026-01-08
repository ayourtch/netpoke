# Quick Verification Guide

## How to Verify the Traceroute Fix Works

### Step 1: Build and Run the Server

```bash
cd /home/runner/work/netpoke/netpoke
cargo build --release -p netpoke-server
sudo ./target/release/netpoke-server
```

Note: `sudo` is required for the ICMP listener to work (needs CAP_NET_RAW).

### Step 2: Enable Debug Logging (Optional)

To see detailed logs showing TTL values being set:

```bash
RUST_LOG=debug sudo ./target/release/netpoke-server
```

Look for these debug messages that confirm TTL is being set:
```
DEBUG: UdpSocket::send_with_options called with TTL=Some(1)
DEBUG: sendmsg_with_options called with fd=X, TTL=Some(1)
DEBUG: Adding IPv6 hop limit control message: 1
DEBUG: sendmsg succeeded, sent N bytes
```

### Step 3: Capture Packets with tcpdump

In a separate terminal, capture UDP packets to verify TTL values:

```bash
# Replace PORT with the actual UDP port your server is using
sudo tcpdump -i any -vvv 'udp and port PORT' 2>&1 | grep -E 'ttl|hlim'
```

You should see packets with TTL values of 1, 2, 3, etc.:
```
IP (... ttl 1, ...) source > dest: UDP, length ...
IP (... ttl 2, ...) source > dest: UDP, length ...
IP (... ttl 3, ...) source > dest: UDP, length ...
```

### Step 4: Check for ICMP Messages

If you have routers between the server and client, you should see ICMP Time Exceeded messages in the server logs:

```
DEBUG: Received ICMP packet: size=56, from=ROUTER_IP:0
DEBUG: ICMP type=11, code=0  // Time Exceeded
INFO: Matched ICMP error to packet seq=X
```

### Step 5: Verify Warning is Gone

The following warning should NO LONGER appear in logs:
```
WARN: Received echoed probe seq X but couldn't find matching sent probe
```

**Note**: If you're testing on a LAN without intermediate routers, the packets may still reach the client (which is normal - there are no routers to expire the packets). In that case, the echoed probe warnings might still appear, but the TTL values in tcpdump will prove the fix is working.

## What Changed?

The fix ensures that `send_with_options()` calls are properly forwarded through the WebRTC Mux Endpoint wrapper to the underlying UDP socket. Previously, the Endpoint was using a default trait implementation that discarded UDP options.

## Quick Test Results

Run all tests to ensure nothing is broken:
```bash
cargo test --workspace
```

Expected: All tests pass (103 tests in total).

## For Detailed Information

See `TRACEROUTE_TTL_FIX.md` for comprehensive documentation including:
- Root cause analysis
- Code path explanation
- Complete solution details
- Expected behavior
