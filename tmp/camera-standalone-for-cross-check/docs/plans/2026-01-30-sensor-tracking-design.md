# Sensor Tracking for WASM Camera Recorder

**Date:** 2026-01-30
**Status:** Design - Ready for Implementation

## Overview

Add comprehensive sensor tracking to the WASM camera recorder, matching the functionality in camera-tracker.html. This includes GPS, magnetometer, orientation, and acceleration tracking with optional visual overlay rendered directly into the video.

## Goals

1. Track all device sensors during recording (GPS, magnetometer, orientation, acceleration)
2. Store motion data alongside video in IndexedDB
3. Provide optional canvas overlay showing real-time sensor readings baked into video
4. Enable motion data export as JSON file
5. Maintain same UI/UX as camera-tracker.html

## Architecture

### Core Components

1. **Sensor Manager** (`src/sensors.rs`)
   - New Rust module for sensor data management
   - Handles permission requests for motion/orientation sensors
   - Receives sensor events from JavaScript via callbacks
   - Stores motion data array that grows during recording
   - Provides current sensor state to canvas renderer

2. **Motion Data Storage** (extend `src/storage.rs`)
   - Add `motionData: Vec<MotionDataPoint>` to IndexedDB recordings
   - Serialize/deserialize alongside video blob
   - Export motion data as JSON

3. **Canvas Overlay Renderer** (extend `src/canvas_renderer.rs`)
   - New method to render sensor overlay in top-left corner
   - Semi-transparent black background panel
   - Compact text layout matching camera-tracker.html
   - Only renders when checkbox is enabled

4. **UI Updates** (extend `src/ui.rs`)
   - Add "Show Sensors in Video" checkbox in metrics panel
   - Add "Download Motion Data" button to each recording item
   - Update real-time metrics display with sensor data

## Data Structures

