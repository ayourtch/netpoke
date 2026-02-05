# Survey Upload Feature - Design Document

**Date:** 2026-02-05
**Status:** Approved
**Author:** Claude (Brainstorming Session)

## Overview

Add one-click upload capability for survey recordings (video + sensor data) with server-side storage, access control, and association with existing survey metrics.

## Goals

1. Enable surveyors to upload recordings (video + sensors) with a single click
2. Store uploaded data alongside server-side metrics, PCAP, and DTLS keylog
3. Provide analysts with a web interface to browse, view, and export survey data
4. Implement configurable access control and data retention policies
5. Support resumable uploads for reliability on poor networks

## High-Level Architecture

### System Components

1. **Client Upload UI** (nettest.html)
   - "Upload" button per recording in recordings list
   - Progress bar showing chunk upload status
   - Editable notes field per recording

2. **Upload API** (new Rust endpoints)
   - POST `/api/upload/prepare` - Initiate/resume upload
   - POST `/api/upload/chunk` - Upload individual chunks
   - POST `/api/upload/finalize` - Complete upload

3. **SQLite Database** (new)
   - Survey sessions, metrics timeseries, recordings metadata
   - Soft delete tracking on all tables

4. **File Storage**
   - Directory structure: `uploads/{magic_key}/{YYYY}/{MM}/{DD}/{session_id}/{recording_uuid}.{ext}`
   - WebM videos + sensor JSON files

5. **Analyst UI** (new page: `/admin/surveys`)
   - Browse surveys by magic key
   - View metrics, download recordings/PCAP/keylog
   - Export statistics to CSV/JSON

### Data Flow

```
Survey Start → Create DB Record → Metrics Collection (1/sec)
     ↓                                    ↓
  Session ID                     Update last_update_time
     ↓                                    ↓
User Records Video + Sensors         Save to DB
     ↓
User Clicks Upload
     ↓
Chunked Transfer (SHA-256 verified)
     ↓
Server Storage

Survey Stop (Explicit) → Capture PCAP/Keylog
Survey Stop (Implicit/Crash) → No PCAP/Keylog
```

## Database Schema

### survey_sessions

Stores survey session metadata and timestamps.

```sql
CREATE TABLE survey_sessions (
  session_id TEXT PRIMARY KEY,           -- UUID
  magic_key TEXT NOT NULL,
  user_login TEXT,                       -- nullable, if logged in
  start_time INTEGER NOT NULL,           -- Unix timestamp ms
  last_update_time INTEGER NOT NULL,     -- Updated with each metric
  pcap_path TEXT,                        -- nullable, only if explicitly stopped
  keylog_path TEXT,                      -- nullable, only if explicitly stopped
  created_at INTEGER NOT NULL,
  deleted INTEGER DEFAULT 0,
  deleted_at INTEGER,
  deleted_by TEXT
);

CREATE INDEX idx_session_magic_key ON survey_sessions(magic_key, start_time);
CREATE INDEX idx_session_deleted ON survey_sessions(deleted);
```

### survey_metrics

Stores per-second metrics from both client and server.

```sql
CREATE TABLE survey_metrics (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT NOT NULL,
  timestamp_ms INTEGER NOT NULL,         -- When metric was captured
  source TEXT NOT NULL,                  -- 'client' or 'server'
  conn_id TEXT,                          -- Connection/path UUID (for ECMP)
  direction TEXT,                        -- 'c2s' or 's2c'

  -- ProbeStats metrics (from DirectionStats)
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

CREATE INDEX idx_metrics_session ON survey_metrics(session_id, timestamp_ms);
CREATE INDEX idx_metrics_deleted ON survey_metrics(deleted);
```

### recordings

Stores metadata for uploaded video + sensor recordings.

