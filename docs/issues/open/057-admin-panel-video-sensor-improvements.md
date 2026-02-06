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
