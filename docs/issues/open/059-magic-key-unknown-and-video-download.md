# Issue 059: Magic Key "unknown" and Missing Video Download Link

## Summary
Two issues:
1. All survey sessions are saved with `magic_key = "unknown"` because the WASM client doesn't pass the magic key in the `StartSurveySession` WebRTC message
2. The admin panel has no direct download link for video files, making it impossible to verify server-side video data integrity

## Location
- `server/src/auth_handlers.rs`: `AuthStatusResponse` struct and auth status endpoints
- `server/static/nettest.html`: Auth status check and magic key storage
- `client/src/lib.rs`: Survey session initialization
- `client/src/webrtc.rs`: `send_start_survey_session()` method
- `server/static/admin/surveys.html`: Recording actions UI

## Current Behavior
1. **Magic Key**: The client generates a plain UUID as the survey_session_id and sends `magic_key: None` in the `StartSurveySession` message. The server's `extract_magic_key_from_session_id()` function expects the format `survey_{key}_{ts}_{uuid}` which doesn't match a plain UUID, so it falls back to "unknown".
2. **Video Download**: The admin panel only has a "Play Video" button (inline HTML5 player). There is no way to download the raw video file to verify data integrity on the server.

## Expected Behavior
1. Survey sessions should be saved with the correct magic key from the authentication session
2. A "Download Video" link should be available alongside the "Play Video" button

## Impact
1. All sessions appear under "unknown" magic key, breaking the organization model and access control
2. Users cannot verify whether video data on the server is complete/correct

## Root Cause Analysis
1. **Magic Key**: The `AuthStatusResponse` doesn't include the magic key, so the JS page has no way to pass it to the WASM client. The WASM client's `send_start_survey_session()` hardcodes `magic_key: None`.
2. **Video Download**: The UI only renders a Play button, not a download link.

## Suggested Implementation

### Magic Key Fix
1. Add `magic_key` field to `AuthStatusResponse` in `auth_handlers.rs`
2. Extract the magic key from the cookie session ID in auth status endpoints
3. Store `window.currentMagicKey` in `nettest.html` when auth status is checked
4. Read `window.currentMagicKey` in the WASM client and pass it in `StartSurveySession`
5. Update `send_start_survey_session()` to accept optional magic_key parameter

### Video Download Fix
1. Add a "Download Video" anchor link next to the "Play Video" button in `surveys.html`
2. Use the existing `/admin/api/recordings/{id}/video` endpoint with the `download` HTML attribute

## Resolution

### Changes Made:
1. `server/src/auth_handlers.rs`:
   - Added `magic_key: Option<String>` field to `AuthStatusResponse`
   - Added `extract_magic_key_from_session()` helper function
   - Both `auth_status` and `auth_status_with_cache` now return the magic key for magic key sessions
2. `server/static/nettest.html`:
   - Stores `window.currentMagicKey` from auth status response
3. `client/src/lib.rs`:
   - Added `get_magic_key_from_js()` to read `window.currentMagicKey`
   - Passes magic key to `send_start_survey_session()` calls
4. `client/src/webrtc.rs`:
   - `send_start_survey_session()` now accepts `magic_key: Option<String>`
5. `server/static/admin/surveys.html`:
   - Added "⬇ Download Video" link next to "▶ Play Video" button