```sql
CREATE TABLE recordings (
  recording_id TEXT PRIMARY KEY,         -- UUID from client
  session_id TEXT NOT NULL,
  video_path TEXT NOT NULL,
  sensor_path TEXT NOT NULL,
  video_size_bytes INTEGER NOT NULL,
  sensor_size_bytes INTEGER NOT NULL,
  video_uploaded_bytes INTEGER DEFAULT 0,
  sensor_uploaded_bytes INTEGER DEFAULT 0,
  upload_status TEXT DEFAULT 'pending',  -- pending/uploading/complete/failed
  device_info_json TEXT,                 -- JSON blob
  user_notes TEXT,
  created_at INTEGER NOT NULL,
  completed_at INTEGER,
  deleted INTEGER DEFAULT 0,
  deleted_at INTEGER,
  deleted_by TEXT,

  FOREIGN KEY(session_id) REFERENCES survey_sessions(session_id)
);

CREATE INDEX idx_recordings_session ON recordings(session_id);
CREATE INDEX idx_recordings_status ON recordings(upload_status);
CREATE INDEX idx_recordings_deleted ON recordings(deleted);
```

## Upload Protocol

### Chunked Upload Mechanism

- **Chunk size:** 1MB (1,048,576 bytes)
- **Checksum:** SHA-256 per chunk
- **Resume:** Byte offset tracking via chunk checksums
- **Upload mode:** Sequential (one chunk at a time)

### Upload Flow

#### Step 1: Prepare Upload

Client initiates upload and gets server state.

**Request:**
```http
POST /api/upload/prepare
Content-Type: application/json

{
  "session_id": "a7f3b2c1-...",
  "recording_id": "d4e5f6a7-...",
  "video_size_bytes": 52428800,
  "sensor_size_bytes": 1048576,
  "device_info": {
    "browser": "Safari 17.2",
    "os": "iOS 17.3.1",
    "screen_width": 1170,
    "screen_height": 2532
  },
  "user_notes": "Basement survey, poor signal"
}
```

**Response:**
```json
{
  "recording_id": "d4e5f6a7-...",
  "video_chunks": [
    { "chunk_index": 0, "checksum": "sha256:abc123..." },
    { "chunk_index": 1, "checksum": "sha256:def456..." },
    null,  // Chunk 2 not yet uploaded
    null   // Chunk 3 not yet uploaded
  ],
  "sensor_chunks": [
    { "chunk_index": 0, "checksum": "sha256:xyz789..." }
  ],
  "video_uploaded_bytes": 2097152,
  "sensor_uploaded_bytes": 1048576
}
```

**Server Logic:**
1. Verify session exists in database
2. Create or update recording metadata
3. Check if files exist on disk
4. If files exist, calculate SHA-256 for each 1MB chunk
5. Return chunk checksums for resume capability

**Client Logic:**
1. Calculate SHA-256 for each chunk locally
2. Compare with server's checksums
3. Only upload chunks where checksum differs or is missing
4. Enables efficient resume after interruption

#### Step 2: Upload Chunks

Client uploads individual chunks sequentially.

**Request:**
```http
POST /api/upload/chunk
Content-Type: application/octet-stream
X-Recording-Id: d4e5f6a7-...
X-File-Type: video
X-Chunk-Index: 2
X-Chunk-Checksum: sha256:abc123...

[Binary chunk data - 1MB]
```

**Response:**
```json
{
  "status": "received",
  "chunk_index": 2,
  "bytes_received": 1048576
}
```

**Server Logic:**
1. Validate session and recording exist
2. Calculate SHA-256 of received chunk
3. Verify checksum matches header
4. Write chunk at offset `chunk_index * 1MB` in file
5. Update `{video|sensor}_uploaded_bytes` in database
6. Return success

**Error Handling:**
- Checksum mismatch → Return 400 Bad Request, client retries
- File write failure → Return 500, client retries
- After 3 retries → Mark upload as failed

#### Step 3: Finalize Upload

Client signals completion after all chunks uploaded.

**Request:**
```http
POST /api/upload/finalize
Content-Type: application/json

{
  "recording_id": "d4e5f6a7-...",
  "video_final_checksum": "sha256:hash_of_all_video_chunk_hashes",
  "sensor_final_checksum": "sha256:hash_of_all_sensor_chunk_hashes"
}
```

