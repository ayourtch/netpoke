# Issue 063: Dataplane Logging Performance

## Summary

Hot-path / dataplane code in the server has excessive logging that impacts performance.
Debug-level logging fires on every packet in ICMP listeners, packet tracker, and data channel
message handlers. Two leftover `XXXXX` debug print statements fire unconditionally at `info!`
level for every single message and test-probe echo.

## Location

- `server/src/data_channels.rs` — line 59 (`info!("XXXXX ...")`) fires every message
- `server/src/measurements.rs` — line 917 (`info!("XXXXX ...")`) fires every test probe echo
- `server/src/packet_tracker.rs` — ~20 `debug!` calls in `match_icmp_error()` and `tracking_receiver_task()` fire per-packet
- `server/src/icmp_listener.rs` — ~30 `debug!` calls in receive loops and ICMP parsing fire per-packet
- `server/src/data_channels.rs` — `handle_message()` logs `info!` for every message
- `server/src/measurements.rs` — loop-exit messages at `info!` level, per-probe `debug!` logging

## Current Behavior

- `XXXXX` debug remnants produce info-level log spam on every WebRTC message and test probe echo
- Per-packet `debug!` logging in ICMP listener and packet tracker creates overhead even when
  debug level is not enabled (format string arguments are still evaluated in some cases)
- `info!`-level logging for every received WebRTC data channel message creates log noise
- Duplicate logging (same event logged at both `error!` and `debug!` level)
- Redundant summary logging (same parse result logged in both itemized and single-line form)

## Expected Behavior

- No `XXXXX` debug remnants in production code
- Per-packet logging at `trace!` level (only visible when explicitly enabled)
- Per-message logging at `debug!` level (not `info!`)
- No duplicate logging for the same event
- Loop lifecycle messages at `debug!` level

## Impact

Performance overhead from string formatting and logging in the hottest code paths. Log noise
obscures important messages when running at info or debug level.

## Root Cause Analysis

Development debug logging was left in place and not cleaned up before merge. Logging levels
were not reviewed for dataplane code paths where per-packet overhead matters.

## Suggested Implementation

1. Remove `XXXXX` debug lines entirely
2. Downgrade per-packet `debug!` → `trace!` in packet_tracker.rs, icmp_listener.rs
3. Remove duplicate logging (error+debug for same event in icmp_listener.rs)
4. Remove redundant summary logs in ICMP parse functions (keep single consolidated log)
5. Downgrade per-message `info!` → `debug!` in data_channels.rs `handle_message()`
6. Downgrade loop lifecycle `info!` → `debug!` in measurements.rs

## Resolution

All suggested changes implemented:

### Files Modified
- **`server/src/data_channels.rs`**: Removed `XXXXX` debug remnant (line 59); downgraded per-message `info!` → `debug!` in `handle_message()`
- **`server/src/measurements.rs`**: Removed `XXXXX` debug remnant (line 917); downgraded loop-exit `info!` → `debug!` for probe/bulk/measurement senders and stats reporter; downgraded per-probe/testprobe `debug!` → `trace!` in echo handlers; downgraded MTU drain polling `debug!` → `trace!`
- **`server/src/packet_tracker.rs`**: Downgraded ~20 per-packet `debug!` → `trace!` in `match_icmp_error()`, `tracking_receiver_task()`, and `drain_events_for_conn_id()`; also fixed typo "push bach" → "push back"
- **`server/src/icmp_listener.rs`**: Downgraded ~30 per-packet `debug!` → `trace!` in receive loops, `parse_icmp_error()`, and `parse_icmpv6_error()`; removed duplicate error+debug logging for recv errors; removed redundant init-time duplicate debug messages; consolidated itemized summary logs into single consolidated log per parse

### Verification
- `cargo check` passes with same 27 pre-existing warnings (no new warnings)
- Pre-existing test compilation failures are unrelated (missing `CONN_ID_HASH_RANGE`, `hash_conn_id` etc.)

