# 067 - Sensor Data Empty Due to Late SensorManager Initialization

## Summary

Sensor data uploaded to the server contains only `[]` (empty JSON array, 2 bytes). The SensorManager is initialized too late in `start_recording()` — after the async media stream acquisition — causing sensor events fired during that setup phase to be silently dropped.

## Location

- **File**: `client/src/recorder/state.rs`
- **Function**: `start_recording()`, lines 154-172 (SensorManager init)
- **Function**: `stop_recording()`, lines 397-406 (motion data retrieval)

## Current Behavior

1. User clicks "Start Recording"
2. `requestSensorPermissions()` registers `devicemotion` / `deviceorientation` event listeners
3. `startSensorTracking()` starts GPS
4. `start_recording()` begins:
   - Gets media streams via `get_camera_stream().await` (can take seconds due to permission dialogs)
   - **Only then** creates SensorManager and stores it in `SENSOR_MANAGER` global
5. During the async media acquisition (step 4), sensor events fire but `SENSOR_MANAGER` is `None`, so events are silently dropped by `on_motion()`, `on_orientation()`, etc.
6. On stop, `motion_data` is empty → saved as `[]` in IndexedDB → uploaded as `[]` to server

The `on_motion()` handler in `lib.rs` silently drops events when SENSOR_MANAGER is None:
```rust
if let Ok(mut manager_guard) = SENSOR_MANAGER.lock() {
    if let Some(ref mut mgr) = *manager_guard {
        mgr.add_motion_event(...);
    }
    // else: silently dropped!
}
```

## Expected Behavior

The SensorManager should be initialized **before** any async operations, so sensor events are captured from the moment event listeners are registered.

## Impact

All sensor data uploads result in empty files on the server. Users collecting sensor data during surveys cannot retrieve it later for analysis.

## Root Cause Analysis

Initialization order in `start_recording()`:
1. Set `start_time` (line 70-71) ← synchronous
2. Get media streams (lines 73-152) ← **async, sensor events dropped here**
3. Create SensorManager (lines 154-172) ← too late!

The SensorManager creation depends only on `start_time` and `source_type`, both available before step 2.

## Suggested Implementation

1. Move SensorManager initialization (lines 154-172) to before the media stream acquisition (line 73)
2. Add diagnostic logging in `stop_recording()` to report the number of sensor data points collected

## Resolution

Moved SensorManager initialization to before async media stream operations in `start_recording()`:

**Files modified:**
- `client/src/recorder/state.rs`: Moved SensorManager creation block from after media stream acquisition to immediately after `start_time` initialization (before any `.await` calls). Added diagnostic logging in `stop_recording()` to report collected data point count and warn on edge cases (None manager, lock failure).
- `client/src/recorder/ui.rs`: Added SensorManager cleanup in the start-recording error handler to clear the manager if recording fails to start.

**Verification:**
- `cargo check -p netpoke-client` passes with no new warnings
- `cargo check -p netpoke-server` passes with no new warnings