**Response:**
```json
{
  "status": "complete",
  "video_verified": true,
  "sensor_verified": true
}
```

**Server Logic:**
1. Read all chunks from files
2. Calculate SHA-256 for each chunk
3. Calculate SHA-256 of concatenated chunk hashes
4. Verify matches client's final checksum
5. Update `upload_status = 'complete'`, set `completed_at`

## Metrics Collection & Storage

### Integration Points

Existing metrics code in `server/src/measurements.rs` already calculates statistics. We need to persist these to SQLite.

**New Service:**

```rust
pub struct MetricsRecorder {
    db: Arc<Mutex<Connection>>,
}

impl MetricsRecorder {
    pub async fn record_probe_stats(
        &self,
        session_id: &str,
        conn_id: &str,
        timestamp_ms: u64,
        c2s_stats: &DirectionStats,
        s2c_stats: &DirectionStats,
    ) -> Result<()> {
        // Insert two rows into survey_metrics:
        // - One for c2s direction (source = "server")
        // - One for s2c direction (source = "server")
    }

    pub async fn record_client_metrics(
        &self,
        session_id: &str,
        conn_id: &str,
        timestamp_ms: u64,
        s2c_stats: &DirectionStats,
    ) -> Result<()> {
        // Insert row with source = "client"
    }
}
```

**Modification Points:**

1. **`server/src/measurements.rs`**
   - In `calculate_and_send_probe_stats()`: Call `recorder.record_probe_stats()`
   - Captures server-side calculated metrics

2. **Control message handler**
   - When client sends ProbeStats feedback: Call `recorder.record_client_metrics()`
   - Captures client-side calculated metrics

3. **Session management**
   - On `StartSurveySession` message: Insert into `survey_sessions` table
   - On each metric write: Update `last_update_time`
   - On explicit stop: Save PCAP/keylog paths to database

### Session Lifecycle

**No explicit "complete" status needed.**

- Survey starts → Create DB record
- Metrics arrive → Update `last_update_time`, insert metrics
- Survey stops explicitly (Stop Testing) → Capture PCAP/DTLS keylog, save paths
- Survey stops implicitly (browser blur/crash) → Metrics stop, no PCAP/DTLS

Sessions are implicitly done when metrics stop arriving. No background task needed.

## File Storage & Cleanup

### Directory Structure

```
{base_path}/uploads/
  SURVEY-001/
    2026/
      02/
        05/
          session-uuid-1/
            recording-uuid-a.webm
            recording-uuid-a.json
            recording-uuid-b.webm
            recording-uuid-b.json
            session.pcap           # Only if explicitly stopped
            session.keylog         # Only if explicitly stopped
```

**Path Construction:**
- Base path: Configurable in `server_config.toml` (default: `/var/lib/netpoke/uploads`)
- Magic key: From survey session
- Date: Year/Month/Day from session start time
- Session ID: Survey session UUID
- Recording files: Named as `{recording_uuid}.{ext}`

**Auto-creation:**
```rust
async fn ensure_upload_path(
    magic_key: &str,
    session_id: &str,
    session_start_time: DateTime<Utc>,
    base_path: &Path,
) -> Result<PathBuf> {
    let path = base_path
        .join(magic_key)
        .join(format!("{}", session_start_time.year()))
        .join(format!("{:02}", session_start_time.month()))
        .join(format!("{:02}", session_start_time.day()))
        .join(session_id);

    tokio::fs::create_dir_all(&path).await?;
    Ok(path)
}
```

### Cleanup Policies

**1. Failed Uploads**

Delete recordings where upload failed and not retried.

- **Condition:** `upload_status = 'failed'` AND `created_at < now() - 2 days`
- **Action:**
  - Delete files from disk
  - Set `deleted = 1, deleted_at = now(), deleted_by = 'system'`
- **Frequency:** Daily background task

