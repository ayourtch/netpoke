# Camera Recorder - Current State

**Last Updated:** 2026-02-04
**APP_VERSION:** 8

## Project Overview

WASM-based camera/screen recorder with sensor tracking. Records video to IndexedDB with GPS, orientation, acceleration, and magnetometer data. Displays sensor overlay on video during recording.

## What's Working ✅

### Core Features
- **Video Recording:** Camera, screen, and combined (PiP) modes working
- **IndexedDB Storage:** Video blobs and motion data saved locally
- **Video Download:** Download recordings as .webm files
- **Motion Data Export:** Download sensor data as JSON
- **Cache Busting:** Manual version increment system (APP_VERSION)

### Sensor Tracking (iOS)
- **GPS:** ✅ Working - shows position, accuracy
- **Orientation:** ✅ Working - alpha/beta/gamma values showing
- **Acceleration:** ✅ Working - x/y/z values showing
- **Magnetometer:** ⚠️ Not working on iOS Safari (expected - see Known Issues)

### UI
- Sensor overlay renders on canvas (baked into video)
- Checkbox to toggle sensor display (checked by default)
- Debug log visible on screen for mobile debugging
- Overlay positioning: x=80px, y=30px, left-aligned text

## Known Issues & Decisions

### iOS Magnetometer (`deviceorientationabsolute`)
**Status:** Event doesn't fire on iOS Safari

**Root Cause:** iOS Safari doesn't support `deviceorientationabsolute` event, even though hardware has compass.

**Workaround Available:** Compass heading is available in regular `deviceorientation` event's `alpha` value when `absolute: true`. Not yet implemented.

**Evidence:**
- Debug log shows orientation events firing
- No `deviceorientationabsolute` events ever fire
- This is documented Safari limitation

**Next Step:** Update code to extract compass heading from orientation data's alpha value instead of waiting for deviceorientationabsolute event.

### iOS Sensor Permission Requirements
**Critical Discovery:** iOS requires event listeners to be attached in the **same synchronous task** as permission request. Any `await` between permission and listener attachment breaks it.

**Current Implementation:**
1. Button click → `requestSensorPermissions()` (awaits permission dialogs)
2. Immediately adds event listeners (before returning)
3. Then → `start_tracking()` → `startSensorTracking()` (starts GPS only)

**Location:** `index.html` lines 262-311

## Architecture

### Technology Stack
- **Frontend:** Vanilla JavaScript + WASM
- **Backend:** Rust (compiled to WASM via wasm-pack)
- **Storage:** IndexedDB
- **Build:** wasm-pack 0.13.1

### Key Files

#### Rust/WASM (src/)
- **src/lib.rs** - WASM exports, global SENSOR_MANAGER, sensor callbacks
- **src/app.rs** - Main application state and tracking logic
- **src/ui.rs** - Button handlers, `request_sensor_permissions()` helper
- **src/sensors.rs** - SensorManager struct, motion data collection
- **src/types.rs** - Data structures (MotionDataPoint, GpsData, etc.)
- **src/canvas_renderer.rs** - Video rendering + sensor overlay (line 219+)
- **src/storage.rs** - IndexedDB wrapper
- **src/recorder.rs** - MediaRecorder integration

#### JavaScript
- **index.html** - Main UI, sensor bridge (lines 173-400)
  - `debugLog()` - Logs to console and on-screen debug panel
  - `requestSensorPermissions()` - Requests iOS permissions + attaches listeners
  - `startSensorTracking()` - Starts GPS only (listeners already attached)
  - `handleMotion()`, `handleOrientation()`, `handleMagnetometer()` - Event handlers
- **js/indexed_db.js** - IndexedDB operations
- **js/media_recorder.js** - MediaRecorder setup

### Data Flow

