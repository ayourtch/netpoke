# 061 - Add Wipe Survey Session Button to Admin View

## Summary

The admin survey data browser needs a button to wipe (permanently delete) a given survey session, including all associated data from the database tables and uploaded files.

## Location

- **API**: `server/src/analyst_api.rs` - New DELETE endpoint
- **Routes**: `server/src/main.rs` - Route registration
- **UI**: `server/static/admin/surveys.html` - Wipe button in session details

## Current Behavior

The admin survey data browser allows viewing sessions, recordings, metrics, and downloading files, but provides no way to delete a session or its associated data.

## Expected Behavior

- A "Wipe Session" button appears in the expanded session details view
- Clicking it shows a confirmation dialog to prevent accidental deletion
- On confirmation, all associated data is permanently deleted:
  - `survey_metrics` rows for the session
  - `recordings` rows for the session
  - `survey_sessions` row
  - Associated files on disk (video, sensor, pcap, keylog)
- The UI refreshes to reflect the deletion

## Impact

Administrators cannot clean up unwanted or test survey sessions, leading to unnecessary data accumulation.

## Root Cause Analysis

Feature not yet implemented - the admin UI was built with read-only browsing capabilities.

## Suggested Implementation

1. Add `DELETE /admin/api/sessions/{session_id}` endpoint in `analyst_api.rs`
   - Reuse existing access control pattern
   - Query file paths before deletion
   - Hard-delete from all three tables
   - Delete files from disk
   - Return deletion summary
2. Register route in `main.rs`
3. Add wipe button + confirmation dialog in `surveys.html`
4. Add CSS styling for the delete button

## Resolution

Implemented as described in the suggested implementation:

### Changes Made

- **`server/src/analyst_api.rs`**: Added `wipe_session` handler with `WipeSessionResult` response struct. The handler queries file paths before deletion, hard-deletes from all three tables (metrics → recordings → sessions), deletes files from disk with proper error handling, and returns a summary.
- **`server/src/main.rs`**: Registered DELETE method on existing `/admin/api/sessions/{session_id}` route.
- **`server/static/admin/surveys.html`**: Added "🗑 Wipe Session" button (`.btn-danger` styled), `wipeSession()` JavaScript function with confirmation dialog, DOM card removal on success, and status messages showing deletion results.

### Verification

- `cargo check` passes (no new compilation errors)
- Button appears in all session detail views (with or without PCAP/keylog/metrics)
- Confirmation dialog prevents accidental deletion