**2. Retention by Magic Key**

Configurable retention period per magic key.

**Configuration (`server_config.toml`):**
```toml
[retention]
default_days = 14

[retention.magic_keys]
"SURVEY-001" = 30
"SURVEY-002" = 7
```

**Cleanup Logic:**
- **Condition:** `start_time < now() - retention_days` for that magic key
- **Action:**
  - Soft delete DB records: `deleted = 1, deleted_at = now(), deleted_by = 'system'`
  - Hard delete files from disk (videos, sensors, PCAP, keylog)
- **Frequency:** Daily background task

**3. Background Cleanup Service**

```rust
async fn cleanup_task(db: Arc<Connection>, config: Arc<Config>) {
    loop {
        // 1. Clean up failed uploads (> 2 days old)
        cleanup_failed_uploads(&db).await;

        // 2. Apply retention policies per magic key
        cleanup_old_surveys(&db, &config).await;

        // Sleep for 24 hours
        tokio::time::sleep(Duration::from_secs(86400)).await;
    }
}
```

## Access Control & Configuration

### Configuration Structure

**File:** `server_config.toml`

```toml
# Storage configuration
[storage]
base_path = "/var/lib/netpoke/uploads"

# Analyst access control - maps usernames to accessible magic keys
[analyst_access]
"analyst1@example.com" = ["SURVEY-001", "SURVEY-002"]
"analyst2@example.com" = ["SURVEY-001"]
"admin@example.com" = ["*"]  # Wildcard for all magic keys

# Retention policies per magic key
[retention]
default_days = 14

[retention.magic_keys]
"SURVEY-001" = 30
"SURVEY-002" = 7
```

### Authentication & Authorization

**Upload Endpoints:**
- **Authentication:** None required
- **Authorization:** Valid `session_id` (UUID) must exist in database
- **Rationale:** Session ID is secret/random, knowing it is sufficient proof

**Analyst Endpoints:**
- **Authentication:** Existing username/password system (future: OAuth)
- **Authorization:** User must have access to magic key in config
- **Enforcement:** Middleware checks on all browse/download/export endpoints

### Authorization Middleware

```rust
async fn check_analyst_access(
    user: &str,
    magic_key: &str,
    config: &Config,
) -> Result<bool> {
    let allowed_keys = config.analyst_access.get(user)
        .ok_or(Error::Unauthorized)?;

    Ok(allowed_keys.contains(&"*".to_string()) ||
       allowed_keys.contains(&magic_key.to_string()))
}
```

### Analyst API Endpoints

```
GET /admin/surveys?magic_key=SURVEY-001
  → Returns list of sessions for that magic key (if user has access)
  → Filters by deleted = 0

GET /admin/surveys/{session_id}
  → Returns session details, metrics, recordings
  → Checks user has access to session's magic key

GET /admin/surveys/{session_id}/metrics
  → Returns timeseries metrics as JSON

GET /admin/surveys/{session_id}/export/metrics.csv
  → Exports metrics as CSV

GET /admin/surveys/{session_id}/export/metrics.json
  → Exports metrics as JSON

GET /admin/surveys/{session_id}/pcap
  → Downloads PCAP file (if exists)

GET /admin/surveys/{session_id}/keylog
  → Downloads DTLS keylog file (if exists)

GET /admin/surveys/{session_id}/recordings/{recording_id}/video
  → Downloads video file

GET /admin/surveys/{session_id}/recordings/{recording_id}/sensors
  → Downloads sensor data file
```

## User Interface Design

### A. Surveyor Upload UI

**Location:** `server/static/nettest.html` - Recordings section

**Modifications:**

1. Add upload button to each recording
2. Add progress bar for upload status
3. Add edit notes button

**HTML Structure:**

