# Issue 021: Missing on_magnetometer Export in nettest.html

## Summary
The `on_magnetometer` function is exported from the WASM module but not imported or used in the integrated HTML file (nettest.html). This means absolute compass heading data is not being collected from iOS devices with magnetometer support.

## Location
- **WASM Export**: `client/src/lib.rs` (lines 2005-2018)
- **HTML Integration**: `server/static/nettest.html` (missing from imports around line 85)
- **Working Reference**: `tmp/camera-standalone-for-cross-check/index.html` (lines 317, 340, 395)

## Current Behavior
In nettest.html:
```javascript
const { default: init, start_measurement, start_measurement_with_count, 
        analyze_path, analyze_path_with_count, analyze_network, 
        analyze_network_with_count, stop_testing, is_testing_active, 
        init_recorder, recorder_render_frame, on_gps_update, 
        on_orientation, on_motion } = module;
```

The `on_magnetometer` function is NOT in the import list, and there's no `deviceorientationabsolute` event listener set up.

## Expected Behavior
Should match the standalone implementation:
1. Import `on_magnetometer` from WASM module
2. Add event listener for `deviceorientationabsolute` event
3. Call `on_magnetometer(alpha, beta, gamma, absolute)` from both:
   - The `deviceorientationabsolute` event handler
   - The `deviceorientation` handler when compass data is available (iOS Safari)

## Impact
**Priority**: Medium

Without this integration:
- Absolute compass heading is not captured on devices with magnetometer support
- Camera direction calculations will be less accurate
- iOS devices that provide compass data won't have it recorded

## Suggested Implementation

### Step 1: Add to WASM imports in nettest.html
```javascript
const { default: init, start_measurement, start_measurement_with_count, 
        analyze_path, analyze_path_with_count, analyze_network, 
        analyze_network_with_count, stop_testing, is_testing_active, 
        init_recorder, recorder_render_frame, on_gps_update, 
        on_orientation, on_motion, on_magnetometer } = module;
```

### Step 2: Add magnetometer event listener in requestSensorPermissions
After line where orientation listener is added, add:
```javascript
// Magnetometer (absolute orientation)
window.addEventListener('deviceorientationabsolute', (event) => {
    if (on_magnetometer && event.alpha !== null) {
        on_magnetometer(
            event.alpha || 0,
            event.beta || 0,
            event.gamma || 0,
            true  // absolute = true for deviceorientationabsolute
        );
    }
});
```

### Step 3: Update orientation handler to pass compass data
In the orientation event handler, after calling `on_orientation`, add:
```javascript
// iOS Safari provides compass in alpha even without absolute flag
if (event.alpha !== null && on_magnetometer) {
    on_magnetometer(
        event.alpha,
        event.beta || 0,
        event.gamma || 0,
        event.absolute || false
    );
}
```

### Step 4: Update stopSensorTracking
Add cleanup for magnetometer listener when tracking stops.

## Reference
See `tmp/camera-standalone-for-cross-check/index.html` lines 317, 340, and 395 for working implementation pattern.

## Resolution

**Status**: Resolved

**Changes Made**:

1. **Added on_magnetometer to WASM imports in nettest.html** (line 2593):
   - Added `on_magnetometer` to the destructured imports from the WASM module
   
2. **Added magnetometer listener variable** (lines 2617-2621):
   - Added `let magnetometerListener = null;`
   - Added `let sensorListenersActive = false;` flag to prevent duplicate listeners

3. **Integrated magnetometer listeners in requestSensorPermissions** (lines 2624-2710):
   - Added `deviceorientationabsolute` event listener registration immediately after permission grant (iOS requirement)
   - Updated `deviceorientation` handler to call `on_magnetometer` with compass data when available
   - Listeners are now added in the same synchronous task as permission grant (Issue 024 fix)

4. **Updated stopSensorTracking** (lines 2735-2752):
   - Added cleanup for `magnetometerListener`
   - Reset `sensorListenersActive` flag

**Files Modified**:
- `server/static/nettest.html`

**Verification**:
- WASM module compiled successfully
- Verified `on_magnetometer` is exported: `export function on_magnetometer(alpha, beta, gamma, absolute)`
- All 4 parameters (alpha, beta, gamma, absolute) are correctly passed

**Notes**:
- This fix also addresses Issue 024 by moving event listener registration to `requestSensorPermissions()`
- iOS Safari compatibility maintained by registering listeners immediately after permission grant

---
*Created: 2026-02-04*
*Resolved: 2026-02-04*
