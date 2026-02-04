# Issue 003: Missing Sensor Callback Exports in NetPoke Client

## Summary
The standalone camera app exports WASM functions for sensor callbacks (`on_gps_update`, `on_orientation`, `on_magnetometer`, `on_motion`, `set_sensor_overlay_enabled`). These are critical for receiving sensor data from JavaScript. While a global `SENSOR_MANAGER` exists in netpoke's lib.rs, the callback functions are not exported.

## Location
- File: `client/src/lib.rs`
- Reference: `tmp/camera-standalone-for-cross-check/src/lib.rs` lines 136-259

## Current Behavior
The netpoke `client/src/lib.rs` has:
```rust
static SENSOR_MANAGER: Lazy<Mutex<Option<recorder::sensors::SensorManager>>> =
    Lazy::new(|| Mutex::new(None));
```

But it does NOT export the callback functions:
- `on_gps_update()`
- `on_orientation()`
- `on_magnetometer()`
- `on_motion()`
- `set_sensor_overlay_enabled()`

The standalone camera exports these at lines 136-259.

## Expected Behavior
The netpoke client should export these functions so JavaScript can send sensor data to the WASM module:

```rust
#[wasm_bindgen]
pub fn on_gps_update(
    latitude: f64,
    longitude: f64,
    altitude: Option<f64>,
    accuracy: f64,
    altitude_accuracy: Option<f64>,
    heading: Option<f64>,
    speed: Option<f64>,
) {
    // ...
}

#[wasm_bindgen]
pub fn on_orientation(
    alpha: Option<f64>,
    beta: Option<f64>,
    gamma: Option<f64>,
    absolute: bool,
) {
    // ...
}

// etc.
```

## Impact
- **Priority: High**
- Sensor overlay will never show data (GPS, orientation, acceleration)
- Compass will never display (requires orientation data)
- Motion data JSON export will be empty
- One of the key features (sensor tracking) is completely non-functional

## Suggested Implementation
Copy the following functions from `tmp/camera-standalone-for-cross-check/src/lib.rs` to `client/src/lib.rs`:

1. `on_gps_update()` (lines 136-160)
2. `on_orientation()` (lines 162-190)
3. `on_magnetometer()` (lines 192-210)
4. `on_motion()` (lines 212-250)
5. `set_sensor_overlay_enabled()` (lines 252-259)

Update type imports from `crate::types::*` to `crate::recorder::types::*`.

Also need to add JavaScript sensor tracking code to `server/static/nettest.html` that:
1. Requests sensor permissions (iOS requirement)
2. Starts geolocation watch
3. Adds device orientation/motion event listeners
4. Calls the WASM callback functions with sensor data

The JavaScript can be copied from the standalone camera's HTML file.

---
*Created: 2026-02-04*
