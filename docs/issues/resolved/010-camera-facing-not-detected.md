# Issue 010: Camera Facing Detection Not Implemented

## Summary
The standalone camera app detects which camera is being used (front/user vs back/environment) and adjusts the compass direction calculation accordingly. The netpoke implementation always defaults to `CameraFacing::Unknown`, so compass direction is never calculated correctly.

## Location
- File: `client/src/recorder/state.rs` - `stop_recording()` line 280
- File: `client/src/recorder/sensors.rs` - `calculate_camera_direction()` line 126-128
- Reference: `tmp/camera-standalone-for-cross-check/src/app.rs` lines 99-104

## Current Behavior
In `client/src/recorder/state.rs` `stop_recording()`:
```rust
let metadata = RecordingMetadata {
    // ...
    camera_facing: CameraFacing::Unknown,  // Always Unknown!
    // ...
};
```

In `client/src/recorder/sensors.rs` `calculate_camera_direction()`:
```rust
let camera_direction = match self.camera_facing {
    CameraFacing::Environment => device_heading,
    CameraFacing::User => (device_heading + 180.0) % 360.0,
    CameraFacing::Unknown => return None,  // Returns None when Unknown
};
```

## Expected Behavior
The standalone camera determines camera facing based on source type:
```rust
let camera_facing = match source_type {
    SourceType::Camera => CameraFacing::User,    // Front camera
    SourceType::Combined => CameraFacing::User,  // Front camera for PiP
    SourceType::Screen => CameraFacing::Unknown, // No camera
};
```

Then passes this to the SensorManager:
```rust
let sensor_manager = SensorManager::new(self.start_time, camera_facing);
```

## Impact
- **Priority: Medium**
- Compass will never display (camera_direction always None for Unknown)
- "Camera facing" field in recordings is always "unknown"
- Useful directional metadata is lost
- Compass feature from design is non-functional

## Suggested Implementation
1. In `start_recording()`, determine camera facing from source type:
   ```rust
   let camera_facing = match self.source_type {
       SourceType::Camera | SourceType::Combined => CameraFacing::User,
       SourceType::Screen => CameraFacing::Unknown,
   };
   ```

2. Pass this to SensorManager initialization (see Issue 007)

3. In `stop_recording()`, get camera facing from the sensor manager:
   ```rust
   let camera_facing = if let Ok(manager_guard) = crate::SENSOR_MANAGER.lock() {
       if let Some(ref mgr) = *manager_guard {
           mgr.get_camera_facing()
       } else {
           CameraFacing::Unknown
       }
   } else {
       CameraFacing::Unknown
   };
   
   let metadata = RecordingMetadata {
       // ...
       camera_facing,
       // ...
   };
   ```

**Future Enhancement:**
For mobile devices with multiple cameras, could potentially detect actual camera in use from MediaStreamTrack.getSettings().facingMode. This would require additional code to query the track.

---
*Created: 2026-02-04*

## Resolution
**Fixed in commit 6854bb8**

Updated `client/src/recorder/state.rs`:
1. In `start_recording()`: Set camera_facing based on source_type when creating SensorManager
2. In `stop_recording()`: Retrieved actual camera_facing from SENSOR_MANAGER instead of hardcoding Unknown
3. Recording metadata now contains correct camera facing information

Files modified:
- `client/src/recorder/state.rs`

---
*Resolved: 2026-02-04*
