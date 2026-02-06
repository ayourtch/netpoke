# Issue 057: Admin Panel Needs Video Viewing and Sensor Data Download

## Summary
The admin panel shows recording metadata (size, status, device info) but does not provide the ability to view video recordings in-place or download sensor data files.

## Location
- `server/static/admin/surveys.html` - Recording display section
- `server/src/analyst_api.rs` - Session and recording API endpoints

## Current Behavior
Recordings are listed with metadata only. There are no video player controls, no video playback capability, and no download buttons for sensor data files.

## Expected Behavior
- Video recordings should be viewable in-place using an HTML5 video player
- Sensor data should be downloadable as JSON files
- Both should be accessible through the admin panel UI

## Impact
- **Priority**: Medium
- Analysts must manually locate files on the server to view recordings
- No convenient way to download sensor data for analysis

## Suggested Implementation
1. Add API endpoints for serving recording files (video and sensor data)
2. Add inline `<video>` player in the recording section of the admin panel
3. Add download button for sensor data
4. Ensure access control checks (verify analyst has access to the session's magic key)

## Created
2026-02-06

## Resolution
1. Added `download_recording_video` and `download_recording_sensor` API endpoints to `analyst_api.rs`
2. Registered new routes at `/admin/api/recordings/{recording_id}/video` and `/admin/api/recordings/{recording_id}/sensor`
3. Added inline HTML5 video player with play/pause toggle in the admin panel
4. Added sensor data download button for completed recordings
5. Added CSS styles for video container, action buttons, and download links
6. All endpoints include access control (verify user has access to recording's magic key)

### Files Modified
- `server/src/analyst_api.rs` - Added recording file download endpoints with access control
- `server/src/main.rs` - Registered new recording routes
- `server/static/admin/surveys.html` - Added video player, sensor download, and improved UI

## Resolved
2026-02-06
