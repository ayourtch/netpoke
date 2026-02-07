# Issue 065: Metrics Collected After Test Stop

## Summary

Server-side probe stats reporter and measurement probe sender continue running after the client stops testing, leading to stale metrics being recorded to the database.

## Location

- `server/src/measurements.rs`: `start_probe_stats_reporter()`, `start_measurement_probe_sender()`
- `server/src/signaling.rs`: `on_peer_connection_state_change` handler

## Current Behavior

When a client stops testing (by closing the browser tab, clicking stop, or disconnecting):

1. The client calls `stop_testing()` which calls `clear_active_resources()` to close peer connections
2. Client-side intervals are cleared, but no explicit `StopProbeStreams` message is sent to the server before disconnect
3. On the server side, `probe_streams_active` flag is only set to `false` when a `StopProbeStreams` message is received
4. The `start_probe_stats_reporter()` loop only checks `probe_streams_active` - it does NOT check if the control channel or peer connection is still alive
5. The stats reporter continues calculating stats from stale (rolling window) data and recording them to the database
6. Similarly, `start_measurement_probe_sender()` checks `probe_streams_active` before checking channel state

## Expected Behavior

When a client disconnects, the server should:
1. Detect the disconnection via peer connection state change
2. Set `probe_streams_active = false` and `traffic_active = false` on the session's measurement state
3. Stop the probe stats reporter and measurement probe sender immediately
4. Not record any more metrics for that session

## Impact

- Database accumulates stale/ghost metrics entries after tests complete
- Resource waste on the server (tasks keep running unnecessarily)
- Inaccurate data in the survey metrics database

## Root Cause Analysis

The `on_peer_connection_state_change` handler in `signaling.rs` only handles `RTCPeerConnectionState::Connected` to populate the peer address. It does not handle `Disconnected` or `Failed` states to clean up the measurement state.

Additionally, the `start_probe_stats_reporter` loop doesn't check the control channel readiness, so even if the channel closes, the reporter keeps running.

## Suggested Implementation

1. In `signaling.rs`, add handlers for `Disconnected` and `Failed` states that set `probe_streams_active = false` and `traffic_active = false`
2. In `start_probe_stats_reporter`, add a check for control channel readiness before recording
3. In `start_probe_stats_reporter`, add a check for peer connection state

## Resolution

### Changes Made

1. **`server/src/signaling.rs`**: Added handling for `Disconnected`, `Failed`, and `Closed` peer connection states in `on_peer_connection_state_change`. When any of these states is detected, `traffic_active` and `probe_streams_active` are set to `false`, which causes all measurement loops (probe sender, bulk sender, stats reporter) to exit.

2. **`server/src/measurements.rs`**: Added a control channel readiness check in `start_probe_stats_reporter()`. If the control channel is no longer open, the reporter stops and sets `probe_streams_active = false`. This provides a secondary detection mechanism beyond the peer connection state change.

### Verification

- Compilation passes (`cargo check`)
- The fix addresses all three entry points for stopping metrics:
  1. Explicit `StopProbeStreams` message from client (existing)
  2. Peer connection state change to Disconnected/Failed/Closed (new)
  3. Control channel closing (new)
