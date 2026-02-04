# Issue 008: Missing Sensor Tracking JavaScript Code

## Summary
The nettest.html is missing the JavaScript code required to:
1. Request sensor permissions (required for iOS)
2. Start GPS tracking via Geolocation API
3. Listen for device orientation/motion events
4. Call the WASM sensor callback functions

Without this JavaScript, even if the WASM callbacks were exported (see Issue 003), they would never be called.

## Location
- File: `server/static/nettest.html`
- Reference: Standalone camera has this in its HTML file (not in repository, but design document references it)

## Current Behavior
The nettest.html does not include:
- `requestSensorPermissions()` function
- `startSensorTracking()` function
- `stopSensorTracking()` function
- Device orientation event listeners
- Device motion event listeners
- Geolocation watch setup

## Expected Behavior
The following JavaScript functions should exist and be callable from WASM:

```javascript
// Sensor Permission and Tracking Functions
async function requestSensorPermissions() {
    // iOS requires explicit permission request
    if (typeof DeviceOrientationEvent !== 'undefined' &&
        typeof DeviceOrientationEvent.requestPermission === 'function') {
        const response = await DeviceOrientationEvent.requestPermission();
        if (response !== 'granted') {
            return false;
        }
    }
    if (typeof DeviceMotionEvent !== 'undefined' &&
        typeof DeviceMotionEvent.requestPermission === 'function') {
        const response = await DeviceMotionEvent.requestPermission();
        if (response !== 'granted') {
            return false;
        }
    }
    return true;
}

let watchId = null;
let orientationListener = null;
let motionListener = null;

function startSensorTracking() {
    // Start GPS
    if (navigator.geolocation) {
        watchId = navigator.geolocation.watchPosition(
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
            },
            (error) => console.warn('GPS error:', error),
            { enableHighAccuracy: true, maximumAge: 1000 }
        );
    }

    // Start orientation tracking
    orientationListener = (event) => {
        on_orientation(event.alpha, event.beta, event.gamma, event.absolute);
    };
    window.addEventListener('deviceorientation', orientationListener);

    // Start motion tracking
    motionListener = (event) => {
        const accel = event.acceleration || { x: 0, y: 0, z: 0 };
        const accelG = event.accelerationIncludingGravity || { x: 0, y: 0, z: 0 };
        const rot = event.rotationRate || { alpha: 0, beta: 0, gamma: 0 };
        on_motion(
            accel.x || 0, accel.y || 0, accel.z || 0,
            accelG.x || 0, accelG.y || 0, accelG.z || 0,
            rot.alpha || 0, rot.beta || 0, rot.gamma || 0
        );
    };
    window.addEventListener('devicemotion', motionListener);
}

function stopSensorTracking() {
    if (watchId !== null) {
        navigator.geolocation.clearWatch(watchId);
        watchId = null;
    }
    if (orientationListener) {
        window.removeEventListener('deviceorientation', orientationListener);
        orientationListener = null;
    }
    if (motionListener) {
        window.removeEventListener('devicemotion', motionListener);
        motionListener = null;
    }
}
```

## Impact
- **Priority: High**
- No sensor data will be collected even if other issues are fixed
- GPS tracking will not work
- Device orientation/motion events will not be captured
- This is a blocking issue for sensor functionality

## Suggested Implementation
Add the JavaScript code above to `server/static/nettest.html` in the `<script>` section, after WASM is initialized but before recording can start.

The WASM module must export `on_gps_update`, `on_orientation`, and `on_motion` functions (see Issue 003).

Call `startSensorTracking()` after requesting permissions when recording starts, and call `stopSensorTracking()` when recording stops.

## Resolution
Fixed in commit 9ab2ea2 (2026-02-04).

**Changes made:**
1. Added sensor tracking JavaScript functions to `server/static/nettest.html`:
   - `requestSensorPermissions()` - Requests iOS DeviceOrientation and DeviceMotion permissions
   - `startSensorTracking()` - Starts GPS tracking via geolocation.watchPosition, adds deviceorientation and devicemotion event listeners
   - `stopSensorTracking()` - Stops all sensor tracking and removes event listeners
2. Exposed WASM callbacks (`on_gps_update`, `on_orientation`, `on_motion`) in the module import
3. Made functions globally accessible via `window` object

The sensor tracking functions are now in place and will call the existing WASM callbacks when sensors report data. GPS, compass, and motion sensors are all supported with proper iOS permission handling.

---
*Created: 2026-02-04*
*Resolved: 2026-02-04*
