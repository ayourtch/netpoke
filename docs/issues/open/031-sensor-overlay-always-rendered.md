# Issue 031: Sensor Overlay Always Rendered Regardless of Toggle

## Summary
The integrated recorder always renders the sensor overlay and compass, ignoring the "Show Sensors" checkbox state. The standalone reference correctly checks `is_overlay_enabled()` before rendering, but the integrated version skips this check.

## Location
- **Integrated Code**: `client/src/recorder/state.rs` (lines 280-298)
- **Reference Code**: `tmp/camera-standalone-for-cross-check/src/app.rs` (lines 331-350)

## Current Behavior

In the integrated `state.rs`:
```rust
// Render sensor overlay if we have sensor data
if let Ok(manager_guard) = crate::SENSOR_MANAGER.lock() {
    if let Some(ref mgr) = *manager_guard {
        let motion_data = mgr.get_motion_data();
        if let Some(latest) = motion_data.last() {
            let _ = renderer.render_sensor_overlay(...);
            let _ = renderer.render_compass(latest.camera_direction);
        }
    }
}
```

The sensor overlay and compass are **always rendered** if sensor data exists.

## Expected Behavior

Should match the standalone reference in `app.rs`:
```rust
// Render sensor overlay if enabled
if let Ok(manager_guard) = crate::SENSOR_MANAGER.lock() {
    if let Some(ref mgr) = *manager_guard {
        if mgr.is_overlay_enabled() {   // <-- This check is missing!
            let timestamp = js_sys::Date::new_0().to_iso_string().as_string().unwrap();
            let camera_direction = mgr.get_current_camera_direction();
            let _ = renderer.render_sensor_overlay(...);
            let _ = renderer.render_compass(camera_direction);
        }
    }
}
```

The sensor overlay and compass should only render when the "Show Sensors" checkbox is checked.

## Impact
**Priority**: Medium

- The "Show Sensors" checkbox in the recording panel UI does nothing
- Users cannot disable the sensor overlay on recordings
- Recordings will always have the sensor data panel overlayed on the video, even when users don't want it
- This degrades the user experience for recordings where sensor data visibility is not desired

## Suggested Implementation

### Fix in client/src/recorder/state.rs

Update the `render_frame()` method (around line 280) to add the `is_overlay_enabled()` check:

```rust
// Render sensor overlay if we have sensor data AND overlay is enabled
if let Ok(manager_guard) = crate::SENSOR_MANAGER.lock() {
    if let Some(ref mgr) = *manager_guard {
        if mgr.is_overlay_enabled() {  // Add this check
            let motion_data = mgr.get_motion_data();
            if let Some(latest) = motion_data.last() {
                let _ = renderer.render_sensor_overlay(
                    &latest.timestamp_utc,
                    &latest.gps,
                    &latest.magnetometer,
                    &latest.orientation,
                    &Some(latest.acceleration.clone()),
                    &latest.camera_direction,
                );

                // Render compass if we have camera direction
                let _ = renderer.render_compass(latest.camera_direction);
            }
        }
    }
}
```

## Related Issues
- Issue 015: sensor-overlay-toggle-not-wired - The toggle was wired up in UI, but the render check was not implemented

## Verification
1. Start a recording with "Show Sensors" checkbox checked - overlay should appear
2. Start a recording with "Show Sensors" checkbox unchecked - overlay should NOT appear
3. Toggle the checkbox mid-recording and verify overlay visibility changes

---
*Created: 2026-02-05*
