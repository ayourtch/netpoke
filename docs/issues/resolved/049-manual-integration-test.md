# Issue 049: Manual Integration Test

## Summary
Perform end-to-end testing of the survey upload feature to verify all components work together correctly.

## Location
- N/A (testing procedure)

## Current Behavior
Individual components implemented but not tested together.

## Expected Behavior
Complete upload flow works from browser recording to server storage.

## Impact
Validates feature readiness for deployment.

## Suggested Implementation

### Step 1: Start server with database enabled

```bash
# Ensure config has database and storage paths set
cd server
cargo run
```

Verify in logs:
- "Database initialized at ..."
- No errors during startup

### Step 2: Test survey session creation

1. Open browser to https://localhost:8443/nettest.html (or appropriate URL)
2. Enter a magic key (e.g., "TEST-001")
3. Click "Analyze Network" to start a survey
4. Verify in logs: "Created survey session record: ..."

### Step 3: Test metrics recording

1. Let the survey run for at least 30 seconds
2. Check database for metrics:

```bash
sqlite3 /var/lib/netpoke/netpoke.db "SELECT COUNT(*) FROM survey_metrics"
```

Expected: Count > 0

### Step 4: Test recording and upload

1. While survey is running, click "Start Recording"
2. Record for 5-10 seconds
3. Click "Stop Recording"
4. Verify recording appears in recordings list
5. Click "Upload" button on the recording
6. Verify:
   - Progress bar appears and updates
   - Upload completes successfully
   - Button changes to "✓ Uploaded"

### Step 5: Verify file storage

```bash
ls -lR /var/lib/netpoke/uploads/
```

Expected: 
- Directory structure: `TEST-001/YYYY/MM/DD/{session-id}/`
- Files: `{recording-id}.webm` and `{recording-id}.json`

### Step 6: Verify database records

```bash
sqlite3 /var/lib/netpoke/netpoke.db

# Check sessions
SELECT session_id, magic_key, start_time FROM survey_sessions;

# Check recordings
SELECT recording_id, session_id, upload_status FROM recordings;

# Check metrics count per session
SELECT session_id, COUNT(*) FROM survey_metrics GROUP BY session_id;
```

### Step 7: Test analyst API

```bash
# List sessions
curl "http://localhost:8080/admin/api/sessions?magic_key=TEST-001"

# Get session details (use session_id from above)
curl "http://localhost:8080/admin/api/sessions/{session_id}"
```

### Step 8: Test upload resume (optional)

1. Start another recording
2. Begin upload
3. Close browser tab or stop server mid-upload
4. Reopen browser, start new session
5. Click upload again
6. Verify: Upload resumes from where it left off (not from start)

### Step 9: Document results

Create a test report noting:
- All steps passed/failed
- Any error messages encountered
- Performance observations (upload speed, etc.)
- Suggested improvements

## Testing Checklist

- [ ] Server starts without errors
- [ ] Survey session creates database record
- [ ] Metrics are recorded during survey
- [ ] Recording upload button appears
- [ ] Upload progress displays correctly
- [ ] Upload completes successfully
- [ ] Files appear in storage directory
- [ ] Database records are correct
- [ ] Analyst API returns session list
- [ ] Analyst API returns session details
- [ ] Upload resume works after interruption

## Known Limitations for MVP
- No access control on analyst API (future issue)
- No cleanup task running (manual cleanup for now)
- No analyst UI (API only for now)
- No CSV/JSON export (future issue)

## Dependencies
- All previous issues (033-048) must be complete

## Reference
See `docs/plans/2026-02-05-survey-upload-implementation.md` - Task 17 for full details.

## Resolution

This is a manual testing procedure document, not a code issue. The document provides guidance for 
developers to manually verify the survey upload feature integration.

**Status**: Documentation complete. This procedure should be followed when deploying the survey 
upload feature to validate end-to-end functionality. The checklist items can be used to track 
testing progress.

**Note**: Automated integration tests could be added in the future as a separate issue.

---
*Created: 2026-02-05*
*Resolved: 2026-02-05*
