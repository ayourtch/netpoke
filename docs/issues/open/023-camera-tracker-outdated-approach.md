# Issue 023: camera-tracker.html Uses Outdated Approach Without WASM Integration

## Summary
The `server/static/camera-tracker.html` file implements camera and sensor tracking using plain JavaScript without WASM integration, making it incompatible with the recorder subsystem that expects WASM-based sensor management.

## Location
- **File**: `server/static/camera-tracker.html` (545 lines)
- **Comparison**: Much simpler than integrated `nettest.html` (2875 lines) or standalone reference (525 lines)

## Current Behavior
camera-tracker.html:
- Uses plain JavaScript for GPS, magnetometer, orientation, and acceleration tracking
- Stores sensor data in JavaScript variables (`currentGPS`, `lastMagnetometer`, `lastOrientation`, `lastAcceleration`)
- Uses MediaRecorder API directly without canvas rendering or sensor overlays
- No WASM imports or integration with the recorder subsystem
- No sensor data export to JSON
- No integration with the SensorManager in Rust

## Expected Behavior
camera-tracker.html should either:
1. **Option A**: Be removed or renamed to indicate it's a legacy/deprecated approach
2. **Option B**: Be updated to use the WASM recorder subsystem like nettest.html
3. **Option C**: Be documented as a simplified testing/demo page (not production)

## Impact
**Priority**: Low (Documentation/Cleanup Issue)

This doesn't break functionality since nettest.html is the main entry point, but:
- Creates confusion about which approach to use
- May mislead developers who look at this file as an example
- Duplicates effort if someone tries to maintain both approaches
- Users who access this page won't get the full feature set

## Context
There are three HTML files with camera/sensor functionality:

| File | Purpose | WASM Integration | Status |
|------|---------|------------------|--------|
| `tmp/camera-standalone-for-cross-check/index.html` | Reference standalone implementation | Yes (camera_wasm) | Working reference |
| `server/static/nettest.html` | Integrated network testing + camera | Yes (netpoke_client) | Production |
| `server/static/camera-tracker.html` | Plain JS camera tracking | No | Unclear purpose |

## Suggested Implementation

### Option A: Mark as Deprecated (Recommended)
Rename to `camera-tracker-legacy.html` and add warning banner:
```html
<div style="background: #ff6b6b; color: white; padding: 10px; text-align: center;">
    ⚠️ DEPRECATED: This is a legacy demo. Use <a href="/nettest.html">nettest.html</a> for full features.
</div>
```

### Option B: Update to Use WASM
- Import netpoke_client WASM module
- Replace JavaScript sensor tracking with WASM callbacks
- Add canvas rendering for sensor overlays
- Add IndexedDB storage for recordings

Estimated effort: 4-6 hours (significant refactoring)

### Option C: Document as Demo
Add comment at top of file:
```html
<!--
    SIMPLE CAMERA DEMO (No WASM)
    This is a simplified demonstration of camera + sensor tracking.
    For production use with full features, see nettest.html
-->
```

## Recommendation
**Option A** - Mark as deprecated and point users to nettest.html. This avoids confusion and maintenance burden while preserving the file for reference if needed.

---
*Created: 2026-02-04*
