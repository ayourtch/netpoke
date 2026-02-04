# Sensor Tracking Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add comprehensive sensor tracking (GPS, magnetometer, orientation, acceleration) to WASM camera recorder with optional canvas overlay and motion data export.

**Architecture:** JavaScript sensor APIs feed data via WASM bindings into Rust SensorManager. Motion data stored alongside video in IndexedDB. Optional canvas overlay renders sensor readings directly into video recording.

**Tech Stack:** Rust/WASM, wasm-bindgen, serde, web-sys, IndexedDB, JavaScript Sensor APIs (Geolocation, DeviceMotion, DeviceOrientation)

---

## Task 1: Add Sensor Data Types

**Files:**
- Modify: `src/types.rs`

**Step 1: Add sensor data structures to types.rs**

Add after the existing `RecordingMetadata` struct:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionDataPoint {
    pub timestamp_relative: f64,
    pub timestamp_utc: String,
    pub gps: Option<GpsData>,
    pub magnetometer: Option<OrientationData>,
    pub orientation: Option<OrientationData>,
    pub acceleration: AccelerationData,
    pub acceleration_including_gravity: AccelerationData,
    pub rotation_rate: RotationData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpsData {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: Option<f64>,
    pub accuracy: f64,
    pub altitude_accuracy: Option<f64>,
    pub heading: Option<f64>,
    pub speed: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrientationData {
    pub alpha: Option<f64>,
    pub beta: Option<f64>,
    pub gamma: Option<f64>,
    pub absolute: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccelerationData {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationData {
    pub alpha: f64,
    pub beta: f64,
    pub gamma: f64,
}
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: SUCCESS (compiles without errors)

**Step 3: Commit**

```bash
git add src/types.rs
git commit -m "feat: add sensor data type definitions

Add MotionDataPoint and related sensor data structures for GPS,
magnetometer, orientation, acceleration, and rotation tracking.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 2: Create Sensor Manager Module

**Files:**
- Create: `src/sensors.rs`
- Modify: `src/lib.rs`

**Step 1: Create sensors.rs module**

Create `src/sensors.rs`:

```rust
use crate::types::{AccelerationData, GpsData, MotionDataPoint, OrientationData, RotationData};

pub struct SensorManager {
    motion_data: Vec<MotionDataPoint>,
    current_gps: Option<GpsData>,
    current_magnetometer: Option<OrientationData>,
    current_orientation: Option<OrientationData>,
    current_acceleration: Option<AccelerationData>,
    current_acceleration_g: Option<AccelerationData>,
    current_rotation: Option<RotationData>,
    start_time: f64,
    overlay_enabled: bool,
}

impl SensorManager {
    pub fn new(start_time: f64) -> Self {
        Self {
            motion_data: Vec::new(),
            current_gps: None,
            current_magnetometer: None,
            current_orientation: None,
            current_acceleration: None,
            current_acceleration_g: None,
            current_rotation: None,
            start_time,
            overlay_enabled: false,
        }
    }

    pub fn set_overlay_enabled(&mut self, enabled: bool) {
        self.overlay_enabled = enabled;
    }

    pub fn is_overlay_enabled(&self) -> bool {
        self.overlay_enabled
    }

    pub fn update_gps(&mut self, gps: GpsData) {
        self.current_gps = Some(gps);
    }

    pub fn update_magnetometer(&mut self, mag: OrientationData) {
        self.current_magnetometer = Some(mag);
    }

    pub fn update_orientation(&mut self, orientation: OrientationData) {
        self.current_orientation = Some(orientation);
    }

    pub fn add_motion_event(
        &mut self,
        timestamp_utc: String,
        current_time: f64,
        acceleration: AccelerationData,
        acceleration_g: AccelerationData,
        rotation: RotationData,
    ) {
        self.current_acceleration = Some(acceleration.clone());
        self.current_acceleration_g = Some(acceleration_g.clone());
        self.current_rotation = Some(rotation.clone());

        let data_point = MotionDataPoint {
            timestamp_relative: current_time - self.start_time,
            timestamp_utc,
            gps: self.current_gps.clone(),
            magnetometer: self.current_magnetometer.clone(),
            orientation: self.current_orientation.clone(),
            acceleration,
            acceleration_including_gravity: acceleration_g,
            rotation_rate: rotation,
        };

        self.motion_data.push(data_point);
    }

    pub fn get_motion_data(&self) -> &Vec<MotionDataPoint> {
        &self.motion_data
    }

    pub fn get_current_gps(&self) -> &Option<GpsData> {
        &self.current_gps
    }

    pub fn get_current_magnetometer(&self) -> &Option<OrientationData> {
        &self.current_magnetometer
    }

    pub fn get_current_orientation(&self) -> &Option<OrientationData> {
        &self.current_orientation
    }

    pub fn get_current_acceleration(&self) -> &Option<AccelerationData> {
        &self.current_acceleration
    }

    pub fn clear(&mut self) {
        self.motion_data.clear();
        self.current_gps = None;
        self.current_magnetometer = None;
        self.current_orientation = None;
        self.current_acceleration = None;
        self.current_acceleration_g = None;
        self.current_rotation = None;
    }
}
```

**Step 2: Add sensors module to lib.rs**

In `src/lib.rs`, add after other mod declarations:

```rust
mod sensors;
```

**Step 3: Verify it compiles**

Run: `cargo check`
Expected: SUCCESS

**Step 4: Commit**

```bash
git add src/sensors.rs src/lib.rs
git commit -m "feat: add SensorManager module

Create SensorManager to collect and store motion data from JavaScript
sensor callbacks. Tracks GPS, magnetometer, orientation, and acceleration.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 3: Update IndexedDB for Motion Data

**Files:**
- Modify: `js/indexed_db.js`

**Step 1: Update saveRecording to accept motionData**

In `js/indexed_db.js`, update the `saveRecording` function signature and implementation:

```javascript
export async function saveRecording(id, videoBlob, metadata, motionData = []) {
    if (!db) throw new Error('Database not initialized');

    const recording = {
        id,
        videoBlob,
        metadata,
        motionData,  // Add motion data field
        timestamp: Date.now()
    };

    return new Promise((resolve, reject) => {
        const transaction = db.transaction(['recordings'], 'readwrite');
        const store = transaction.objectStore('recordings');
        const request = store.put(recording);

        request.onsuccess = () => resolve();
        request.onerror = () => reject(request.error);
    });
}
```

**Step 2: Test in browser console**

Manual test (after building):
1. Open browser DevTools console
2. Check that recordings still save/load correctly
Expected: No errors, backward compatible with existing recordings

**Step 3: Commit**

```bash
git add js/indexed_db.js
git commit -m "feat: add motionData field to IndexedDB recordings

Update saveRecording to accept and store motion data array alongside
video blob. Defaults to empty array for backward compatibility.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 4: Update Storage Module for Motion Data

**Files:**
- Modify: `src/storage.rs`

**Step 1: Update IndexedDbWrapper::save_recording signature**

In `src/storage.rs`, update the `save_recording` method to accept motion data:

```rust
pub async fn save_recording(
    &self,
    id: &str,
    blob: &web_sys::Blob,
    metadata: &RecordingMetadata,
    motion_data: &[crate::types::MotionDataPoint],
) -> Result<(), JsValue> {
    let metadata_js = serde_wasm_bindgen::to_value(metadata)?;
    let motion_data_js = serde_wasm_bindgen::to_value(motion_data)?;
    saveRecording(id, blob, &metadata_js, &motion_data_js).await
}
```

**Step 2: Update extern declaration**

Update the `saveRecording` extern function signature:

```rust
#[wasm_bindgen(catch)]
pub async fn saveRecording(
    id: &str,
    blob: &web_sys::Blob,
    metadata: &JsValue,
    motion_data: &JsValue,
) -> Result<(), JsValue>;
```

**Step 3: Verify it compiles**

Run: `cargo check`
Expected: SUCCESS

**Step 4: Commit**

```bash
git add src/storage.rs
git commit -m "feat: update storage to handle motion data

Update save_recording to accept and serialize motion data array
alongside video blob and metadata.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 5: Add Canvas Overlay Rendering

**Files:**
- Modify: `src/canvas_renderer.rs`

**Step 1: Read current canvas_renderer.rs structure**

Check the existing structure to understand how to add the overlay method.

**Step 2: Add render_sensor_overlay method**

Add this method to the `CanvasRenderer` impl block:

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

    // Draw background panel
    ctx.set_fill_style(&JsValue::from_str("rgba(0, 0, 0, 0.8)"));
    ctx.fill_rect(15.0, 15.0, 400.0, 120.0);

    // Set text style
    ctx.set_fill_style(&JsValue::from_str("#ffffff"));
    ctx.set_font("12px monospace");

    let mut y = 32.0;
    let x = 25.0;
    let line_height = 18.0;

    // Timestamp
    ctx.fill_text(timestamp_utc, x, y)?;
    y += line_height;

    // GPS
    let gps_text = if let Some(gps_data) = gps {
        format!(
            "GPS: {:.6}, {:.6} ±{:.1}m",
            gps_data.latitude, gps_data.longitude, gps_data.accuracy
        )
    } else {
        "GPS: acquiring...".to_string()
    };
    ctx.fill_text(&gps_text, x, y)?;
    y += line_height;

    // Magnetometer
    let mag_text = if let Some(mag) = magnetometer {
        format!(
            "Magnetometer: heading:{}° β:{}° γ:{}°",
            mag.alpha.map(|v| format!("{:.0}", v)).unwrap_or("-".to_string()),
            mag.beta.map(|v| format!("{:.0}", v)).unwrap_or("-".to_string()),
            mag.gamma.map(|v| format!("{:.0}", v)).unwrap_or("-".to_string())
        )
    } else {
        "Magnetometer: not available".to_string()
    };
    ctx.fill_text(&mag_text, x, y)?;
    y += line_height;

    // Orientation
    let orient_text = if let Some(orient) = orientation {
        format!(
            "Orientation: α:{}° β:{}° γ:{}°",
            orient.alpha.map(|v| format!("{:.0}", v)).unwrap_or("-".to_string()),
            orient.beta.map(|v| format!("{:.0}", v)).unwrap_or("-".to_string()),
            orient.gamma.map(|v| format!("{:.0}", v)).unwrap_or("-".to_string())
        )
    } else {
        "Orientation: -".to_string()
    };
    ctx.fill_text(&orient_text, x, y)?;
    y += line_height;

    // Acceleration
    let accel_text = if let Some(accel) = acceleration {
        format!(
            "Accel: x:{:.2} y:{:.2} z:{:.2}",
            accel.x, accel.y, accel.z
        )
    } else {
        "Accel: -".to_string()
    };
    ctx.fill_text(&accel_text, x, y)?;

    Ok(())
}
```

**Step 3: Verify it compiles**

Run: `cargo check`
Expected: SUCCESS

**Step 4: Commit**

```bash
git add src/canvas_renderer.rs
git commit -m "feat: add sensor overlay rendering to canvas

Add render_sensor_overlay method that draws real-time sensor readings
in top-left corner with semi-transparent background.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 6: Add UI Controls for Sensor Overlay

**Files:**
- Modify: `src/ui.rs`

**Step 1: Add checkbox HTML in render method**

Find where the metrics div is rendered and add the checkbox. Add this in the appropriate location in `UiController::new()` or wherever the HTML structure is set up. If the UI is managed differently, add JavaScript to index.html instead (see Task 8).

For now, note that we'll add the checkbox in the HTML directly in Task 8.

**Step 2: Update recording list template to include motion data button**

In `src/lib.rs`, find the `delete_recording_by_id` function where the recordings list is rendered. Update the HTML template to add the "Download Motion Data" button:

```rust
format!(
    r#"
    <div class="recording-item">
        <div class="data">ID: {} <span class="source-label {}">{}</span></div>
        <div class="data">Date: {}</div>
        <div class="data">Duration: {:.1}s</div>
        <div class="data">Frames: {}</div>
        <div class="data">Size: {:.2} MB</div>
        <button onclick="downloadVideo('{}')">Download Video</button>
        <button onclick="downloadMotionData('{}')">Download Motion Data</button>
        <button class="danger" onclick="deleteRecordingById('{}')">Delete</button>
    </div>
    "#,
    rec.id, source_class, source_label,
    date.to_locale_string("en-US", &JsValue::UNDEFINED).as_string().unwrap(),
    rec.metadata.duration, rec.metadata.frame_count, size_mb,
    rec.id, rec.id, rec.id
)
```

Also update the same template in `src/ui.rs` in the `render_recordings_list` method.

**Step 3: Verify it compiles**

Run: `cargo check`
Expected: SUCCESS

**Step 4: Commit**

```bash
git add src/ui.rs src/lib.rs
git commit -m "feat: add Download Motion Data button to UI

Add button to recording items for downloading motion data as JSON.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 7: Add WASM Exports for Sensor Callbacks

**Files:**
- Modify: `src/lib.rs`

**Step 1: Add global sensor manager**

Add at the top of `src/lib.rs` after imports:

```rust
use std::sync::Mutex;
use once_cell::sync::Lazy;

static SENSOR_MANAGER: Lazy<Mutex<Option<crate::sensors::SensorManager>>> =
    Lazy::new(|| Mutex::new(None));
```

Note: You'll need to add `once_cell` to Cargo.toml dependencies.

**Step 2: Add sensor callback exports**

Add these functions to `src/lib.rs`:

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
    let gps = crate::types::GpsData {
        latitude,
        longitude,
        altitude,
        accuracy,
        altitude_accuracy,
        heading,
        speed,
    };

    if let Ok(mut manager) = SENSOR_MANAGER.lock() {
        if let Some(mgr) = manager.as_mut() {
            mgr.update_gps(gps);
        }
    }
}

#[wasm_bindgen]
pub fn on_orientation(
    alpha: Option<f64>,
    beta: Option<f64>,
    gamma: Option<f64>,
    absolute: bool,
) {
    let orientation = crate::types::OrientationData {
        alpha,
        beta,
        gamma,
        absolute,
    };

    if let Ok(mut manager) = SENSOR_MANAGER.lock() {
        if let Some(mgr) = manager.as_mut() {
            mgr.update_orientation(orientation);
        }
    }
}

#[wasm_bindgen]
pub fn on_magnetometer(
    alpha: Option<f64>,
    beta: Option<f64>,
    gamma: Option<f64>,
) {
    let magnetometer = crate::types::OrientationData {
        alpha,
        beta,
        gamma,
        absolute: true,
    };

    if let Ok(mut manager) = SENSOR_MANAGER.lock() {
        if let Some(mgr) = manager.as_mut() {
            mgr.update_magnetometer(magnetometer);
        }
    }
}

#[wasm_bindgen]
pub fn on_motion(
    accel_x: f64,
    accel_y: f64,
    accel_z: f64,
    accel_g_x: f64,
    accel_g_y: f64,
    accel_g_z: f64,
    rot_alpha: f64,
    rot_beta: f64,
    rot_gamma: f64,
) {
    let acceleration = crate::types::AccelerationData {
        x: accel_x,
        y: accel_y,
        z: accel_z,
    };

    let acceleration_g = crate::types::AccelerationData {
        x: accel_g_x,
        y: accel_g_y,
        z: accel_g_z,
    };

    let rotation = crate::types::RotationData {
        alpha: rot_alpha,
        beta: rot_beta,
        gamma: rot_gamma,
    };

    let timestamp_utc = js_sys::Date::new_0().to_iso_string().as_string().unwrap();
    let current_time = js_sys::Date::now();

    if let Ok(mut manager) = SENSOR_MANAGER.lock() {
        if let Some(mgr) = manager.as_mut() {
            mgr.add_motion_event(timestamp_utc, current_time, acceleration, acceleration_g, rotation);
        }
    }
}

#[wasm_bindgen]
pub fn set_sensor_overlay_enabled(enabled: bool) {
    if let Ok(mut manager) = SENSOR_MANAGER.lock() {
        if let Some(mgr) = manager.as_mut() {
            mgr.set_overlay_enabled(enabled);
        }
    }
}

#[wasm_bindgen]
pub async fn download_motion_data(id: String) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("No window")?;
    let document = window.document().ok_or("No document")?;

    let recording_js = storage::getRecording(&id).await?;
    let obj = js_sys::Object::from(recording_js);

    let metadata_js = js_sys::Reflect::get(&obj, &"metadata".into())?;
    let motion_data_js = js_sys::Reflect::get(&obj, &"motionData".into())?;

    // Create JSON object
    let json_obj = js_sys::Object::new();
    js_sys::Reflect::set(&json_obj, &"id".into(), &JsValue::from_str(&id))?;
    js_sys::Reflect::set(&json_obj, &"metadata".into(), &metadata_js)?;
    js_sys::Reflect::set(&json_obj, &"motionData".into(), &motion_data_js)?;

    let json_string = js_sys::JSON::stringify_with_replacer_and_space(
        &json_obj,
        &JsValue::NULL,
        &JsValue::from_f64(2.0),
    )?;

    // Create blob and download
    let array = js_sys::Array::new();
    array.push(&json_string);
    let blob = web_sys::Blob::new_with_str_sequence_and_options(
        &array,
        web_sys::BlobPropertyBag::new().type_("application/json"),
    )?;

    let url = web_sys::Url::create_object_url_with_blob(&blob)?;
    let a: web_sys::HtmlAnchorElement = document.create_element("a")?.dyn_into()?;
    a.set_href(&url);
    a.set_download(&format!("motion_{}.json", id));
    a.click();

    let url_clone = url.clone();
    let closure = Closure::wrap(Box::new(move || {
        let _ = web_sys::Url::revoke_object_url(&url_clone);
    }) as Box<dyn Fn()>);
    window.set_timeout_with_callback_and_timeout_and_arguments_0(
        closure.as_ref().unchecked_ref(),
        1000,
    )?;
    closure.forget();

    Ok(())
}
```

**Step 3: Add once_cell dependency**

Add to `Cargo.toml` dependencies:

```toml
once_cell = "1.19"
```

**Step 4: Verify it compiles**

Run: `cargo check`
Expected: SUCCESS

**Step 5: Commit**

```bash
git add src/lib.rs Cargo.toml
git commit -m "feat: add WASM exports for sensor callbacks

Add on_gps_update, on_orientation, on_magnetometer, on_motion,
set_sensor_overlay_enabled, and download_motion_data exports for
JavaScript to call. Use global SENSOR_MANAGER for state.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 8: Add JavaScript Sensor Bridge

**Files:**
- Modify: `index.html`

**Step 1: Add sensor overlay checkbox to HTML**

In `index.html`, find the metrics div and add the checkbox before the existing metric displays:

```html
<div id="metrics" style="display:none; margin-top:10px;">
    <div style="margin-bottom: 10px;">
        <label>
            <input type="checkbox" id="showSensorsOverlay">
            Show Sensors in Video
        </label>
    </div>
    <div class="data">Source: <span id="sourceType">-</span></div>
    <div class="data">Frames: <span id="frames">0</span></div>
    <div class="data">GPS: <span id="gps">acquiring...</span></div>
    <div class="data">Magnetometer: <span id="magnetometer">-</span></div>
    <div class="data">Orientation: <span id="orientation">-</span></div>
    <div class="data">Acceleration: <span id="acceleration">-</span></div>
    <div class="data">Recording: <span id="duration">0.0s</span></div>
    <div class="data">Video size: <span id="videoSize">0 MB</span></div>
</div>
```

**Step 2: Add sensor tracking functions to script module**

In `index.html`, update the script module section to add sensor tracking:

```javascript
<script type="module">
    import init, {
        start,
        download_video,
        delete_recording_by_id,
        download_motion_data,
        on_gps_update,
        on_orientation,
        on_magnetometer,
        on_motion,
        set_sensor_overlay_enabled
    } from './pkg/camera_wasm.js';

    // Make functions globally available
    window.downloadVideo = download_video;
    window.deleteRecordingById = delete_recording_by_id;
    window.downloadMotionData = download_motion_data;

    let gpsWatchId = null;
    let sensorListenersActive = false;

    // Sensor event handlers
    function handleMotion(e) {
        on_motion(
            e.acceleration?.x || 0,
            e.acceleration?.y || 0,
            e.acceleration?.z || 0,
            e.accelerationIncludingGravity?.x || 0,
            e.accelerationIncludingGravity?.y || 0,
            e.accelerationIncludingGravity?.z || 0,
            e.rotationRate?.alpha || 0,
            e.rotationRate?.beta || 0,
            e.rotationRate?.gamma || 0
        );

        // Update UI
        const accelEl = document.getElementById('acceleration');
        if (accelEl && e.acceleration) {
            accelEl.textContent = `x:${(e.acceleration.x || 0).toFixed(2)} y:${(e.acceleration.y || 0).toFixed(2)} z:${(e.acceleration.z || 0).toFixed(2)}`;
        }
    }

    function handleOrientation(e) {
        on_orientation(e.alpha, e.beta, e.gamma, e.absolute || false);

        // Update UI
        const orientEl = document.getElementById('orientation');
        if (orientEl) {
            orientEl.textContent = `α:${e.alpha?.toFixed(0) || '-'}° β:${e.beta?.toFixed(0) || '-'}° γ:${e.gamma?.toFixed(0) || '-'}°`;
        }
    }

    function handleMagnetometer(e) {
        on_magnetometer(e.alpha, e.beta, e.gamma);

        // Update UI
        const magEl = document.getElementById('magnetometer');
        if (magEl) {
            magEl.textContent = `heading:${e.alpha?.toFixed(0) || '-'}° β:${e.beta?.toFixed(0) || '-'}° γ:${e.gamma?.toFixed(0) || '-'}°`;
        }
    }

    // Start sensor tracking
    window.startSensorTracking = async function() {
        // Request permissions (iOS requirement)
        if (typeof DeviceMotionEvent !== 'undefined' &&
            typeof DeviceMotionEvent.requestPermission === 'function') {
            try {
                const motionPermission = await DeviceMotionEvent.requestPermission();
                if (motionPermission !== 'granted') {
                    console.warn('Motion permission denied');
                    return false;
                }
            } catch (error) {
                console.error('Error requesting motion permission:', error);
                return false;
            }
        }

        if (typeof DeviceOrientationEvent !== 'undefined' &&
            typeof DeviceOrientationEvent.requestPermission === 'function') {
            try {
                const orientationPermission = await DeviceOrientationEvent.requestPermission();
                if (orientationPermission !== 'granted') {
                    console.warn('Orientation permission denied');
                    return false;
                }
            } catch (error) {
                console.error('Error requesting orientation permission:', error);
                return false;
            }
        }

        // Start GPS tracking
        if (navigator.geolocation) {
            gpsWatchId = navigator.geolocation.watchPosition(
                (position) => {
                    on_gps_update(
                        position.coords.latitude,
                        position.coords.longitude,
                        position.coords.altitude,
                        position.coords.accuracy,
                        position.coords.altitudeAccuracy,
                        position.coords.heading,
                        position.coords.speed
                    );

                    // Update UI
                    const gpsEl = document.getElementById('gps');
                    if (gpsEl) {
                        gpsEl.textContent = `${position.coords.latitude.toFixed(6)}, ${position.coords.longitude.toFixed(6)} ±${position.coords.accuracy.toFixed(1)}m`;
                    }
                },
                (error) => {
                    console.error('GPS error:', error);
                    const gpsEl = document.getElementById('gps');
                    if (gpsEl) {
                        gpsEl.textContent = 'error';
                    }
                },
                { enableHighAccuracy: true, maximumAge: 0, timeout: 5000 }
            );
        }

        // Add motion/orientation listeners
        window.addEventListener('devicemotion', handleMotion);
        window.addEventListener('deviceorientation', handleOrientation);
        window.addEventListener('deviceorientationabsolute', handleMagnetometer);
        sensorListenersActive = true;

        return true;
    };

    // Stop sensor tracking
    window.stopSensorTracking = function() {
        if (gpsWatchId !== null) {
            navigator.geolocation.clearWatch(gpsWatchId);
            gpsWatchId = null;
        }

        if (sensorListenersActive) {
            window.removeEventListener('devicemotion', handleMotion);
            window.removeEventListener('deviceorientation', handleOrientation);
            window.removeEventListener('deviceorientationabsolute', handleMagnetometer);
            sensorListenersActive = false;
        }
    };

    // Checkbox handler
    document.addEventListener('DOMContentLoaded', () => {
        const checkbox = document.getElementById('showSensorsOverlay');
        if (checkbox) {
            checkbox.addEventListener('change', (e) => {
                set_sensor_overlay_enabled(e.target.checked);
            });
        }
    });

    async function run() {
        try {
            await init();
            await start();
        } catch (e) {
            console.error('Failed to initialize:', e);
            document.getElementById('status').textContent = 'Error: ' + e;
        }
    }

    run();
</script>
```

**Step 3: Test in browser**

Manual test:
1. Build: `wasm-pack build --target web --out-dir pkg`
2. Open index.html in browser
3. Check that checkbox appears
4. Check console for any errors
Expected: No errors, checkbox visible

**Step 4: Commit**

```bash
git add index.html
git commit -m "feat: add JavaScript sensor bridge and UI controls

Add sensor tracking functions, event listeners, and checkbox for
overlay toggle. Wire up GPS, magnetometer, orientation, and motion
events to WASM callbacks.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 9: Integrate Sensors into AppState

**Files:**
- Modify: `src/app.rs`

**Step 1: Add sensor_manager to AppState**

In `src/app.rs`, add to the AppState struct:

```rust
sensor_manager: Option<crate::sensors::SensorManager>,
start_time: f64,
```

**Step 2: Initialize sensor_manager in new()**

In the `new()` method, initialize:

```rust
sensor_manager: None,
start_time: 0.0,
```

**Step 3: Start sensor tracking in start_tracking()**

In `start_tracking()` method, after resetting state:

```rust
// Initialize sensor manager
self.start_time = js_sys::Date::now();
self.sensor_manager = Some(crate::sensors::SensorManager::new(self.start_time));

// Update global sensor manager
if let Ok(mut global_mgr) = crate::lib::SENSOR_MANAGER.lock() {
    *global_mgr = self.sensor_manager.clone();
}

// Start sensor tracking
let window = web_sys::window().ok_or("No window")?;
let start_sensors = js_sys::Reflect::get(&window, &"startSensorTracking".into())?;
if start_sensors.is_function() {
    let start_fn: js_sys::Function = start_sensors.dyn_into()?;
    let promise: js_sys::Promise = start_fn.call0(&window)?.dyn_into()?;
    wasm_bindgen_futures::JsFuture::from(promise).await?;
}
```

**Step 4: Render overlay in render loop**

In the render method or render_frame callback, add sensor overlay rendering when enabled:

```rust
// Render sensor overlay if enabled
if let Some(ref mgr) = self.sensor_manager {
    if mgr.is_overlay_enabled() {
        let timestamp = js_sys::Date::new_0().to_iso_string().as_string().unwrap();
        self.canvas_renderer.render_sensor_overlay(
            &timestamp,
            mgr.get_current_gps(),
            mgr.get_current_magnetometer(),
            mgr.get_current_orientation(),
            mgr.get_current_acceleration(),
        )?;
    }
}
```

**Step 5: Save motion data in stop_tracking()**

In `stop_tracking()` method, after creating the blob:

```rust
// Get motion data from sensor manager
let motion_data = if let Some(ref mgr) = self.sensor_manager {
    mgr.get_motion_data().clone()
} else {
    Vec::new()
};

// Save with motion data
self.db.save_recording(&recording_id, &video_blob, &metadata, &motion_data).await?;

// Stop sensors
let window = web_sys::window().ok_or("No window")?;
let stop_sensors = js_sys::Reflect::get(&window, &"stopSensorTracking".into())?;
if stop_sensors.is_function() {
    let stop_fn: js_sys::Function = stop_sensors.dyn_into()?;
    stop_fn.call0(&window)?;
}

// Clear sensor manager
if let Some(ref mut mgr) = self.sensor_manager {
    mgr.clear();
}
```

**Step 6: Verify it compiles**

Run: `cargo check`
Expected: SUCCESS

**Step 7: Commit**

```bash
git add src/app.rs
git commit -m "feat: integrate sensor tracking into AppState

Initialize SensorManager on recording start, render overlay when
enabled, save motion data with recordings, and cleanup on stop.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 10: Build and Test

**Files:**
- None (testing phase)

**Step 1: Build WASM**

Run: `wasm-pack build --target web --out-dir pkg`
Expected: SUCCESS with no errors

**Step 2: Test basic functionality**

Manual test:
1. Open index.html in browser (use local server)
2. Start camera recording
3. Verify sensors display in metrics panel
4. Check "Show Sensors in Video" checkbox
5. Stop recording
6. Download video and verify overlay is visible
7. Download motion data and verify JSON contains sensor readings

**Step 3: Test on mobile (iOS)**

Manual test on iOS device:
1. Grant motion/orientation permissions
2. Start recording
3. Move device around
4. Verify sensor data updates
5. Stop and download

**Step 4: Test backward compatibility**

Manual test:
1. Load page with old recordings (without motion data)
2. Verify they still load and play
Expected: No errors

**Step 5: Final commit**

If any fixes needed during testing, commit them:

```bash
git add .
git commit -m "test: verify sensor tracking functionality

Tested sensor tracking on desktop and mobile. All features working:
- GPS tracking
- Motion/orientation sensors
- Canvas overlay rendering
- Motion data export
- Backward compatibility with old recordings

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Testing Checklist

After completing all tasks, verify:

- [ ] Sensor permissions requested on iOS (button click context)
- [ ] GPS acquires position during recording
- [ ] Motion data collected at ~30-60 Hz
- [ ] Checkbox toggles overlay rendering
- [ ] Overlay visible in recorded video when enabled
- [ ] Overlay NOT in video when checkbox unchecked
- [ ] Motion data saved to IndexedDB
- [ ] Download Motion Data button works
- [ ] JSON export contains all sensor readings
- [ ] Works in all modes (camera, screen, combined)
- [ ] Old recordings load without errors
- [ ] Metrics display updates in real-time
- [ ] GPS shows "acquiring..." until first fix
- [ ] Magnetometer shows "not available" on unsupported devices

---

## Notes

- Total tasks: 10
- Estimated time: 2-4 hours
- Critical path: Types → Sensors → Storage → Canvas → Integration
- Test on real mobile device for sensor accuracy
- GPS may take 30+ seconds for first fix
- Motion data can be 1-5 MB for 10 minute recording
- Use Chrome DevTools Application tab to inspect IndexedDB
