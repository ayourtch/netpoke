# Issue 055: Sessions Saved Under "unknown" Magic Key

## Summary
All survey sessions are being saved to the database with `magic_key = "unknown"` because the client does not send the magic key in the `StartSurveySession` WebRTC message, and the server falls back to "unknown" when the magic key is not provided.

## Location
- `server/src/data_channels.rs` - Lines 215-224 in `StartSurveySession` handler
- `client/src/webrtc.rs` - Line 718, `magic_key: None` in `send_start_survey_session()`

## Current Behavior
The client sends `magic_key: None` in the `StartSurveySession` control message. The server checks for `start_survey_msg.magic_key.as_deref()` and when it finds `None`, defaults to `"unknown"`. All sessions are stored in the database under the "unknown" magic key.

## Expected Behavior
Sessions should be stored with the correct magic key. The magic key is already encoded in the survey session ID format: `survey_{magic_key}_{timestamp}_{uuid}` (with hyphens replaced by underscores). The server should extract the magic key from the session ID when the client doesn't provide it explicitly.

## Impact
- **Priority**: High
- All sessions appear under "unknown" in the admin survey browser
- Upload file storage is organized under "unknown/" instead of the correct magic key directory
- Analyst access control by magic key is effectively broken

## Suggested Implementation
Add a helper function `extract_magic_key_from_session_id()` that parses the survey session ID format to extract the magic key. Use this as a fallback when the client doesn't provide the magic key in the message.

## Created
2026-02-06
