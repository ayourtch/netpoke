# Issue 001: IndexedDB Database Name Not Updated

## Summary
The design document specifies using "NetpokeRecordingsDB" for the IndexedDB database, but the implementation still uses "CameraTrackingDB" from the standalone camera app.

## Location
- File: `server/static/lib/recorder/indexed_db.js`
- Function: `openDb()`
- Line: 4

## Current Behavior
```javascript
const request = indexedDB.open('CameraTrackingDB', 2);
```
The database is named 'CameraTrackingDB', which was the name used in the standalone camera application.

## Expected Behavior
Per the design document `docs/plans/2026-02-04-camera-recording-integration-design.md`:
```javascript
const request = indexedDB.open('NetpokeRecordingsDB', 1);
```
The database should be named 'NetpokeRecordingsDB' to reflect the netpoke product branding.

## Impact
- **Priority: Low**
- Branding inconsistency - users examining browser storage will see "CameraTrackingDB" instead of "NetpokeRecordingsDB"
- No functional impact, but violates the design specification
- If both the standalone camera app and netpoke are used on the same browser, recordings are shared (may be intentional or unintentional)

## Suggested Implementation
1. In `server/static/lib/recorder/indexed_db.js`, change line 4:
   ```javascript
   const request = indexedDB.open('NetpokeRecordingsDB', 1);
   ```
2. Note: Existing users may have recordings in 'CameraTrackingDB'. Consider:
   - Adding a migration to copy existing recordings (complex)
   - OR accepting data loss for users who have existing recordings (simple)
   - OR keeping the database name as-is if sharing with camera app is desired

## Resolution
**Fixed in commit b10cf2c**

Changed the database name in `server/static/lib/recorder/indexed_db.js` from `'CameraTrackingDB'` to `'NetpokeRecordingsDB'` and updated the version from 2 to 1 to match the design specification.

Files modified:
- `server/static/lib/recorder/indexed_db.js`

---
*Created: 2026-02-04*
*Resolved: 2026-02-04*