### New Types (in `src/types.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionDataPoint {
    pub timestamp_relative: f64,  // milliseconds from recording start
    pub timestamp_utc: String,     // ISO timestamp
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
    pub alpha: Option<f64>,  // heading/rotation around z-axis
    pub beta: Option<f64>,   // rotation around x-axis
    pub gamma: Option<f64>,  // rotation around y-axis
    pub absolute: bool,      // true for magnetometer (absolute north)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccelerationData {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationData {
    pub alpha: f64,  // rotation rate around z-axis
    pub beta: f64,   // rotation rate around x-axis
    pub gamma: f64,  // rotation rate around y-axis
}
```

### Updated Recording Storage

```rust
// In IndexedDB (JavaScript side)
{
    id: "...",
    videoBlob: Blob,
    motionData: MotionDataPoint[],  // NEW FIELD
    metadata: RecordingMetadata,
    timestamp: number
}
```

## Recording Flow

1. **Start Recording**
   - Request motion/orientation permissions (iOS requirement)
   - Start GPS watch with high accuracy
   - Set up JavaScript event listeners for sensors
   - Initialize empty motion_data vector

2. **During Recording**
   - JavaScript fires sensor callbacks into WASM
   - WASM appends MotionDataPoint to vector
   - If overlay enabled: render sensor data on canvas each frame
   - Update UI metrics display

3. **Stop Recording**
   - Stop GPS watch
   - Remove sensor event listeners
   - Save video blob + motion data array to IndexedDB
   - Clear motion data vector

4. **Export Motion Data**
   - Retrieve motion data from IndexedDB
   - Serialize as JSON with metadata
   - Download as `motion_${id}.json`

## UI Design

### Sensor Overlay Checkbox

**Location:** In metrics div, below PiP controls
**HTML:** `<input type="checkbox" id="showSensorsOverlay"> Show Sensors in Video`
**Default:** Unchecked (overlay disabled)
**Behavior:** Toggle overlay rendering on canvas

### Sensor Overlay Visual (when enabled)

**Position:** Top-left corner of canvas
**Padding:** 15px from edges
**Background:** `rgba(0, 0, 0, 0.8)`
**Text:** White, 12px monospace font

**Layout:**
```
┌─────────────────────────────────────┐
│ 2026-01-30 14:32:15.234 UTC        │
│ GPS: 37.123456, -122.654321 ±5.2m  │
│ Magnetometer: heading:245° β:12° γ:3°│
│ Orientation: α:245° β:12° γ:3°     │
│ Accel: x:0.05 y:-0.12 z:9.81       │
└─────────────────────────────────────┘
```

**Update Rate:** Every frame (30 fps)
**Fallbacks:**
- GPS: "acquiring..." if no fix yet
- Magnetometer: "not available" if unsupported
- Values: "-" for null/undefined

### Recording List Updates

Add new button to each recording item:
```html
<button onclick="downloadMotionData('${rec.id}')">Download Motion Data</button>
```

Position: Between "Download Video" and "Delete"

## JavaScript/WASM Bridge

### JavaScript Sensor Setup (in `index.html`)

```javascript
export async function setupSensorTracking(wasmCallbacks) {
    // Request permissions (iOS requirement)
    if (typeof DeviceMotionEvent.requestPermission === 'function') {
        await DeviceMotionEvent.requestPermission();
    }
    if (typeof DeviceOrientationEvent.requestPermission === 'function') {
        await DeviceOrientationEvent.requestPermission();
    }

    // GPS watch
    const gpsWatchId = navigator.geolocation.watchPosition(
        (pos) => wasmCallbacks.on_gps_update(
            pos.coords.latitude,
            pos.coords.longitude,
            pos.coords.altitude,
            pos.coords.accuracy,
            pos.coords.altitudeAccuracy,
            pos.coords.heading,
            pos.coords.speed,
            new Date(pos.timestamp).toISOString()
        ),
        (err) => console.error('GPS error:', err),
        { enableHighAccuracy: true, maximumAge: 0, timeout: 5000 }
    );

    // Motion/orientation listeners
    window.addEventListener('devicemotion', (e) => {
        wasmCallbacks.on_motion(
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
    });

    window.addEventListener('deviceorientation', (e) => {
        wasmCallbacks.on_orientation(e.alpha, e.beta, e.gamma, e.absolute || false);
    });

    window.addEventListener('deviceorientationabsolute', (e) => {
        wasmCallbacks.on_magnetometer(e.alpha, e.beta, e.gamma);
    });

    return gpsWatchId;
}

export function stopSensorTracking(gpsWatchId) {
    if (gpsWatchId !== null) {
        navigator.geolocation.clearWatch(gpsWatchId);
    }
    // Event listeners removed in WASM
}
```

### WASM Exports (in `src/lib.rs`)

```rust
#[wasm_bindgen]
pub fn request_sensor_permissions() -> js_sys::Promise;

#[wasm_bindgen]
pub fn on_gps_update(
    latitude: f64,
    longitude: f64,
    altitude: Option<f64>,
    accuracy: f64,
    altitude_accuracy: Option<f64>,
    heading: Option<f64>,
    speed: Option<f64>,
    timestamp: String,
);

#[wasm_bindgen]
pub fn on_motion(
    accel_x: f64, accel_y: f64, accel_z: f64,
    accel_g_x: f64, accel_g_y: f64, accel_g_z: f64,
    rot_alpha: f64, rot_beta: f64, rot_gamma: f64,
);

#[wasm_bindgen]
pub fn on_orientation(
    alpha: Option<f64>,
    beta: Option<f64>,
    gamma: Option<f64>,
    absolute: bool,
);

#[wasm_bindgen]
pub fn on_magnetometer(
    alpha: Option<f64>,
    beta: Option<f64>,
    gamma: Option<f64>,
);

#[wasm_bindgen]
pub fn download_motion_data(id: String) -> js_sys::Promise;

#[wasm_bindgen]
pub fn set_sensor_overlay_enabled(enabled: bool);
```

## Implementation Order

1. **Types & Data Structures** (`src/types.rs`)
   - Add MotionDataPoint, GpsData, OrientationData, AccelerationData, RotationData
   - Update Recording type if needed

2. **Sensor Manager** (`src/sensors.rs`)
   - Create SensorManager struct
   - Implement data collection methods
   - Implement current state getters for UI/overlay

3. **Storage Updates** (`src/storage.rs`)
   - Update IndexedDB schema to include motionData
   - Add serialize/deserialize for motion data
   - Add download_motion_data export

4. **Canvas Renderer** (`src/canvas_renderer.rs`)
   - Add render_sensor_overlay method
   - Format sensor data as text overlay
   - Draw on canvas when enabled

5. **UI Controller** (`src/ui.rs`)
   - Add checkbox for sensor overlay toggle
   - Add "Download Motion Data" button template
   - Wire up checkbox to WASM callback

6. **JavaScript Bridge** (`index.html`)
   - Add sensor setup functions
   - Wire up event listeners to WASM callbacks
   - Handle permissions on iOS

7. **App State** (`src/app.rs`)
   - Integrate SensorManager into AppState
   - Call sensor methods during recording lifecycle
   - Pass sensor data to canvas renderer

8. **IndexedDB Updates** (`js/indexed_db.js`)
   - Update saveRecording to accept motionData
   - Update getAllRecordings to return motionData
   - Add getMotionData if needed

## Testing Checklist

- [ ] Request sensor permissions on iOS
- [ ] GPS acquires position during recording
- [ ] Motion data collected during recording
- [ ] Sensor overlay renders on canvas when checkbox enabled
- [ ] Sensor overlay NOT rendered when checkbox disabled
- [ ] Sensor data saved to IndexedDB with recording
- [ ] Motion data downloads as JSON
- [ ] Sensor readings visible in video playback (when overlay was enabled)
- [ ] Works on all three modes (camera, screen, combined)
- [ ] Old recordings without motion data still load (backward compatibility)

## Notes

- Motion data can grow large for long recordings (several MB for 10+ minute recordings)
- IndexedDB should handle this fine, but consider chunking for very long recordings (>1 hour)
- GPS may take 5-30 seconds to acquire initial fix
- Magnetometer may not be available on all devices
- iOS requires permissions in user gesture context (button click)
