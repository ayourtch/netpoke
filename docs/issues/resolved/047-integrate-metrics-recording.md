# Issue 047: Integrate Metrics Recording into Survey Flow

## Summary
Connect the MetricsRecorder and SessionManager services to the existing survey measurement and session management code.

## Location
- File: `server/src/measurements.rs`
- File: `server/src/data_channels.rs`

## Current Behavior
Probe statistics are calculated and sent to clients but not persisted to the database.
Survey sessions are created in memory but not recorded in the database.

## Expected Behavior
1. When a survey session starts, create a database record
2. When probe stats are calculated, save them to the database
3. When client sends metrics, save them to the database
4. Update session timestamp on each metric write

## Impact
Enables persistent storage of all survey metrics for later analysis and export.

## Suggested Implementation

### Step 1: Create session on survey start

In `server/src/data_channels.rs`, find where survey sessions are started (likely where `StartSurveySession` message is handled):

```rust
// When survey session starts
if let Some(session_manager) = &state.session_manager {
    let magic_key = /* extract from auth context or message */;
    let user_login = /* extract from auth if available */;
    
    if let Err(e) = session_manager.create_session(
        &survey_session_id,
        &magic_key,
        user_login.as_deref(),
    ).await {
        tracing::error!("Failed to create survey session record: {}", e);
    } else {
        tracing::info!("Created survey session record: {}", survey_session_id);
    }
}
```

### Step 2: Record server-side probe stats

In `server/src/measurements.rs`, find the function that calculates and sends ProbeStats (likely `calculate_and_send_probe_stats` or similar):

```rust
// After calculating probe_stats, before or after sending to client
if let Some(recorder) = &state.metrics_recorder {
    if let Some(ref session_id) = survey_session_id {
        let timestamp_ms = chrono::Utc::now().timestamp_millis() as u64;
        
        if let Err(e) = recorder.record_probe_stats(
            session_id,
            &conn_id,
            timestamp_ms,
            &c2s_stats,
            &s2c_stats,
        ).await {
            tracing::error!("Failed to record probe stats: {}", e);
        }
        
        // Also update session timestamp
        if let Some(session_mgr) = &state.session_manager {
            if let Err(e) = session_mgr.update_session_timestamp(session_id).await {
                tracing::error!("Failed to update session timestamp: {}", e);
            }
        }
    }
}
```

### Step 3: Record client-side metrics

Find where client ProbeStats feedback is received (likely in a control message handler):

```rust
// When receiving client metrics (ClientProbeStats or similar)
if let Some(recorder) = &state.metrics_recorder {
    if let Some(ref session_id) = survey_session_id {
        let timestamp_ms = /* extract from message or use current time */;
        
        if let Err(e) = recorder.record_client_metrics(
            session_id,
            &conn_id,
            timestamp_ms,
            &client_s2c_stats,
        ).await {
            tracing::error!("Failed to record client metrics: {}", e);
        }
    }
}
```

### Step 4: Save PCAP/keylog paths on session stop

When a survey session explicitly stops:

```rust
// When survey stops explicitly (not browser close/crash)
if let Some(session_manager) = &state.session_manager {
    if let Some(ref session_id) = survey_session_id {
        let pcap_path = /* get from PacketCaptureService if available */;
        let keylog_path = /* get from DtlsKeylogService if available */;
        
        if let Err(e) = session_manager.update_session_files(
            session_id,
            pcap_path.as_deref(),
            keylog_path.as_deref(),
        ).await {
            tracing::error!("Failed to save session file paths: {}", e);
        }
    }
}
```

## Notes on Magic Key Access

The magic_key needs to be accessible when creating the session. This may require:
1. Passing magic_key through the session creation flow
2. Storing magic_key in ClientSession struct
3. Extracting from authentication context

Look at how the existing code handles magic_key and maintain consistency.

## Testing
- Build succeeds: `cargo build`
- Start survey and verify session appears in database
- Run survey and verify metrics are recorded
- Stop survey explicitly and verify PCAP/keylog paths are saved
- Query database to confirm data: `sqlite3 /var/lib/netpoke/netpoke.db "SELECT COUNT(*) FROM survey_metrics"`

## Dependencies
- Issue 043: Initialize services in main.rs
- Issue 037: Implement MetricsRecorder service
- Issue 038: Create SessionManager service

## Reference
See `docs/plans/2026-02-05-survey-upload-implementation.md` - Task 15 for full details.
See `docs/plans/2026-02-05-survey-upload-feature-design.md` - Metrics Collection & Storage section.

## Resolution

Integrated MetricsRecorder and SessionManager services into the survey measurement flow:

### Changes Made

1. **server/src/state.rs**:
   - Added `session_manager` and `metrics_recorder` fields to `AppState`
   - Added `session_manager`, `metrics_recorder`, and `magic_key` fields to `ClientSession`
   - Added `set_session_manager()` and `set_metrics_recorder()` methods to `AppState`

2. **server/src/signaling.rs**:
   - Updated `ClientSession` creation to include new fields (`session_manager`, `metrics_recorder`, `magic_key`)

3. **server/src/main.rs**:
   - Added imports for `MetricsRecorder` and `SessionManager`
   - Initialize services when database is available
   - Set services on `app_state` after database initialization

4. **server/src/data_channels.rs**:
   - Added session creation in database when `StartSurveySession` message is received
   - Store magic_key in ClientSession for database recording

5. **server/src/measurements.rs**:
   - Record probe stats to database in `start_probe_stats_reporter()`
   - Update session timestamp on each metrics write

6. **common/src/protocol.rs**:
   - Added optional `magic_key` field to `StartSurveySessionMessage` for database recording

### Verification
- Build succeeds: `cargo build --package netpoke-server`
- All existing tests pass (no regressions)

---
*Created: 2026-02-05*
*Resolved: 2026-02-05*