```html
<div class="recording-item" data-recording-id="d4e5f6a7-...">
  <div class="recording-item-info">
    <strong>Recording 1</strong>
    <span>2026-02-05 14:23:15</span>
    <span>Video: 45.2 MB, Sensors: 1.2 MB</span>
    <div class="recording-notes">Notes: Basement survey, poor signal</div>
  </div>

  <div class="recording-item-actions">
    <button onclick="editNotes('d4e5f6a7-...')" class="capture-btn blue">
      ✏️ Edit Notes
    </button>
    <button onclick="startUpload('d4e5f6a7-...')"
            id="upload-btn-d4e5f6a7"
            class="capture-btn green">
      📤 Upload
    </button>
    <button onclick="playRecording('d4e5f6a7-...')" class="capture-btn purple">
      ▶️ Play
    </button>
    <button onclick="deleteRecording('d4e5f6a7-...')" class="capture-btn red">
      🗑️ Delete
    </button>
  </div>

  <!-- Progress bar (hidden initially) -->
  <div class="upload-progress" id="progress-d4e5f6a7" style="display:none;">
    <div class="progress-bar">
      <div class="progress-fill" style="width: 0%"></div>
    </div>
    <span class="progress-text">Uploading video: 0% (0/51 chunks)</span>
  </div>
</div>
```

**Upload Button States:**
- **Pending:** "📤 Upload" (green button)
- **Uploading:** Button disabled, progress bar visible
- **Complete:** "✓ Uploaded" (green, disabled)
- **Failed:** "⚠️ Retry Upload" (red button, clickable)

**Progress Display:**
- Discrete progress bar showing percentage
- Text: "Uploading video: 45% (23/51 chunks)"
- Separate progress for sensors after video complete
- Green checkmark when both complete

### B. Analyst UI

**New Page:** `server/static/admin/surveys.html`

**Route:** `/admin/surveys`

**Authentication:** Requires login via existing auth system

**Layout:**

```
┌──────────────────────────────────────────────────┐
│ NetPoke - Survey Data Browser                    │
│ Logged in as: analyst1@example.com    [Logout]   │
└──────────────────────────────────────────────────┘

Magic Key: [Dropdown: SURVEY-001 ▼]  [Refresh]

┌──────────────────────────────────────────────────┐
│ Survey Sessions for SURVEY-001                   │
├──────────────────────────────────────────────────┤
│                                                  │
│ ┌──────────────────────────────────────────────┐ │
│ │ Session: a7f3b2c1-4d5e-6f7a-8b9c-0d1e2f3a4b5c │ │
│ │ Date: 2026-02-05 14:23:15 - 14:45:32         │ │
│ │ Duration: 22 minutes 17 seconds              │ │
│ │ Metrics: 1,337 data points                   │ │
│ │ Recordings: 2 videos                         │ │
│ │ PCAP: ✓ Available (12.3 MB)                  │ │
│ │ Keylog: ✓ Available (1.2 KB)                 │ │
│ │                                              │ │
│ │ [View Metrics Chart] [Download PCAP]         │ │
│ │ [Download Keylog] [Export CSV] [Export JSON] │ │
│ │                                              │ │
│ │ Recordings:                                  │ │
│ │   📹 recording-1.webm (45.2 MB)              │ │
│ │      Notes: Basement survey, poor signal     │ │
│ │      Device: Safari 17.2 on iOS 17.3.1       │ │
│ │      [▶️ Play] [⬇️ Video] [⬇️ Sensors]       │ │
│ │                                              │ │
│ │   📹 recording-2.webm (32.1 MB)              │ │
│ │      Notes: First floor, good signal         │ │
│ │      Device: Safari 17.2 on iOS 17.3.1       │ │
│ │      [▶️ Play] [⬇️ Video] [⬇️ Sensors]       │ │
│ └──────────────────────────────────────────────┘ │
│                                                  │
│ ┌──────────────────────────────────────────────┐ │
│ │ Session: b8c9d0e1-...                        │ │
│ │ Date: 2026-02-04 09:15:22 - 09:28:45         │ │
│ │ ...                                          │ │
│ └──────────────────────────────────────────────┘ │
│                                                  │
└──────────────────────────────────────────────────┘
```

