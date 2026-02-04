# Issue 022: on_magnetometer Function Signature Mismatch

## Summary
The `on_magnetometer` function in WASM expects 4 parameters (alpha, beta, gamma, absolute), but the standalone HTML only passes 3 parameters (alpha, beta, gamma), missing the `absolute: bool` parameter.

## Location
- **Rust Function**: `client/src/lib.rs` (lines 2005-2018)
- **JavaScript Caller**: `tmp/camera-standalone-for-cross-check/index.html` (lines 317, 340)

## Current Behavior

**Rust signature (CORRECT - 4 parameters):**
```rust
#[wasm_bindgen]
pub fn on_magnetometer(alpha: f64, beta: f64, gamma: f64, absolute: bool) {
    // ...
}
```

**JavaScript call in standalone (INCORRECT - 3 parameters):**
```javascript
// Line 317 (from deviceorientation handler)
on_magnetometer(e.alpha, e.beta, e.gamma);

// Line 340 (from deviceorientationabsolute handler)
on_magnetometer(e.alpha, e.beta, e.gamma);
```

## Expected Behavior
JavaScript should pass the `absolute` parameter:

```javascript
// From deviceorientationabsolute - absolute is always true
on_magnetometer(e.alpha, e.beta, e.gamma, true);

// From deviceorientation on iOS - check if compass available
const hasCompass = e.alpha !== null && e.alpha !== undefined;
const effectiveAbsolute = e.absolute || hasCompass;
on_magnetometer(e.alpha, e.beta, e.gamma, effectiveAbsolute);
```

## Impact
**Priority**: High

This is a runtime error waiting to happen:
- When nettest.html is updated to include `on_magnetometer` (Issue 021), this mismatch will cause JavaScript errors
- The function will receive `undefined` for the `absolute` parameter
- Rust may interpret this as `false` or cause type conversion errors
- Camera direction calculations depend on knowing if the orientation is absolute

## Root Cause
The standalone HTML (`tmp/camera-standalone-for-cross-check/index.html`) was created before the `absolute` parameter was added to the WASM function signature. The integrated version has the correct signature but isn't being used yet.

## Suggested Implementation

### Fix standalone reference (tmp/camera-standalone-for-cross-check/index.html)

**Line 317 (in handleOrientation):**
```javascript
// iOS Safari provides compass heading in alpha but doesn't set absolute=true
// Always treat alpha as compass heading on iOS if we have a value
const hasCompass = e.alpha !== null && e.alpha !== undefined;
const effectiveAbsolute = e.absolute || hasCompass;

if (hasCompass) {
    on_magnetometer(e.alpha, e.beta, e.gamma, effectiveAbsolute);
}
```

**Line 340 (in handleMagnetometer):**
```javascript
on_magnetometer(e.alpha, e.beta, e.gamma, true);  // absolute is always true
```

### When implementing Issue 021
Make sure nettest.html uses the correct 4-parameter signature from the start.

## Resolution Dependencies
- Must be fixed before or during implementation of Issue 021
- If Issue 021 is implemented without fixing this, runtime errors will occur

## Resolution

**Status**: Incorrectly Modified - Reverted

**Important Note**: The file `tmp/camera-standalone-for-cross-check/index.html` is a **reference implementation** that should NOT be modified. It exists to serve as a cross-reference for the integrated version, potentially containing known issues for comparison purposes.

**Original Action (Incorrect)**:
- Modified `tmp/camera-standalone-for-cross-check/index.html` to add 4th parameter to `on_magnetometer` calls
- This was a mistake - reference files should remain unchanged

**Corrective Action**:
- Reverted changes to `tmp/camera-standalone-for-cross-check/index.html`
- Restored original 3-parameter signature: `on_magnetometer(e.alpha, e.beta, e.gamma)`
- Updated `prompts/fix-issues.md` to explicitly prohibit modifying files in `tmp/camera-standalone-for-cross-check/`

**Actual Fix Applied**:
- Issue 021 was fixed by updating `server/static/nettest.html` with correct 4-parameter signature
- The integrated version now has the correct implementation
- The standalone reference remains unchanged as it should

**Files Modified** (Correctly):
- `server/static/nettest.html` - Added proper `on_magnetometer` integration with 4 parameters
- `prompts/fix-issues.md` - Added rule to never modify reference code

**Files Reverted**:
- `tmp/camera-standalone-for-cross-check/index.html` - Restored to original state

**Lesson Learned**: Reference implementations exist for cross-checking and should never be modified, even if they contain bugs or outdated patterns.

---
*Created: 2026-02-04*
*Resolved: 2026-02-04*
*Corrected: 2026-02-04*