**Sensor Data Collection:**
1. User clicks start button → Rust button handler (src/ui.rs:138)
2. Rust calls `request_sensor_permissions()` → JavaScript `requestSensorPermissions()`
3. JavaScript requests iOS permissions → immediately adds event listeners
4. Rust continues → `start_tracking()` → calls JavaScript `startSensorTracking()` for GPS
5. JavaScript sensor events fire → call WASM exports (`on_gps_update`, `on_orientation`, `on_motion`)
6. WASM stores in global `SENSOR_MANAGER` (src/lib.rs:17)
7. Canvas render loop reads from `SENSOR_MANAGER` → draws overlay (src/canvas_renderer.rs:219)
8. Recording stops → reads motion data from global `SENSOR_MANAGER` → saves to IndexedDB

**Critical:** Must read from global `SENSOR_MANAGER`, not local AppState copy, because callbacks update global only.

### Global State

```rust
// src/lib.rs:17
static SENSOR_MANAGER: Lazy<Mutex<Option<crate::sensors::SensorManager>>> =
    Lazy::new(|| Mutex::new(None));
```

JavaScript callbacks (on_gps_update, on_orientation, on_motion) update this global state.

## File Modifications Summary

### Recent Changes (Sensor Tracking Implementation)
- Added: src/sensors.rs (new file)
- Modified: src/types.rs - Added sensor data structures with #[serde(default)]
- Modified: src/lib.rs - Added SENSOR_MANAGER, 6 WASM exports
- Modified: src/app.rs - Integrated SensorManager lifecycle
- Modified: src/canvas_renderer.rs - Added render_sensor_overlay()
- Modified: src/storage.rs - Accept motion_data parameter
- Modified: src/ui.rs - Added request_sensor_permissions() helper
- Modified: js/indexed_db.js - Store motionData field
- Modified: index.html - Sensor bridge, debug logging, permission handling
- Added: CACHE_BUSTING.md - Documents manual version approach
- Modified: Cargo.toml - Added once_cell dependency

### Critical Bug Fixes
1. **Empty motion data:** Fixed by reading from global SENSOR_MANAGER (was reading local copy)
2. **iOS permission error:** Split permission request into separate function called from button handler
3. **Events not firing:** Moved listener attachment into permission function (same sync task requirement)

## Current Code Snippets

### Sensor Permission Flow (src/ui.rs)
```rust
async fn request_sensor_permissions() -> Result<(), JsValue> {
    crate::utils::log("[RUST] request_sensor_permissions called");
    let window = web_sys::window().ok_or("No window")?;
    let request_fn = js_sys::Reflect::get(&window, &"requestSensorPermissions".into())?;

    if !request_fn.is_function() {
        return Err(JsValue::from_str("requestSensorPermissions not found"));
    }

    let request_fn: js_sys::Function = request_fn.dyn_into()?;
    let promise: js_sys::Promise = request_fn.call0(&window)?.dyn_into()?;
    let result = wasm_bindgen_futures::JsFuture::from(promise).await?;

    if result.is_truthy() {
        Ok(())
    } else {
        Err(JsValue::from_str("Sensor permissions denied"))
    }
}
```

### JavaScript Permission Handler (index.html:262)
```javascript
window.requestSensorPermissions = async function() {
    debugLog('[SENSOR] requestSensorPermissions called - debugLog');

    // Request iOS permissions
    if (typeof DeviceMotionEvent !== 'undefined' &&
        typeof DeviceMotionEvent.requestPermission === 'function') {
        const motionPermission = await DeviceMotionEvent.requestPermission();
        if (motionPermission !== 'granted') return false;
    }

    if (typeof DeviceOrientationEvent !== 'undefined' &&
        typeof DeviceOrientationEvent.requestPermission === 'function') {
        const orientationPermission = await DeviceOrientationEvent.requestPermission();
        if (orientationPermission !== 'granted') return false;
    }

    // CRITICAL: Add listeners immediately (same sync task)
    if (!sensorListenersActive) {
        window.addEventListener('devicemotion', handleMotion);
        window.addEventListener('deviceorientation', handleOrientation);
        window.addEventListener('deviceorientationabsolute', handleMagnetometer);
        sensorListenersActive = true;
    }

    return true;
};
```

