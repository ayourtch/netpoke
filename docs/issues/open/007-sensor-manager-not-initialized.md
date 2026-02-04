# Issue 007: SensorManager Not Initialized Before Recording

## Summary
The global `SENSOR_MANAGER` in netpoke's lib.rs is never initialized before recording starts. The standalone camera app creates and initializes the SensorManager at the start of tracking, but the netpoke integration lacks this initialization.

## Location
- File: `client/src/lib.rs` - Global SENSOR_MANAGER definition (lines 16-17)
- File: `client/src/recorder/state.rs` - Recording start (line 57+)
- Reference: `tmp/camera-standalone-for-cross-check/src/app.rs` - start_tracking() (lines 89-122)

## Current Behavior
In `client/src/lib.rs`:
```rust
static SENSOR_MANAGER: Lazy<Mutex<Option<recorder::sensors::SensorManager>>> =
    Lazy::new(|| Mutex::new(None));
```

In `client/src/recorder/state.rs` `start_recording()`:
The code attempts to read from `crate::SENSOR_MANAGER` (line 210) but never initializes it.

## Expected Behavior
The standalone camera initializes the SensorManager at tracking start:
```rust
// In start_tracking():
self.start_time = js_sys::Date::now();

let camera_facing = match source_type {
    SourceType::Camera => CameraFacing::User,
    SourceType::Combined => CameraFacing::User,
    SourceType::Screen => CameraFacing::Unknown,
};

let mut new_sensor_manager = SensorManager::new(self.start_time, camera_facing);

// Get checkbox state and set overlay enabled
if let Some(checkbox) = document.get_element_by_id("showSensorsOverlay") {
    if let Ok(input) = checkbox.dyn_into::<HtmlInputElement>() {
        new_sensor_manager.set_overlay_enabled(input.checked());
    }
}

self.sensor_manager = Some(new_sensor_manager.clone());

// Update global sensor manager
if let Ok(mut global_mgr) = crate::SENSOR_MANAGER.lock() {
    *global_mgr = Some(new_sensor_manager);
}
```

## Impact
- **Priority: High**
- `SENSOR_MANAGER` will always be `None`
- All sensor data lookups in `render_frame()` will fail
- Motion data in saved recordings will be empty
- Sensor overlay and compass will never render

## Suggested Implementation
In `start_recording()` in `client/src/recorder/state.rs`, add sensor manager initialization:

```rust
pub async fn start_recording(&mut self) -> Result<(), JsValue> {
    use crate::recorder::utils::log;
    use crate::recorder::types::CameraFacing;
    use crate::recorder::sensors::SensorManager;

    log("[Recorder] Starting recording");

    // Get document
    let document = web_sys::window()
        .ok_or("No window")?
        .document()
        .ok_or("No document")?;

    // Initialize start time
    let start_time = js_sys::Date::now();
    self.start_time = start_time;

    // Determine camera facing based on source type
    let camera_facing = match self.source_type {
        SourceType::Camera | SourceType::Combined => CameraFacing::User,
        SourceType::Screen => CameraFacing::Unknown,
    };

    // Create and initialize sensor manager
    let mut sensor_manager = SensorManager::new(start_time, camera_facing);
    
    // Check if sensor overlay checkbox is checked
    if let Some(checkbox) = document.get_element_by_id("show-sensors-overlay") {
        if let Ok(input) = checkbox.dyn_into::<web_sys::HtmlInputElement>() {
            sensor_manager.set_overlay_enabled(input.checked());
        }
    }

    // Update global sensor manager
    if let Ok(mut global_mgr) = crate::SENSOR_MANAGER.lock() {
        *global_mgr = Some(sensor_manager);
    }

    // ... rest of the function
}
```

Also add cleanup in `stop_recording()`:
```rust
// Clear global sensor manager
if let Ok(mut manager_guard) = crate::SENSOR_MANAGER.lock() {
    if let Some(ref mut mgr) = *manager_guard {
        mgr.clear();
    }
}
```

---
*Created: 2026-02-04*
