# Issue 034: Create Database Schema Migration

## Summary
Create the SQLite database schema for storing survey sessions, metrics, and recordings.

## Location
- File: `server/migrations/001_survey_upload_schema.sql` (new file)

## Current Behavior
No database schema exists for survey data storage.

## Expected Behavior
A SQL migration file should define tables for:
- `survey_sessions` - stores survey session metadata
- `survey_metrics` - stores per-second metrics from client and server
- `recordings` - stores metadata for uploaded video + sensor recordings

## Impact
This is a foundational schema required for all survey data storage features.

## Suggested Implementation

### Step 1: Create migrations directory

```bash
mkdir -p server/migrations
```

### Step 2: Create schema SQL file

Create `server/migrations/001_survey_upload_schema.sql` with the following tables:

**survey_sessions table:**
```sql
CREATE TABLE IF NOT EXISTS survey_sessions (
  session_id TEXT PRIMARY KEY,
  magic_key TEXT NOT NULL,
  user_login TEXT,
  start_time INTEGER NOT NULL,
  last_update_time INTEGER NOT NULL,
  pcap_path TEXT,
  keylog_path TEXT,
  created_at INTEGER NOT NULL,
  deleted INTEGER DEFAULT 0,
  deleted_at INTEGER,
  deleted_by TEXT
);

CREATE INDEX IF NOT EXISTS idx_session_magic_key ON survey_sessions(magic_key, start_time);
CREATE INDEX IF NOT EXISTS idx_session_deleted ON survey_sessions(deleted);
```

**survey_metrics table:**
```sql
CREATE TABLE IF NOT EXISTS survey_metrics (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT NOT NULL,
  timestamp_ms INTEGER NOT NULL,
  source TEXT NOT NULL,
  conn_id TEXT,
  direction TEXT,
  delay_p50_ms REAL,
  delay_p99_ms REAL,
  delay_min_ms REAL,
  delay_max_ms REAL,
  jitter_p50_ms REAL,
  jitter_p99_ms REAL,
  jitter_min_ms REAL,
  jitter_max_ms REAL,
  rtt_p50_ms REAL,
  rtt_p99_ms REAL,
  rtt_min_ms REAL,
  rtt_max_ms REAL,
  loss_rate REAL,
  reorder_rate REAL,
  probe_count INTEGER,
  baseline_delay_ms REAL,
  created_at INTEGER NOT NULL,
  deleted INTEGER DEFAULT 0,
  deleted_at INTEGER,
  deleted_by TEXT,
  FOREIGN KEY(session_id) REFERENCES survey_sessions(session_id)
);

CREATE INDEX IF NOT EXISTS idx_metrics_session ON survey_metrics(session_id, timestamp_ms);
CREATE INDEX IF NOT EXISTS idx_metrics_deleted ON survey_metrics(deleted);
```

**recordings table:**
```sql
CREATE TABLE IF NOT EXISTS recordings (
  recording_id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  video_path TEXT NOT NULL,
  sensor_path TEXT NOT NULL,
  video_size_bytes INTEGER NOT NULL,
  sensor_size_bytes INTEGER NOT NULL,
  video_uploaded_bytes INTEGER DEFAULT 0,
  sensor_uploaded_bytes INTEGER DEFAULT 0,
  upload_status TEXT DEFAULT 'pending',
  device_info_json TEXT,
  user_notes TEXT,
  created_at INTEGER NOT NULL,
  completed_at INTEGER,
  deleted INTEGER DEFAULT 0,
  deleted_at INTEGER,
  deleted_by TEXT,
  FOREIGN KEY(session_id) REFERENCES survey_sessions(session_id)
);

CREATE INDEX IF NOT EXISTS idx_recordings_session ON recordings(session_id);
CREATE INDEX IF NOT EXISTS idx_recordings_status ON recordings(upload_status);
CREATE INDEX IF NOT EXISTS idx_recordings_deleted ON recordings(deleted);
```

## Testing
- Verify the SQL syntax is valid by loading into a test SQLite database
- Verify all foreign key relationships are correct

## Dependencies
- Issue 033: Add database dependencies (must be completed first)

## Reference
See `docs/plans/2026-02-05-survey-upload-implementation.md` - Task 2 for full details.
See `docs/plans/2026-02-05-survey-upload-feature-design.md` - Database Schema section for schema rationale.

---
*Created: 2026-02-05*
---
*Resolved: 2026-02-05*

## Resolution

Implemented as part of the survey upload feature implementation.