### Sensor Overlay Rendering (src/canvas_renderer.rs:219)
```rust
pub fn render_sensor_overlay(
    &self,
    timestamp_utc: &str,
    gps: &Option<crate::types::GpsData>,
    magnetometer: &Option<crate::types::OrientationData>,
    orientation: &Option<crate::types::OrientationData>,
    acceleration: &Option<crate::types::AccelerationData>,
) -> Result<(), JsValue> {
    let ctx = &self.ctx;

    // Background panel
    ctx.set_fill_style(&JsValue::from_str("rgba(0, 0, 0, 0.8)"));
    ctx.fill_rect(15.0, 15.0, 400.0, 120.0);

    // Text style
    ctx.set_fill_style(&JsValue::from_str("#ffffff"));
    ctx.set_font("12px monospace");
    ctx.set_text_align("left");
    ctx.set_text_baseline("top");

    let mut y = 30.0;
    let x = 80.0;
    let line_height = 18.0;

    // Render sensor data...
}
```

## Debug Tools

### On-Screen Debug Log
- Green panel at bottom of page
- Shows all [SENSOR] and [RUST] prefixed messages
- Auto-scrolls
- Visible on mobile devices
- Added in index.html:176-179

### Cache Busting
- **Method:** Manual version increment
- **Location:** index.html line 175: `const APP_VERSION = 8;`
- **Process:**
  1. Run `wasm-pack build --target web --out-dir pkg`
  2. Increment APP_VERSION in index.html
  3. Deploy both files
- **Documentation:** CACHE_BUSTING.md

## Next Steps

### Immediate (Magnetometer Fix)
1. Remove `deviceorientationabsolute` listener (Safari doesn't support)
2. Use `deviceorientation` event's `alpha` value for compass heading
3. Check `event.absolute` flag to determine if alpha is compass or relative
4. Update magnetometer UI to show orientation.alpha when absolute=true

### Future Enhancements
- Add progress indicators during recording
- Add verification before completion
- Clean up deprecated warnings in canvas_renderer.rs
- Remove unused utility functions

## Testing Notes

### Verified Working on iOS Safari
- GPS acquiring and showing coordinates
- Orientation showing alpha/beta/gamma values
- Acceleration showing x/y/z values
- Permission dialogs appear on first button click
- Sensor overlay renders on video
- Debug log shows detailed flow
- Video records and saves to IndexedDB

### Known Not Working
- deviceorientationabsolute events (Safari limitation)

## Build Commands

```bash
# Build WASM
wasm-pack build --target web --out-dir pkg

# After build, increment APP_VERSION in index.html

# Deploy (example)
make deploy  # or your deployment command
```

## Important Patterns

### Reading Sensor Data
Always read from global SENSOR_MANAGER, never from AppState.sensor_manager:

```rust
// ✅ CORRECT
let motion_data = if let Ok(manager_guard) = crate::SENSOR_MANAGER.lock() {
    if let Some(ref mgr) = *manager_guard {
        mgr.get_motion_data().clone()
    } else {
        Vec::new()
    }
} else {
    Vec::new()
};

// ❌ WRONG - local copy doesn't receive callback updates
let motion_data = self.sensor_manager.get_motion_data();
```

### iOS Permissions
Always request in button click handler before any async operations:

```rust
// Button handler
let closure = Closure::wrap(Box::new(move || {
    wasm_bindgen_futures::spawn_local(async move {
        // Request permissions FIRST (still in gesture context)
        if let Err(e) = request_sensor_permissions().await {
            return;
        }

        // Then start recording
        let result = app.borrow_mut().start_tracking(SourceType::Camera).await;
    });
}));
```

## Related Documentation

- **Design:** docs/plans/2026-01-30-sensor-tracking-design.md
- **Implementation Plan:** docs/plans/2026-01-30-sensor-tracking-implementation.md
- **Cache Busting:** CACHE_BUSTING.md
- **Original Implementation:** camera-tracker.html (reference)

## Session Context

This state document was created after successfully implementing sensor tracking with GPS, orientation, and acceleration working on iOS. The only remaining issue is extracting compass heading from orientation data instead of relying on unsupported deviceorientationabsolute event.

**To resume:**
1. Read this STATE.md
2. Review the magnetometer issue in "Known Issues"
3. Implement the fix described in "Next Steps - Immediate"
4. Test on iOS device
5. Update APP_VERSION and redeploy