**Features:**

1. **Magic Key Selection**
   - Dropdown populated from user's accessible keys in config
   - Only shows keys user has access to
   - Wildcard "*" shows all magic keys

2. **Session List**
   - Sorted by start_time descending (newest first)
   - Collapsible/expandable sections
   - Shows key metadata at a glance
   - Filters out deleted sessions

3. **Metrics Visualization**
   - "View Metrics Chart" opens modal with Chart.js graph
   - Shows latency, jitter, loss over time
   - Separate lines for IPv4/IPv6, C2S/S2C

4. **Download Actions**
   - Direct links to PCAP/keylog (if available)
   - Export metrics as CSV or JSON
   - Download individual recordings (video/sensors)

5. **Video Playback**
   - Inline player in modal
   - Shows device info and user notes
   - Synced sensor data visualization (future enhancement)

## Implementation Details

### Dependencies

Add to `server/Cargo.toml`:

```toml
[dependencies]
rusqlite = { version = "0.30", features = ["bundled"] }
tokio-rusqlite = "0.5"
sha2 = "0.10"
hex = "0.4"
```

### Server Startup Sequence

1. Load configuration from `server_config.toml`
2. Initialize SQLite database
   - Create tables if not exist
   - Run migrations if schema changes
3. Start background cleanup task
4. Initialize MetricsRecorder service
5. Start web server with new routes

### Database Initialization

```rust
async fn init_database(db_path: &Path) -> Result<Connection> {
    let conn = Connection::open(db_path)?;

    // Create tables
    conn.execute_batch(include_str!("../migrations/001_initial_schema.sql"))?;

    // Enable foreign keys
    conn.execute("PRAGMA foreign_keys = ON", [])?;

    // Set WAL mode for better concurrency
    conn.execute("PRAGMA journal_mode = WAL", [])?;

    Ok(conn)
}
```

### Error Handling

**Upload Errors:**

| Error | HTTP Status | Action |
|-------|-------------|--------|
| Session not found | 404 Not Found | Return error, client aborts |
| Checksum mismatch | 400 Bad Request | Client retries chunk |
| Disk full | 507 Insufficient Storage | Mark upload failed, notify user |
| File > 1GB | 413 Payload Too Large | Reject on prepare |
| Network timeout | (client-side) | Client retries chunk (max 3) |

**Database Errors:**

| Error | Action |
|-------|--------|
| SQLite lock timeout | Retry with exponential backoff (max 5) |
| Foreign key violation | Return 400, log error |
| Disk full | Return 507, alert admin |
| Corruption | Log critical error, fail gracefully |

**Session Edge Cases:**

| Case | Action |
|------|--------|
| Upload for non-existent session | Return 404 |
| Duplicate recording_id | Return 409 Conflict |
| Upload to deleted session | Return 410 Gone |
| Session ID invalid format | Return 400 Bad Request |

### Configuration

**server_config.toml additions:**

```toml
[database]
path = "/var/lib/netpoke/netpoke.db"

[storage]
base_path = "/var/lib/netpoke/uploads"
max_video_size_bytes = 1073741824  # 1 GB
chunk_size_bytes = 1048576          # 1 MB

[cleanup]
failed_upload_retention_days = 2
cleanup_interval_hours = 24

[retention]
default_days = 14

[retention.magic_keys]
# Per-key overrides
# "SURVEY-001" = 30

[analyst_access]
# Username to magic key mapping
# "analyst@example.com" = ["SURVEY-001", "SURVEY-002"]
# "admin@example.com" = ["*"]  # All keys
```

### Metrics Recording Integration

**Modify `server/src/measurements.rs`:**

