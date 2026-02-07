# 062 - Empty Sensor JSON Download

## Summary

When downloading sensor JSON data for a recording that was uploaded to the server, the downloaded file is empty (contains only `[]`). This is because the upload code references the wrong field name when serializing sensor data from IndexedDB.

## Location

- **File**: `server/static/nettest.html`
- **Line 3064**: Display of sensor data size in recordings list
- **Line 3326**: Serialization of sensor data for upload

## Current Behavior

The upload code reads `recording.sensorData` which does not exist on the IndexedDB recording object. Since `undefined || []` evaluates to `[]`, the sensor data blob becomes `"[]"` (2 bytes). This empty array is uploaded and stored on the server, so when the analyst downloads the sensor file they get an empty JSON array.

## Expected Behavior

The upload code should read `recording.motionData` which is the actual field name used when saving recordings to IndexedDB. This would serialize the full motion/sensor data collected during the recording.

## Impact

All sensor data uploads result in empty files on the server. Users collecting sensor data during recordings cannot retrieve it later for analysis.

## Root Cause Analysis

Field name mismatch between storage and retrieval:

1. **IndexedDB storage** (`server/static/lib/recorder/indexed_db.js`, line 29): saves the field as `motionData`
2. **camera-tracker.html** (line 169): saves recordings with `motionData` field
3. **nettest.html** (line 3326): reads `recording.sensorData` — wrong field name
4. **nettest.html** (line 3064): reads `rec.sensorData` for display — wrong field name

The field was named `motionData` throughout the recording pipeline but the upload code in `nettest.html` was written using `sensorData` instead.

## Suggested Implementation

1. Change `recording.sensorData` to `recording.motionData` on line 3326 (upload serialization)
2. Change `rec.sensorData` to `rec.motionData` on line 3064 (display size calculation)

## Resolution

Fixed both references in `server/static/nettest.html`:
- Line 3064: `rec.sensorData` → `rec.motionData` (display size in recordings list)
- Line 3326: `recording.sensorData` → `recording.motionData` (upload serialization)

Verified that the field name `motionData` is consistent with:
- `server/static/lib/recorder/indexed_db.js` (storage)
- `server/static/camera-tracker.html` (recording)
- `client/src/lib.rs` (WASM export)
