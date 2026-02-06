# Issue 060: Save Survey Session ID with Video Recording Metadata

## Summary

Video recordings on the client side do not save the testing session ID associated with them. When users upload recordings later, the system cannot correctly attribute them to the original survey session.

## Location

- `client/src/recorder/types.rs`: `RecordingMetadata` struct (missing `survey_session_id` field)
- `client/src/recorder/state.rs`: `stop_recording()` method (does not capture session ID)
- `client/src/lib.rs`: `get_survey_session_id()` (was private, inaccessible from recorder)
- `server/static/nettest.html`: Upload flow and recording list display

## Current Behavior

- Survey session ID is generated in `analyze_network()` and stored in `SURVEY_SESSION_ID` thread-local
- Session ID is exposed to JavaScript via `window.currentSurveySessionId`
- Video recordings are saved to IndexedDB with metadata (duration, frames, source type, etc.) but **without** the session ID
- Upload reads `window.currentSurveySessionId` at upload time — if the session has changed since recording, it would be attributed incorrectly or fail

## Expected Behavior

- Video recordings should store the survey session ID in their metadata at recording time
- When uploading, the stored session ID from the recording metadata should be used (with fallback to current active session)
- The recordings list should display which session a recording belongs to

## Impact

Users who record video during a survey session and upload later (or after starting a new session) cannot correctly attribute recordings to the original session.

## Root Cause Analysis

The `RecordingMetadata` struct did not include a `survey_session_id` field, and the `get_survey_session_id()` function was private to `lib.rs`, making it inaccessible from the recorder module.

## Suggested Implementation

1. Add `survey_session_id: Option<String>` to `RecordingMetadata` struct
2. Make `get_survey_session_id()` `pub(crate)` for cross-module access
3. Capture the session ID in `stop_recording()` when creating metadata
4. Update upload flow to prefer stored session ID from metadata
5. Display session ID in recordings list UI

## Resolution

All five changes implemented:

### Files Modified

- **`client/src/recorder/types.rs`**: Added `survey_session_id: Option<String>` field to `RecordingMetadata` with `#[serde(default)]` for backward compatibility
- **`client/src/lib.rs`**: Changed `get_survey_session_id()` from `fn` to `pub(crate) fn`
- **`client/src/recorder/state.rs`**: Added session ID capture in `stop_recording()`, converting empty string to `None`
- **`server/static/nettest.html`**:
  - Added `.session-id-label` CSS style for displaying session IDs
  - Added session ID display row in recordings list
  - Updated upload to prefer stored `recording.metadata.survey_session_id` over `window.currentSurveySessionId`

### Verification

- Rust code compiles without new warnings
- Backward compatible: `#[serde(default)]` ensures existing recordings without session ID deserialize correctly
- Upload gracefully falls back to current active session if no stored session ID exists
