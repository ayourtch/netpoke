# Issue 032: iOS Compass Missing - effectiveAbsolute Not Implemented

## Summary
On iOS Safari, the compass visualization is not displayed because the integrated code in `nettest.html` doesn't handle iOS Safari's quirk where compass heading is provided in the `alpha` value of deviceorientation events, but the `absolute` flag is not set to `true`.

## Location
- File: `server/static/nettest.html`
- Function: `orientationListener` in the sensor permission handling section (around line 2680-2695)

## Current Behavior
The integrated code passes `event.absolute || false` directly to the WASM `on_orientation()` function:
```javascript
on_orientation(
    event.alpha,
    event.beta,
    event.gamma,
    event.absolute || false  // iOS Safari doesn't set this to true
);
```

Since iOS Safari doesn't set `absolute=true`, the WASM code treats the orientation as relative and doesn't calculate the camera direction. Without a camera direction, the compass indicator is never rendered.

## Expected Behavior
The code should detect when iOS Safari provides a valid compass heading in `alpha` and treat it as absolute orientation, matching the reference implementation:
```javascript
// iOS Safari provides compass heading in alpha but doesn't set absolute=true
// Always treat alpha as compass heading on iOS if we have a value
const hasCompass = event.alpha !== null && event.alpha !== undefined;
const effectiveAbsolute = event.absolute || hasCompass;

on_orientation(event.alpha, event.beta, event.gamma, effectiveAbsolute);
```

## Impact
- **Critical for iOS users**: Compass visualization completely missing on iOS Safari
- Users cannot see which direction the camera is facing during surveys
- Recorded sensor data may lack proper compass orientation data

## Suggested Implementation
1. In `server/static/nettest.html`, update the `orientationListener` to:
   - Check if `event.alpha` has a valid value
   - Compute `effectiveAbsolute` as `event.absolute || hasCompass`
   - Pass `effectiveAbsolute` to `on_orientation()` instead of just `event.absolute`

## Resolution
Fixed by updating `server/static/nettest.html` to implement the iOS Safari workaround:

1. Added `hasCompass` check to detect valid compass heading in alpha
2. Computed `effectiveAbsolute` to treat iOS alpha values as compass data
3. Updated both the `on_orientation()` call and the `on_magnetometer()` call to use the effective absolute flag

Changes made:
- Modified `orientationListener` function in `server/static/nettest.html`

---
*Created: 2026-02-05*
*Resolved: 2026-02-05*