```rust
// Add to AppState
pub struct AppState {
    // ... existing fields ...
    pub metrics_recorder: Option<Arc<MetricsRecorder>>,
}

// In calculate_and_send_probe_stats()
if let Some(recorder) = &state.metrics_recorder {
    recorder.record_probe_stats(
        &survey_session_id,
        &conn_id,
        timestamp_ms,
        &c2s_stats,
        &s2c_stats,
    ).await?;
}

// In control message handler for client probe stats
if let Some(recorder) = &state.metrics_recorder {
    recorder.record_client_metrics(
        &survey_session_id,
        &conn_id,
        timestamp_ms,
        &s2c_stats,
    ).await?;
}
```

**Add to `server/src/main.rs`:**

```rust
// Initialize MetricsRecorder
let db_path = config.database.path;
let metrics_recorder = Arc::new(MetricsRecorder::new(db_path).await?);

// Add to AppState
app_state.metrics_recorder = Some(metrics_recorder.clone());

// Start background cleanup task
tokio::spawn(cleanup_task(metrics_recorder.db.clone(), config.clone()));
```

## Testing Plan

### Unit Tests

1. **Chunked upload logic**
   - Test SHA-256 checksum calculation
   - Test resume from various byte offsets
   - Test checksum mismatch detection

2. **Database operations**
   - Test session creation/update
   - Test metrics insertion
   - Test soft delete
   - Test foreign key constraints

3. **Access control**
   - Test analyst access validation
   - Test wildcard access
   - Test unauthorized access rejection

### Integration Tests

1. **Upload flow**
   - Upload complete file (happy path)
   - Interrupt and resume upload
   - Upload with checksum mismatch (retry)
   - Upload exceeding size limit (rejection)

2. **Cleanup tasks**
   - Test failed upload cleanup
   - Test retention policy enforcement
   - Test soft delete vs hard delete

3. **Analyst UI**
   - Test session listing with access control
   - Test export to CSV/JSON
   - Test file downloads

### Manual Testing Checklist

- [ ] Upload video from iOS Safari
- [ ] Upload video from Chrome/Desktop
- [ ] Interrupt upload mid-way, resume successfully
- [ ] Edit recording notes before upload
- [ ] View uploaded surveys in analyst UI
- [ ] Export metrics to CSV
- [ ] Download PCAP and verify with Wireshark
- [ ] Verify access control (analyst can't see unauthorized keys)
- [ ] Verify cleanup tasks run on schedule
- [ ] Verify retention policies delete old data

## Migration Path

### Phase 1: Database & Backend (Week 1)

1. Create SQLite schema
2. Implement MetricsRecorder service
3. Integrate metrics recording into existing code
4. Implement upload API endpoints
5. Implement background cleanup task

### Phase 2: Surveyor UI (Week 2)

1. Add upload button to recordings list
2. Implement chunked upload client code
3. Add progress bar and status display
4. Add edit notes functionality
5. Handle upload errors and retry

### Phase 3: Analyst UI (Week 3)

1. Create `/admin/surveys` page
2. Implement session listing with filtering
3. Add metrics visualization (Chart.js)
4. Implement export functionality
5. Add download links for files

### Phase 4: Testing & Refinement (Week 4)

1. Integration testing
2. Performance testing with large files
3. Security audit of access control
4. Documentation
5. Deployment

## Open Questions

None - all questions resolved during brainstorming session.

## Future Enhancements

1. **OAuth Integration** - Replace username/password with OAuth for analyst access
2. **Video + Sensor Sync Playback** - Show sensor data overlaid on video during playback
3. **Survey Comparison** - Compare metrics between multiple surveys
4. **Real-time Upload** - Stream video to server during recording
5. **Client-side Encryption** - Encrypt uploads before transmission
6. **Multi-region Storage** - Distribute uploads across geographic regions
7. **Advanced Search** - Full-text search on notes, filter by date/metrics
8. **API for External Tools** - RESTful API for programmatic access
9. **Webhooks** - Notify external systems when surveys complete

## References

- Existing metrics code: `server/src/measurements.rs`
- Existing survey session tracking: `server/src/data_channels.rs`
- Client recording code: `server/static/nettest.html`
- PCAP/Keylog services: `server/src/packet_capture.rs`, `server/src/dtls_keylog.rs`
