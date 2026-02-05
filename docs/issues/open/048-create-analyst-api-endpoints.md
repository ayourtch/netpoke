# Issue 048: Create Basic Analyst API Endpoints

## Summary
Create API endpoints for analysts to list and browse survey sessions by magic key.

## Location
- File: `server/src/analyst_api.rs` (new file)
- File: `server/src/main.rs` (add routes)

## Current Behavior
No analyst-facing API exists for browsing survey data.

## Expected Behavior
API endpoints that:
1. List survey sessions filtered by magic key
2. Return session metadata including recording counts and file availability
3. Provide foundation for analyst UI

## Impact
Enables analysts to programmatically access survey data for analysis.

## Suggested Implementation

### Step 1: Create analyst API module

Create `server/src/analyst_api.rs`:

```rust
use crate::database::DbConnection;
use axum::{
    extract::{State, Path, Query},
    http::StatusCode,
    Json,
};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
pub struct AnalystState {
    pub db: DbConnection,
}

#[derive(Debug, Deserialize)]
pub struct ListSessionsQuery {
    pub magic_key: String,
}

#[derive(Debug, Serialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub magic_key: String,
    pub start_time: i64,
    pub last_update_time: i64,
    pub has_pcap: bool,
    pub has_keylog: bool,
    pub recording_count: i32,
}

/// List sessions by magic key
pub async fn list_sessions(
    State(state): State<Arc<AnalystState>>,
    Query(query): Query<ListSessionsQuery>,
) -> Result<Json<Vec<SessionSummary>>, StatusCode> {
    let db = state.db.lock().await;

    let mut stmt = db.prepare(
        "SELECT s.session_id, s.magic_key, s.start_time, s.last_update_time,
                s.pcap_path, s.keylog_path,
                COUNT(r.recording_id) as recording_count
         FROM survey_sessions s
         LEFT JOIN recordings r ON s.session_id = r.session_id AND r.deleted = 0
         WHERE s.magic_key = ? AND s.deleted = 0
         GROUP BY s.session_id
         ORDER BY s.start_time DESC"
    ).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let sessions = stmt.query_map(params![&query.magic_key], |row| {
        Ok(SessionSummary {
            session_id: row.get(0)?,
            magic_key: row.get(1)?,
            start_time: row.get(2)?,
            last_update_time: row.get(3)?,
            has_pcap: row.get::<_, Option<String>>(4)?.is_some(),
            has_keylog: row.get::<_, Option<String>>(5)?.is_some(),
            recording_count: row.get(6)?,
        })
    }).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let result: Result<Vec<_>, _> = sessions.collect();
    Ok(Json(result.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?))
}

#[derive(Debug, Serialize)]
pub struct SessionDetails {
    pub session_id: String,
    pub magic_key: String,
    pub user_login: Option<String>,
    pub start_time: i64,
    pub last_update_time: i64,
    pub pcap_path: Option<String>,
    pub keylog_path: Option<String>,
    pub recordings: Vec<RecordingSummary>,
    pub metric_count: i32,
}

#[derive(Debug, Serialize)]
pub struct RecordingSummary {
    pub recording_id: String,
    pub video_size_bytes: i64,
    pub sensor_size_bytes: i64,
    pub upload_status: String,
    pub user_notes: Option<String>,
    pub device_info_json: Option<String>,
    pub completed_at: Option<i64>,
}

/// Get session details including recordings
pub async fn get_session(
    State(state): State<Arc<AnalystState>>,
    Path(session_id): Path<String>,
) -> Result<Json<SessionDetails>, StatusCode> {
    let db = state.db.lock().await;

    // Get session info
    let session: (String, Option<String>, i64, i64, Option<String>, Option<String>) = db
        .query_row(
            "SELECT magic_key, user_login, start_time, last_update_time, pcap_path, keylog_path
             FROM survey_sessions WHERE session_id = ? AND deleted = 0",
            params![&session_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
        )
        .map_err(|_| StatusCode::NOT_FOUND)?;

    // Get recordings
    let mut recordings_stmt = db.prepare(
        "SELECT recording_id, video_size_bytes, sensor_size_bytes, upload_status,
                user_notes, device_info_json, completed_at
         FROM recordings WHERE session_id = ? AND deleted = 0"
    ).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let recordings: Vec<RecordingSummary> = recordings_stmt
        .query_map(params![&session_id], |row| {
            Ok(RecordingSummary {
                recording_id: row.get(0)?,
                video_size_bytes: row.get(1)?,
                sensor_size_bytes: row.get(2)?,
                upload_status: row.get(3)?,
                user_notes: row.get(4)?,
                device_info_json: row.get(5)?,
                completed_at: row.get(6)?,
            })
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get metric count
    let metric_count: i32 = db
        .query_row(
            "SELECT COUNT(*) FROM survey_metrics WHERE session_id = ? AND deleted = 0",
            params![&session_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    Ok(Json(SessionDetails {
        session_id: session_id.clone(),
        magic_key: session.0,
        user_login: session.1,
        start_time: session.2,
        last_update_time: session.3,
        pcap_path: session.4,
        keylog_path: session.5,
        recordings,
        metric_count,
    }))
}
```

### Step 2: Add routes in main.rs

```rust
mod analyst_api;

// In main function, after database initialization:
let analyst_state = Arc::new(crate::analyst_api::AnalystState {
    db: db.clone(),
});

let analyst_routes = axum::Router::new()
    .route("/admin/api/sessions", axum::routing::get(crate::analyst_api::list_sessions))
    .route("/admin/api/sessions/:session_id", axum::routing::get(crate::analyst_api::get_session))
    .with_state(analyst_state);

let app = app.merge(analyst_routes);
```

## API Endpoints

### GET /admin/api/sessions?magic_key=SURVEY-001
Returns list of sessions for the specified magic key.

### GET /admin/api/sessions/{session_id}
Returns detailed session info including recordings and metric count.

## Future Enhancements (separate issues)
- Access control middleware (check user permissions)
- Metrics export endpoints (CSV/JSON)
- File download endpoints (PCAP, keylog, recordings)
- Analyst UI page

## Testing
- Build succeeds: `cargo build`
- API returns empty list when no sessions exist
- API returns sessions after running surveys
- Session details include correct recording counts

## Dependencies
- Issue 043: Initialize services in main.rs
- Issue 035: Implement database module

## Reference
See `docs/plans/2026-02-05-survey-upload-implementation.md` - Task 16 for full details.
See `docs/plans/2026-02-05-survey-upload-feature-design.md` - Analyst API Endpoints section.

---
*Created: 2026-02-05*
