-- Survey Upload Schema Migration
-- Version: 001
-- Description: Initial schema for survey sessions, metrics, and recordings

-- Survey sessions table - stores metadata about each survey session
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

-- Survey metrics table - stores per-second metrics from client and server
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

-- Recordings table - stores metadata for uploaded video + sensor recordings
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
