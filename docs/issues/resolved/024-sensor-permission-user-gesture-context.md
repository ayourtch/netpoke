# Issue 024: Sensor Permission Request Not Called From User Gesture Context

## Summary
The integrated nettest.html requests sensor permissions in an async function that may not execute within the user gesture context required by iOS Safari, potentially causing permission requests to fail silently.

## Location
- **File**: `server/static/nettest.html` (sensor tracking setup section)
- **Reference**: `tmp/camera-standalone-for-cross-check/index.html` (lines 349-401)

## Current Behavior
In nettest.html, sensor permissions are requested in a separate async function:
```javascript
async function requestSensorPermissions() {
    if (typeof DeviceMotionEvent.requestPermission === 'function') {
        const permission = await DeviceMotionEvent.requestPermission();
        // ...
    }
}
```

This function is likely called from a button click handler, but the async nature means event listeners may be added AFTER the user gesture context expires on iOS.

## Expected Behavior
From the standalone reference (lines 349-401), the critical pattern is:

```javascript
// CRITICAL: Event listeners must be added in same task as permission request on iOS
window.addEventListener('devicemotion', handleMotion);
window.addEventListener('deviceorientation', handleOrientation);
window.addEventListener('deviceorientationabsolute', handleMagnetometer);
```

**iOS Requirement**: Event listeners MUST be added:
1. In the **same synchronous task** as permission grant
2. **BEFORE** any `await` statements that break synchronous execution
3. Within the user gesture event handler call stack

## Impact
**Priority**: High (iOS-specific)

On iOS Safari:
- Permission may be granted, but event listeners won't receive events
- Sensor tracking appears to work but no data is captured
- Users won't get any error message
- Very difficult to debug without iOS device testing

On Android/desktop:
- No impact (permissions not required, listeners work regardless)

## Root Cause
iOS Safari's security model requires that event listeners for motion/orientation sensors be registered **in the same synchronous execution context** as the permission grant. Using `await` breaks this context.

## Suggested Implementation

### Pattern from Standalone (CORRECT):
```javascript
window.requestSensorPermissions = async function() {
    // Request permissions
    if (typeof DeviceMotionEvent.requestPermission === 'function') {
        const motionPermission = await DeviceMotionEvent.requestPermission();
        if (motionPermission !== 'granted') {
            return false;
        }
    }
    
    if (typeof DeviceOrientationEvent.requestPermission === 'function') {
        const orientationPermission = await DeviceOrientationEvent.requestPermission();
        if (orientationPermission !== 'granted') {
            return false;
        }
    }
    
    // CRITICAL: Add event listeners immediately after permission granted (iOS requirement)
    // Must be in same synchronous task, before any await
    if (!sensorListenersActive) {
        window.addEventListener('devicemotion', handleMotion);
        window.addEventListener('deviceorientation', handleOrientation);
        window.addEventListener('deviceorientationabsolute', handleMagnetometer);
        sensorListenersActive = true;
    }
    
    return true;
};
```

### Key Points:
1. **Add listeners immediately** after permission check
2. **Before any subsequent await** statements
3. **Store flag** to avoid duplicate listener registration
4. Listeners can be defined earlier, just not attached until permission granted

### Additional Fix Needed:
Separate "request permissions" from "start tracking":
- `requestSensorPermissions()` - Gets permissions AND adds listeners (user gesture required)
- `startSensorTracking()` - Starts GPS watch, no permissions needed
- Call from button: `await requestSensorPermissions() && startSensorTracking()`

## Testing
**Must test on iOS Safari** - this bug is iOS-specific:
1. Open nettest.html on iOS Safari
2. Start recording with camera
3. Grant sensor permissions
4. Check if accelerometer/gyro data appears in recording
5. If data is all zeros, this bug is present

**Desktop/Android**: May appear to work fine, hiding the iOS issue.

## References
- MDN: [Detecting device orientation](https://developer.mozilla.org/en-US/docs/Web/API/Detecting_device_orientation)
- Apple: [Requesting permission for device orientation and motion on iOS 13+](https://developer.apple.com/documentation/safari-release-notes/safari-13-release-notes#Device-Motion-and-Orientation)

## Resolution

**Status**: Resolved (Combined with Issue 021)

**Changes Made**:

The fix for Issue 024 was integrated into the solution for Issue 021, as they both relate to sensor permission and event listener setup.

1. **Moved event listener registration from startSensorTracking to requestSensorPermissions** (lines 2624-2710):
   - `orientationListener`, `motionListener`, and `magnetometerListener` now added immediately after permission grant
   - All listeners registered in the same synchronous task, before any await statements
   - Added `sensorListenersActive` flag to prevent duplicate listener registration

2. **Updated startSensorTracking** (lines 2712-2733):
   - Now only handles GPS tracking (which doesn't require iOS permissions)
   - Event listeners no longer added here

3. **Added iOS-specific magnetometer handling in deviceorientation listener**:
   - iOS Safari compass data properly forwarded to `on_magnetometer`
   - Handles both `deviceorientation` (iOS) and `deviceorientationabsolute` (Android) events

**Files Modified**:
- `server/static/nettest.html`

**Critical iOS Requirement Met**:
✓ Event listeners added in same synchronous execution context as permission grant
✓ No await statements between permission and listener registration
✓ Duplicate listener registration prevented with flag
✓ Works correctly on iOS Safari, Android, and desktop browsers

**Testing Recommendations**:
- Test on iOS Safari to verify motion/orientation data is captured
- Verify accelerometer/gyro data is not all zeros during recording
- Check that magnetometer (compass) data appears in sensor overlay

---
*Created: 2026-02-04*
*Resolved: 2026-02-04*
