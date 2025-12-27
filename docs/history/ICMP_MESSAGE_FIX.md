# ICMP Error Message Delivery Fix

## Problem Statement

ICMP errors were being matched to traceroute packets and queued in the event queue, but were never consumed and sent to the client. As seen in the logs:

```
DEBUG: MATCH FOUND! dest=37.228.235.203:55670, udp_length=296
DEBUG: Event added to queue, queue size: 6
[INFO] ICMP error matched to tracked packet: dest=37.228.235.203:55670, udp_length=296
```

However, these messages never appeared in the client textbox showing messages from the server.

## Root Cause

The `start_traceroute_sender()` function in `server/src/measurements.rs` was sending traceroute probes and tracking them, but never draining the ICMP event queue from the packet tracker. It only sent placeholder messages to the client without actual ICMP data.

Additionally, the router IP address (source of the ICMP error) was not being captured or passed through the system.

## Solution

The fix involved completing the data flow from ICMP listener to client:

### 1. Added Router IP Tracking
- Added `router_ip: Option<String>` field to `TrackedPacketEvent` in `common/src/protocol.rs`
- Modified `icmp_listener.rs` to extract router IP from `recv_from()` socket address
- Updated `packet_tracker.match_icmp_error()` to accept and store router IP

### 2. Consume and Process Events
- Modified `start_traceroute_sender()` to drain ICMP events from packet tracker
- Extract hop number from event's send_options TTL
- Calculate RTT from event timestamps
- Create proper `TraceHopMessage` with actual router IP and RTT
- Send messages to client via control channel

### 3. Code Quality Improvements
- Extracted `format_traceroute_message()` helper function to reduce duplication
- Changed `unwrap_or()` to `expect()` for better error messages
- Updated packet tracking API to include router_ip

## Complete Data Flow

1. **Server sends traceroute probe** (measurements.rs)
   - Probe sent with specific TTL via `send_with_options()`
   - Packet tracked with destination, UDP length, and TTL

2. **ICMP error arrives** (icmp_listener.rs)
   - Raw ICMP socket receives packet
   - Router IP extracted from socket address
   - ICMP packet parsed to extract embedded UDP info

3. **Event matched and queued** (packet_tracker.rs)
   - ICMP embedded info matched against tracked packets by (dest, udp_length)
   - Event created with ICMP packet, UDP packet, cleartext, timestamps, and router IP
   - Event added to queue

4. **Event processed and sent** (measurements.rs)
   - Events drained from queue
   - RTT calculated from timestamps
   - TraceHopMessage created with hop number, router IP, and RTT
   - Message serialized and sent via control data channel

5. **Client displays message** (client/measurements.rs)
   - Control channel receives message
   - Parsed as TraceHopMessage
   - Displayed in "server-messages" textarea with format: "[Hop N] Hop N via IP (RTT: X.XXms)"

## Files Changed

- `common/src/protocol.rs` - Added router_ip to TrackedPacketEvent
- `server/src/icmp_listener.rs` - Extract and pass router IP
- `server/src/packet_tracker.rs` - Accept and store router IP in events
- `server/src/measurements.rs` - Drain events and send to client
- `server/src/packet_tracking_api.rs` - Include router_ip in API response

## Testing

All existing tests pass:
- `packet_tracker::tests::test_icmp_matching_with_udp_length`
- `packet_tracker::tests::test_icmp_no_match_different_length`
- `packet_tracker::tests::test_packet_tracker_basic`
- `packet_tracker::tests::test_packet_expiry`

Tests updated to include new router_ip parameter.

## Result

ICMP error messages are now properly delivered to the client and displayed in the textbox, showing:
- Hop number (TTL value)
- Router IP address (source of ICMP error)
- Round-trip time in milliseconds
- Formatted message: "Hop N via IP (RTT: X.XXms)"
