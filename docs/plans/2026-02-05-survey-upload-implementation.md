# Survey Upload Feature Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement survey video + sensor upload with chunked transfer, SQLite storage, metrics recording, and analyst UI for browsing/exporting data.

**Architecture:** SQLite database for sessions/metrics/recordings, chunked upload API with SHA-256 verification, background cleanup task, surveyor UI in nettest.html, new analyst UI at /admin/surveys.

**Tech Stack:** Rust/Axum backend, SQLite (rusqlite), SHA-256 checksums, vanilla JavaScript frontend, Chart.js for visualization.

---

## Phase 1: Database Foundation

### Task 1: Add Database Dependencies

**Files:**
- Modify: `server/Cargo.toml`

**Step 1: Add rusqlite and related dependencies**

Add to `[dependencies]` section in `server/Cargo.toml`:

```toml
rusqlite = { version = "0.30", features = ["bundled"] }
tokio-rusqlite = "0.5"
sha2 = "0.10"
csv = "1.3"
```

**Step 2: Build to verify dependencies**

```bash
cd server
cargo build
```

Expected: Successful build with new dependencies downloaded.

**Step 3: Commit**

```bash
git add server/Cargo.toml Cargo.lock
git commit -m "deps: add SQLite and CSV dependencies for survey upload feature

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 2: Create Database Schema Migration

**Files:**
- Create: `server/migrations/001_survey_upload_schema.sql`

**Step 1: Create migrations directory**

```bash
mkdir -p server/migrations
```

**Step 2: Write schema SQL file**

Create `server/migrations/001_survey_upload_schema.sql`:

```sql
-- Survey sessions table
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

-- Survey metrics table
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

-- Recordings table
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

**Step 3: Commit**

```bash
git add server/migrations/001_survey_upload_schema.sql
git commit -m "db: add survey upload database schema migration

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 3: Implement Database Module

**Files:**
- Create: `server/src/database.rs`
- Modify: `server/src/main.rs` (add mod declaration)

**Step 1: Create database module**

Create `server/src/database.rs`:

```rust
use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

pub type DbConnection = Arc<Mutex<Connection>>;

/// Initialize the SQLite database
pub async fn init_database(db_path: &Path) -> Result<DbConnection, Box<dyn std::error::Error>> {
    let conn = Connection::open(db_path)?;

    // Enable foreign keys
    conn.execute("PRAGMA foreign_keys = ON", [])?;

    // Set WAL mode for better concurrency
    conn.execute("PRAGMA journal_mode = WAL", [])?;

    // Run migrations
    let schema_sql = include_str!("../migrations/001_survey_upload_schema.sql");
    conn.execute_batch(schema_sql)?;

    Ok(Arc::new(Mutex::new(conn)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_database_initialization() {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = temp_file.path();

        let db = init_database(db_path).await.unwrap();

        // Verify tables were created
        let conn = db.lock().await;
        let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table'").unwrap();
        let tables: Vec<String> = stmt.query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert!(tables.contains(&"survey_sessions".to_string()));
        assert!(tables.contains(&"survey_metrics".to_string()));
        assert!(tables.contains(&"recordings".to_string()));
    }
}
```

**Step 2: Add module declaration to main.rs**

Add to `server/src/main.rs` near the top with other `mod` declarations:

```rust
mod database;
```

**Step 3: Add tempfile dev dependency for tests**

Add to `server/Cargo.toml` under `[dev-dependencies]`:

```toml
[dev-dependencies]
tempfile = "3.10"
```

**Step 4: Run tests**

```bash
cd server
cargo test database::tests::test_database_initialization
```

Expected: Test passes.

**Step 5: Commit**

```bash
git add server/src/database.rs server/src/main.rs server/Cargo.toml
git commit -m "feat: add database initialization module with tests

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 4: Add Database Configuration

**Files:**
- Modify: `server/src/config.rs`
- Modify: `server_config.toml.example`

**Step 1: Add database config struct**

Add to `server/src/config.rs` after the imports:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_database_path")]
    pub path: String,
}

fn default_database_path() -> String {
    "/var/lib/netpoke/netpoke.db".to_string()
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: default_database_path(),
        }
    }
}
```

**Step 2: Add database field to Config struct**

Modify the `Config` struct in `server/src/config.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub security: SecurityConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub capture: CaptureConfig,
    #[serde(default)]
    pub tracing: TracingConfig,
    #[serde(default)]
    pub client: ClientConfig,
    #[serde(default)]
    pub iperf3: Iperf3Config,
    #[serde(default)]
    pub database: DatabaseConfig,  // ADD THIS LINE
}
```

**Step 3: Add storage config struct**

Add to `server/src/config.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    #[serde(default = "default_storage_base_path")]
    pub base_path: String,
    #[serde(default = "default_max_video_size")]
    pub max_video_size_bytes: u64,
    #[serde(default = "default_chunk_size")]
    pub chunk_size_bytes: usize,
}

fn default_storage_base_path() -> String {
    "/var/lib/netpoke/uploads".to_string()
}

fn default_max_video_size() -> u64 {
    1_073_741_824 // 1 GB
}

fn default_chunk_size() -> usize {
    1_048_576 // 1 MB
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            base_path: default_storage_base_path(),
            max_video_size_bytes: default_max_video_size(),
            chunk_size_bytes: default_chunk_size(),
        }
    }
}
```

**Step 4: Add storage field to Config**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub security: SecurityConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub capture: CaptureConfig,
    #[serde(default)]
    pub tracing: TracingConfig,
    #[serde(default)]
    pub client: ClientConfig,
    #[serde(default)]
    pub iperf3: Iperf3Config,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub storage: StorageConfig,  // ADD THIS LINE
}
```

**Step 5: Update example config file**

Add to `server_config.toml.example`:

```toml
[database]
path = "/var/lib/netpoke/netpoke.db"

[storage]
base_path = "/var/lib/netpoke/uploads"
max_video_size_bytes = 1073741824  # 1 GB
chunk_size_bytes = 1048576          # 1 MB
```

**Step 6: Build to verify**

```bash
cd server
cargo build
```

Expected: Successful build.

**Step 7: Commit**

```bash
git add server/src/config.rs server_config.toml.example
git commit -m "config: add database and storage configuration options

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Phase 2: Metrics Recording Service

### Task 5: Implement MetricsRecorder Service

**Files:**
- Create: `server/src/metrics_recorder.rs`
- Modify: `server/src/main.rs`

**Step 1: Create metrics recorder module**

Create `server/src/metrics_recorder.rs`:

```rust
use crate::database::DbConnection;
use common::protocol::DirectionStats;
use rusqlite::params;
use std::sync::Arc;

pub struct MetricsRecorder {
    db: DbConnection,
}

impl MetricsRecorder {
    pub fn new(db: DbConnection) -> Self {
        Self { db }
    }

    /// Record server-side probe stats (both directions)
    pub async fn record_probe_stats(
        &self,
        session_id: &str,
        conn_id: &str,
        timestamp_ms: u64,
        c2s_stats: &DirectionStats,
        s2c_stats: &DirectionStats,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db = self.db.lock().await;
        let now_ms = chrono::Utc::now().timestamp_millis() as u64;

        // Insert c2s metrics
        db.execute(
            "INSERT INTO survey_metrics (
                session_id, timestamp_ms, source, conn_id, direction,
                delay_p50_ms, delay_p99_ms, delay_min_ms, delay_max_ms,
                jitter_p50_ms, jitter_p99_ms, jitter_min_ms, jitter_max_ms,
                rtt_p50_ms, rtt_p99_ms, rtt_min_ms, rtt_max_ms,
                loss_rate, reorder_rate, probe_count, baseline_delay_ms,
                created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                session_id, timestamp_ms, "server", conn_id, "c2s",
                c2s_stats.delay_deviation_ms[0], c2s_stats.delay_deviation_ms[1],
                c2s_stats.delay_deviation_ms[2], c2s_stats.delay_deviation_ms[3],
                c2s_stats.jitter_ms[0], c2s_stats.jitter_ms[1],
                c2s_stats.jitter_ms[2], c2s_stats.jitter_ms[3],
                c2s_stats.rtt_ms[0], c2s_stats.rtt_ms[1],
                c2s_stats.rtt_ms[2], c2s_stats.rtt_ms[3],
                c2s_stats.loss_rate, c2s_stats.reorder_rate,
                c2s_stats.probe_count, c2s_stats.baseline_delay_ms,
                now_ms
            ],
        )?;

        // Insert s2c metrics
        db.execute(
            "INSERT INTO survey_metrics (
                session_id, timestamp_ms, source, conn_id, direction,
                delay_p50_ms, delay_p99_ms, delay_min_ms, delay_max_ms,
                jitter_p50_ms, jitter_p99_ms, jitter_min_ms, jitter_max_ms,
                rtt_p50_ms, rtt_p99_ms, rtt_min_ms, rtt_max_ms,
                loss_rate, reorder_rate, probe_count, baseline_delay_ms,
                created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                session_id, timestamp_ms, "server", conn_id, "s2c",
                s2c_stats.delay_deviation_ms[0], s2c_stats.delay_deviation_ms[1],
                s2c_stats.delay_deviation_ms[2], s2c_stats.delay_deviation_ms[3],
                s2c_stats.jitter_ms[0], s2c_stats.jitter_ms[1],
                s2c_stats.jitter_ms[2], s2c_stats.jitter_ms[3],
                s2c_stats.rtt_ms[0], s2c_stats.rtt_ms[1],
                s2c_stats.rtt_ms[2], s2c_stats.rtt_ms[3],
                s2c_stats.loss_rate, s2c_stats.reorder_rate,
                s2c_stats.probe_count, s2c_stats.baseline_delay_ms,
                now_ms
            ],
        )?;

        Ok(())
    }

    /// Record client-side metrics (s2c only)
    pub async fn record_client_metrics(
        &self,
        session_id: &str,
        conn_id: &str,
        timestamp_ms: u64,
        s2c_stats: &DirectionStats,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db = self.db.lock().await;
        let now_ms = chrono::Utc::now().timestamp_millis() as u64;

        db.execute(
            "INSERT INTO survey_metrics (
                session_id, timestamp_ms, source, conn_id, direction,
                delay_p50_ms, delay_p99_ms, delay_min_ms, delay_max_ms,
                jitter_p50_ms, jitter_p99_ms, jitter_min_ms, jitter_max_ms,
                rtt_p50_ms, rtt_p99_ms, rtt_min_ms, rtt_max_ms,
                loss_rate, reorder_rate, probe_count, baseline_delay_ms,
                created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                session_id, timestamp_ms, "client", conn_id, "s2c",
                s2c_stats.delay_deviation_ms[0], s2c_stats.delay_deviation_ms[1],
                s2c_stats.delay_deviation_ms[2], s2c_stats.delay_deviation_ms[3],
                s2c_stats.jitter_ms[0], s2c_stats.jitter_ms[1],
                s2c_stats.jitter_ms[2], s2c_stats.jitter_ms[3],
                s2c_stats.rtt_ms[0], s2c_stats.rtt_ms[1],
                s2c_stats.rtt_ms[2], s2c_stats.rtt_ms[3],
                s2c_stats.loss_rate, s2c_stats.reorder_rate,
                s2c_stats.probe_count, s2c_stats.baseline_delay_ms,
                now_ms
            ],
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::init_database;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_record_probe_stats() {
        let temp_file = NamedTempFile::new().unwrap();
        let db = init_database(temp_file.path()).await.unwrap();
        let recorder = MetricsRecorder::new(db.clone());

        let c2s_stats = DirectionStats {
            delay_deviation_ms: [1.0, 2.0, 0.5, 3.0],
            rtt_ms: [10.0, 15.0, 8.0, 20.0],
            jitter_ms: [0.5, 1.0, 0.3, 1.5],
            loss_rate: 0.01,
            reorder_rate: 0.005,
            probe_count: 100,
            baseline_delay_ms: 5.0,
        };

        let s2c_stats = c2s_stats.clone();

        recorder
            .record_probe_stats("test-session", "test-conn", 1000, &c2s_stats, &s2c_stats)
            .await
            .unwrap();

        // Verify data was inserted
        let conn = db.lock().await;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM survey_metrics", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2); // One for c2s, one for s2c
    }
}
```

**Step 2: Add module declaration**

Add to `server/src/main.rs`:

```rust
mod metrics_recorder;
```

**Step 3: Run tests**

```bash
cd server
cargo test metrics_recorder::tests::test_record_probe_stats
```

Expected: Test passes.

**Step 4: Commit**

```bash
git add server/src/metrics_recorder.rs server/src/main.rs
git commit -m "feat: implement MetricsRecorder service for storing survey metrics

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 6: Create Session Manager

**Files:**
- Create: `server/src/session_manager.rs`
- Modify: `server/src/main.rs`

**Step 1: Create session manager module**

Create `server/src/session_manager.rs`:

```rust
use crate::database::DbConnection;
use rusqlite::params;

pub struct SessionManager {
    db: DbConnection,
}

impl SessionManager {
    pub fn new(db: DbConnection) -> Self {
        Self { db }
    }

    /// Create a new survey session
    pub async fn create_session(
        &self,
        session_id: &str,
        magic_key: &str,
        user_login: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db = self.db.lock().await;
        let now_ms = chrono::Utc::now().timestamp_millis() as u64;

        db.execute(
            "INSERT INTO survey_sessions (
                session_id, magic_key, user_login, start_time, last_update_time, created_at
            ) VALUES (?, ?, ?, ?, ?, ?)",
            params![session_id, magic_key, user_login, now_ms, now_ms, now_ms],
        )?;

        Ok(())
    }

    /// Update session's last_update_time
    pub async fn update_session_timestamp(
        &self,
        session_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db = self.db.lock().await;
        let now_ms = chrono::Utc::now().timestamp_millis() as u64;

        db.execute(
            "UPDATE survey_sessions SET last_update_time = ? WHERE session_id = ?",
            params![now_ms, session_id],
        )?;

        Ok(())
    }

    /// Update PCAP and keylog paths for a session
    pub async fn update_session_files(
        &self,
        session_id: &str,
        pcap_path: Option<&str>,
        keylog_path: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db = self.db.lock().await;

        db.execute(
            "UPDATE survey_sessions SET pcap_path = ?, keylog_path = ? WHERE session_id = ?",
            params![pcap_path, keylog_path, session_id],
        )?;

        Ok(())
    }

    /// Check if session exists
    pub async fn session_exists(&self, session_id: &str) -> Result<bool, Box<dyn std::error::Error>> {
        let db = self.db.lock().await;

        let count: i64 = db.query_row(
            "SELECT COUNT(*) FROM survey_sessions WHERE session_id = ? AND deleted = 0",
            params![session_id],
            |row| row.get(0),
        )?;

        Ok(count > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::init_database;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_create_and_check_session() {
        let temp_file = NamedTempFile::new().unwrap();
        let db = init_database(temp_file.path()).await.unwrap();
        let manager = SessionManager::new(db);

        manager
            .create_session("test-session", "SURVEY-001", None)
            .await
            .unwrap();

        let exists = manager.session_exists("test-session").await.unwrap();
        assert!(exists);

        let not_exists = manager.session_exists("nonexistent").await.unwrap();
        assert!(!not_exists);
    }

    #[tokio::test]
    async fn test_update_session_timestamp() {
        let temp_file = NamedTempFile::new().unwrap();
        let db = init_database(temp_file.path()).await.unwrap();
        let manager = SessionManager::new(db.clone());

        manager
            .create_session("test-session", "SURVEY-001", None)
            .await
            .unwrap();

        // Wait a bit
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        manager.update_session_timestamp("test-session").await.unwrap();

        // Verify timestamp was updated
        let conn = db.lock().await;
        let times: (i64, i64) = conn
            .query_row(
                "SELECT start_time, last_update_time FROM survey_sessions WHERE session_id = ?",
                params!["test-session"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert!(times.1 > times.0);
    }
}
```

**Step 2: Add module declaration**

Add to `server/src/main.rs`:

```rust
mod session_manager;
```

**Step 3: Run tests**

```bash
cd server
cargo test session_manager::tests
```

Expected: All tests pass.

**Step 4: Commit**

```bash
git add server/src/session_manager.rs server/src/main.rs
git commit -m "feat: implement SessionManager for survey session lifecycle

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Phase 3: Upload API Implementation

### Task 7: Implement Chunk Checksum Utilities

**Files:**
- Create: `server/src/upload_utils.rs`
- Modify: `server/src/main.rs`

**Step 1: Create upload utilities module**

Create `server/src/upload_utils.rs`:

```rust
use sha2::{Sha256, Digest};
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

pub const CHUNK_SIZE: usize = 1_048_576; // 1 MB

/// Calculate SHA-256 checksum of a byte slice
pub fn calculate_checksum(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    hex::encode(result)
}

/// Calculate checksums for all chunks in a file
pub async fn calculate_file_checksums(
    file_path: &Path,
    total_size: u64,
) -> Result<Vec<Option<String>>, Box<dyn std::error::Error>> {
    if !file_path.exists() {
        return Ok(vec![]);
    }

    let mut file = File::open(file_path).await?;
    let num_chunks = ((total_size as f64) / CHUNK_SIZE as f64).ceil() as usize;
    let mut checksums = Vec::with_capacity(num_chunks);
    let mut buffer = vec![0u8; CHUNK_SIZE];

    for _ in 0..num_chunks {
        let bytes_read = file.read(&mut buffer).await?;
        if bytes_read == 0 {
            checksums.push(None);
        } else {
            let checksum = calculate_checksum(&buffer[..bytes_read]);
            checksums.push(Some(checksum));
        }
    }

    Ok(checksums)
}

/// Calculate combined checksum from list of chunk checksums
pub fn calculate_combined_checksum(checksums: &[String]) -> String {
    let combined = checksums.join("");
    let mut hasher = Sha256::new();
    hasher.update(combined.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_calculate_checksum() {
        let data = b"hello world";
        let checksum = calculate_checksum(data);
        // SHA-256 of "hello world"
        assert_eq!(
            checksum,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[tokio::test]
    async fn test_calculate_file_checksums() {
        let mut temp_file = NamedTempFile::new().unwrap();

        // Write 2.5 MB of data (3 chunks)
        let chunk1 = vec![1u8; CHUNK_SIZE];
        let chunk2 = vec![2u8; CHUNK_SIZE];
        let chunk3 = vec![3u8; CHUNK_SIZE / 2];

        temp_file.write_all(&chunk1).unwrap();
        temp_file.write_all(&chunk2).unwrap();
        temp_file.write_all(&chunk3).unwrap();
        temp_file.flush().unwrap();

        let total_size = (CHUNK_SIZE * 2 + CHUNK_SIZE / 2) as u64;
        let checksums = calculate_file_checksums(temp_file.path(), total_size)
            .await
            .unwrap();

        assert_eq!(checksums.len(), 3);
        assert!(checksums[0].is_some());
        assert!(checksums[1].is_some());
        assert!(checksums[2].is_some());
    }

    #[test]
    fn test_calculate_combined_checksum() {
        let checksums = vec![
            "abc123".to_string(),
            "def456".to_string(),
            "ghi789".to_string(),
        ];
        let combined = calculate_combined_checksum(&checksums);
        assert!(!combined.is_empty());
        assert_eq!(combined.len(), 64); // SHA-256 produces 64 hex chars
    }
}
```

**Step 2: Add module declaration**

Add to `server/src/main.rs`:

```rust
mod upload_utils;
```

**Step 3: Run tests**

```bash
cd server
cargo test upload_utils::tests
```

Expected: All tests pass.

**Step 4: Commit**

```bash
git add server/src/upload_utils.rs server/src/main.rs
git commit -m "feat: implement chunk checksum utilities for uploads

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 8: Implement Upload API Handlers (Part 1: Prepare)

**Files:**
- Create: `server/src/upload_api.rs`
- Modify: `server/src/main.rs`

**Step 1: Create upload API module with prepare endpoint**

Create `server/src/upload_api.rs`:

```rust
use crate::database::DbConnection;
use crate::upload_utils::{calculate_file_checksums, CHUNK_SIZE};
use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct UploadState {
    pub db: DbConnection,
    pub storage_base_path: String,
}

#[derive(Debug, Deserialize)]
pub struct PrepareUploadRequest {
    pub session_id: String,
    pub recording_id: String,
    pub video_size_bytes: u64,
    pub sensor_size_bytes: u64,
    pub device_info: serde_json::Value,
    pub user_notes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChunkInfo {
    pub chunk_index: usize,
    pub checksum: String,
}

#[derive(Debug, Serialize)]
pub struct PrepareUploadResponse {
    pub recording_id: String,
    pub video_chunks: Vec<Option<ChunkInfo>>,
    pub sensor_chunks: Vec<Option<ChunkInfo>>,
    pub video_uploaded_bytes: u64,
    pub sensor_uploaded_bytes: u64,
}

/// Prepare upload endpoint
pub async fn prepare_upload(
    State(state): State<Arc<UploadState>>,
    Json(req): Json<PrepareUploadRequest>,
) -> Result<Json<PrepareUploadResponse>, StatusCode> {
    // Verify session exists
    let session_exists = {
        let db = state.db.lock().await;
        let count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM survey_sessions WHERE session_id = ? AND deleted = 0",
                params![&req.session_id],
                |row| row.get(0),
            )
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        count > 0
    };

    if !session_exists {
        return Err(StatusCode::NOT_FOUND);
    }

    // Get session details for path construction
    let (magic_key, start_time): (String, i64) = {
        let db = state.db.lock().await;
        db.query_row(
            "SELECT magic_key, start_time FROM survey_sessions WHERE session_id = ?",
            params![&req.session_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };

    // Construct file paths
    let start_dt = chrono::DateTime::from_timestamp_millis(start_time)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let session_dir = PathBuf::from(&state.storage_base_path)
        .join(&magic_key)
        .join(start_dt.format("%Y").to_string())
        .join(start_dt.format("%m").to_string())
        .join(start_dt.format("%d").to_string())
        .join(&req.session_id);

    // Create directory if it doesn't exist
    tokio::fs::create_dir_all(&session_dir)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let video_path = session_dir.join(format!("{}.webm", req.recording_id));
    let sensor_path = session_dir.join(format!("{}.json", req.recording_id));

    // Calculate existing checksums if files exist
    let video_chunks = calculate_file_checksums(&video_path, req.video_size_bytes)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .enumerate()
        .map(|(idx, cs)| cs.map(|checksum| ChunkInfo { chunk_index: idx, checksum }))
        .collect();

    let sensor_chunks = calculate_file_checksums(&sensor_path, req.sensor_size_bytes)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .enumerate()
        .map(|(idx, cs)| cs.map(|checksum| ChunkInfo { chunk_index: idx, checksum }))
        .collect();

    // Get uploaded bytes from existing files
    let video_uploaded_bytes = if video_path.exists() {
        tokio::fs::metadata(&video_path)
            .await
            .map(|m| m.len())
            .unwrap_or(0)
    } else {
        0
    };

    let sensor_uploaded_bytes = if sensor_path.exists() {
        tokio::fs::metadata(&sensor_path)
            .await
            .map(|m| m.len())
            .unwrap_or(0)
    } else {
        0
    };

    // Create or update recording in database
    {
        let db = state.db.lock().await;
        let now_ms = chrono::Utc::now().timestamp_millis() as u64;
        let device_info_json = serde_json::to_string(&req.device_info)
            .map_err(|_| StatusCode::BAD_REQUEST)?;

        // Try to insert, if exists update
        let result = db.execute(
            "INSERT INTO recordings (
                recording_id, session_id, video_path, sensor_path,
                video_size_bytes, sensor_size_bytes,
                video_uploaded_bytes, sensor_uploaded_bytes,
                device_info_json, user_notes, created_at, upload_status
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(recording_id) DO UPDATE SET
                video_uploaded_bytes = ?,
                sensor_uploaded_bytes = ?,
                user_notes = COALESCE(?, user_notes)",
            params![
                &req.recording_id, &req.session_id,
                video_path.to_string_lossy().as_ref(),
                sensor_path.to_string_lossy().as_ref(),
                req.video_size_bytes, req.sensor_size_bytes,
                video_uploaded_bytes, sensor_uploaded_bytes,
                device_info_json, req.user_notes, now_ms, "uploading",
                video_uploaded_bytes, sensor_uploaded_bytes, req.user_notes
            ],
        );

        result.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok(Json(PrepareUploadResponse {
        recording_id: req.recording_id,
        video_chunks,
        sensor_chunks,
        video_uploaded_bytes,
        sensor_uploaded_bytes,
    }))
}
```

**Step 2: Add module declaration**

Add to `server/src/main.rs`:

```rust
mod upload_api;
```

**Step 3: Build to verify**

```bash
cd server
cargo build
```

Expected: Successful build.

**Step 4: Commit**

```bash
git add server/src/upload_api.rs server/src/main.rs
git commit -m "feat: implement upload prepare endpoint

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 9: Implement Upload API Handlers (Part 2: Chunk Upload)

**Files:**
- Modify: `server/src/upload_api.rs`

**Step 1: Add chunk upload handler**

Add to `server/src/upload_api.rs`:

```rust
use axum::{
    body::Bytes,
    extract::{State, Request},
    http::{StatusCode, HeaderMap},
    Json,
};
use tokio::fs::OpenOptions;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};

#[derive(Debug, Serialize)]
pub struct ChunkUploadResponse {
    pub status: String,
    pub chunk_index: usize,
    pub bytes_received: usize,
}

/// Upload chunk endpoint
pub async fn upload_chunk(
    State(state): State<Arc<UploadState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<ChunkUploadResponse>, StatusCode> {
    // Extract headers
    let recording_id = headers
        .get("X-Recording-Id")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::BAD_REQUEST)?;

    let file_type = headers
        .get("X-File-Type")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::BAD_REQUEST)?;

    let chunk_index: usize = headers
        .get("X-Chunk-Index")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .ok_or(StatusCode::BAD_REQUEST)?;

    let expected_checksum = headers
        .get("X-Chunk-Checksum")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::BAD_REQUEST)?;

    // Verify checksum
    let actual_checksum = crate::upload_utils::calculate_checksum(&body);
    if actual_checksum != expected_checksum {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Get file path from database
    let file_path = {
        let db = state.db.lock().await;
        let column = if file_type == "video" { "video_path" } else { "sensor_path" };
        let query = format!("SELECT {} FROM recordings WHERE recording_id = ?", column);

        let path: String = db
            .query_row(&query, params![recording_id], |row| row.get(0))
            .map_err(|_| StatusCode::NOT_FOUND)?;
        PathBuf::from(path)
    };

    // Open file and write chunk at offset
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .open(&file_path)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let offset = (chunk_index * CHUNK_SIZE) as u64;
    file.seek(std::io::SeekFrom::Start(offset))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    file.write_all(&body)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    file.flush()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Update uploaded bytes in database
    let bytes_received = body.len();
    {
        let db = state.db.lock().await;
        let column = if file_type == "video" {
            "video_uploaded_bytes"
        } else {
            "sensor_uploaded_bytes"
        };
        let query = format!(
            "UPDATE recordings SET {} = {} WHERE recording_id = ?",
            column,
            offset + bytes_received as u64
        );

        db.execute(&query, params![recording_id])
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok(Json(ChunkUploadResponse {
        status: "received".to_string(),
        chunk_index,
        bytes_received,
    }))
}
```

**Step 2: Build to verify**

```bash
cd server
cargo build
```

Expected: Successful build.

**Step 3: Commit**

```bash
git add server/src/upload_api.rs
git commit -m "feat: implement chunk upload endpoint with checksum verification

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 10: Implement Upload API Handlers (Part 3: Finalize)

**Files:**
- Modify: `server/src/upload_api.rs`

**Step 1: Add finalize upload handler**

Add to `server/src/upload_api.rs`:

```rust
#[derive(Debug, Deserialize)]
pub struct FinalizeUploadRequest {
    pub recording_id: String,
    pub video_final_checksum: String,
    pub sensor_final_checksum: String,
}

#[derive(Debug, Serialize)]
pub struct FinalizeUploadResponse {
    pub status: String,
    pub video_verified: bool,
    pub sensor_verified: bool,
}

/// Finalize upload endpoint
pub async fn finalize_upload(
    State(state): State<Arc<UploadState>>,
    Json(req): Json<FinalizeUploadRequest>,
) -> Result<Json<FinalizeUploadResponse>, StatusCode> {
    // Get recording details
    let (video_path, sensor_path, video_size, sensor_size): (String, String, i64, i64) = {
        let db = state.db.lock().await;
        db.query_row(
            "SELECT video_path, sensor_path, video_size_bytes, sensor_size_bytes
             FROM recordings WHERE recording_id = ?",
            params![&req.recording_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|_| StatusCode::NOT_FOUND)?
    };

    // Calculate checksums for video
    let video_checksums = calculate_file_checksums(
        &PathBuf::from(&video_path),
        video_size as u64,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    let video_combined = crate::upload_utils::calculate_combined_checksum(&video_checksums);
    let video_verified = video_combined == req.video_final_checksum;

    // Calculate checksums for sensor
    let sensor_checksums = calculate_file_checksums(
        &PathBuf::from(&sensor_path),
        sensor_size as u64,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    let sensor_combined = crate::upload_utils::calculate_combined_checksum(&sensor_checksums);
    let sensor_verified = sensor_combined == req.sensor_final_checksum;

    // Update recording status
    if video_verified && sensor_verified {
        let db = state.db.lock().await;
        let now_ms = chrono::Utc::now().timestamp_millis() as u64;

        db.execute(
            "UPDATE recordings SET upload_status = 'complete', completed_at = ? WHERE recording_id = ?",
            params![now_ms, &req.recording_id],
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    } else {
        let db = state.db.lock().await;
        db.execute(
            "UPDATE recordings SET upload_status = 'failed' WHERE recording_id = ?",
            params![&req.recording_id],
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok(Json(FinalizeUploadResponse {
        status: if video_verified && sensor_verified {
            "complete".to_string()
        } else {
            "failed".to_string()
        },
        video_verified,
        sensor_verified,
    }))
}
```

**Step 2: Build to verify**

```bash
cd server
cargo build
```

Expected: Successful build.

**Step 3: Commit**

```bash
git add server/src/upload_api.rs
git commit -m "feat: implement upload finalize endpoint with verification

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Phase 4: Wire Up Services and Routes

### Task 11: Initialize Services in main.rs

**Files:**
- Modify: `server/src/main.rs`

**Step 1: Add service initialization in main**

Find the main function in `server/src/main.rs` and add database initialization after config loading:

```rust
// Initialize database
let db_path = std::path::PathBuf::from(&config.database.path);
if let Some(parent) = db_path.parent() {
    tokio::fs::create_dir_all(parent).await?;
}
let db = crate::database::init_database(&db_path).await?;

// Initialize services
let metrics_recorder = Arc::new(crate::metrics_recorder::MetricsRecorder::new(db.clone()));
let session_manager = Arc::new(crate::session_manager::SessionManager::new(db.clone()));

// Add to AppState
app_state.metrics_recorder = Some(metrics_recorder.clone());
app_state.session_manager = Some(session_manager.clone());
```

**Step 2: Add fields to AppState**

Find the AppState struct in `server/src/state.rs` and add:

```rust
pub metrics_recorder: Option<Arc<crate::metrics_recorder::MetricsRecorder>>,
pub session_manager: Option<Arc<crate::session_manager::SessionManager>>,
```

**Step 3: Build to verify**

```bash
cd server
cargo build
```

Expected: Successful build.

**Step 4: Commit**

```bash
git add server/src/main.rs server/src/state.rs
git commit -m "feat: initialize database and services in main

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

---

### Task 12: Add Upload API Routes

**Files:**
- Modify: `server/src/main.rs`

**Step 1: Create upload router**

Add after other route definitions in `main.rs`:

```rust
// Upload API routes
let upload_state = Arc::new(crate::upload_api::UploadState {
    db: db.clone(),
    storage_base_path: config.storage.base_path.clone(),
});

let upload_routes = axum::Router::new()
    .route("/api/upload/prepare", axum::routing::post(crate::upload_api::prepare_upload))
    .route("/api/upload/chunk", axum::routing::post(crate::upload_api::upload_chunk))
    .route("/api/upload/finalize", axum::routing::post(crate::upload_api::finalize_upload))
    .with_state(upload_state);
```

**Step 2: Merge with main router**

```rust
let app = app.merge(upload_routes);
```

**Step 3: Build and run**

```bash
cd server
cargo build
cargo run
```

**Step 4: Commit**

```bash
git add server/src/main.rs
git commit -m "feat: add upload API routes to server

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Phase 5: Client Upload UI

### Task 13: Add Upload UI to nettest.html

**Files:**
- Modify: `server/static/nettest.html`

**Step 1: Add upload button and progress UI to recording items**

Find the recording item HTML and add upload button and progress bar. Search for the recording-item-actions div and add:

```html
<button onclick="startUpload(this.dataset.recordingId)"
        class="capture-btn green upload-btn"
        data-recording-id="RECORDING_ID_HERE">
    📤 Upload
</button>

<!-- Progress bar -->
<div class="upload-progress" id="progress-RECORDING_ID" style="display:none;">
    <div class="progress-bar">
        <div class="progress-fill" style="width: 0%"></div>
    </div>
    <span class="progress-text">Uploading: 0%</span>
</div>
```

**Step 2: Add CSS for upload UI**

Add to style section:

```css
.upload-progress {
    margin-top: 8px;
}

.progress-bar {
    width: 100%;
    height: 20px;
    background-color: #e0e0e0;
    border-radius: 10px;
    overflow: hidden;
}

.progress-fill {
    height: 100%;
    background: linear-gradient(90deg, #2196F3, #4CAF50);
    transition: width 0.3s ease;
}

.progress-text {
    font-size: 12px;
    color: #666;
    margin-top: 4px;
    display: block;
}
```

**Step 3: Commit**

```bash
git add server/static/nettest.html
git commit -m "ui: add upload button and progress bar to recordings

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 14: Implement Client Upload Logic

**Files:**
- Modify: `server/static/nettest.html`

**Step 1: Add upload functions to JavaScript**

Add to script section:

```javascript
const CHUNK_SIZE = 1048576; // 1 MB

async function calculateSHA256(data) {
    const hashBuffer = await crypto.subtle.digest('SHA-256', data);
    const hashArray = Array.from(new Uint8Array(hashBuffer));
    return hashArray.map(b => b.toString(16).padStart(2, '0')).join('');
}

async function startUpload(recordingId) {
    const recording = await getRecordingFromIndexedDB(recordingId);
    if (!recording) {
        alert('Recording not found');
        return;
    }

    const button = document.querySelector(`[data-recording-id="${recordingId}"]`);
    const progressDiv = document.getElementById(`progress-${recordingId}`);
    const progressFill = progressDiv.querySelector('.progress-fill');
    const progressText = progressDiv.querySelector('.progress-text');

    button.disabled = true;
    progressDiv.style.display = 'block';

    try {
        // Step 1: Prepare upload
        const deviceInfo = {
            browser: navigator.userAgent,
            os: navigator.platform,
            screen_width: window.screen.width,
            screen_height: window.screen.height
        };

        const prepareReq = {
            session_id: surveySessionId,
            recording_id: recordingId,
            video_size_bytes: recording.video.size,
            sensor_size_bytes: new Blob([JSON.stringify(recording.sensors)]).size,
            device_info: deviceInfo,
            user_notes: recording.notes || null
        };

        const prepareResp = await fetch('/api/upload/prepare', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(prepareReq)
        });

        if (!prepareResp.ok) throw new Error('Prepare failed');
        const prepareData = await prepareResp.json();

        // Step 2: Upload video chunks
        await uploadFile(
            recording.video,
            recordingId,
            'video',
            prepareData.video_chunks,
            (progress) => {
                progressFill.style.width = `${progress}%`;
                progressText.textContent = `Uploading video: ${progress}%`;
            }
        );

        // Step 3: Upload sensor data
        const sensorBlob = new Blob([JSON.stringify(recording.sensors)],
                                     { type: 'application/json' });
        await uploadFile(
            sensorBlob,
            recordingId,
            'sensor',
            prepareData.sensor_chunks,
            (progress) => {
                progressFill.style.width = `${progress}%`;
                progressText.textContent = `Uploading sensors: ${progress}%`;
            }
        );

        // Step 4: Finalize
        const videoChecksums = await calculateAllChunkChecksums(recording.video);
        const sensorChecksums = await calculateAllChunkChecksums(sensorBlob);

        const finalChecksum = await calculateSHA256(
            new TextEncoder().encode(videoChecksums.join(''))
        );
        const sensorFinalChecksum = await calculateSHA256(
            new TextEncoder().encode(sensorChecksums.join(''))
        );

        const finalizeResp = await fetch('/api/upload/finalize', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                recording_id: recordingId,
                video_final_checksum: finalChecksum,
                sensor_final_checksum: sensorFinalChecksum
            })
        });

        if (!finalizeResp.ok) throw new Error('Finalize failed');

        progressText.textContent = '✓ Upload complete';
        button.textContent = '✓ Uploaded';
        button.classList.add('success');

    } catch (error) {
        console.error('Upload failed:', error);
        progressText.textContent = '⚠️ Upload failed';
        button.disabled = false;
        button.textContent = '⚠️ Retry Upload';
        button.classList.add('error');
    }
}

async function uploadFile(blob, recordingId, fileType, existingChunks, onProgress) {
    const totalChunks = Math.ceil(blob.size / CHUNK_SIZE);

    for (let i = 0; i < totalChunks; i++) {
        const start = i * CHUNK_SIZE;
        const end = Math.min(start + CHUNK_SIZE, blob.size);
        const chunk = blob.slice(start, end);
        const chunkData = await chunk.arrayBuffer();
        const checksum = await calculateSHA256(chunkData);

        // Skip if server already has this chunk with matching checksum
        if (existingChunks[i] && existingChunks[i].checksum === checksum) {
            onProgress(Math.round(((i + 1) / totalChunks) * 100));
            continue;
        }

        // Upload chunk
        const response = await fetch('/api/upload/chunk', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/octet-stream',
                'X-Recording-Id': recordingId,
                'X-File-Type': fileType,
                'X-Chunk-Index': i.toString(),
                'X-Chunk-Checksum': checksum
            },
            body: chunkData
        });

        if (!response.ok) throw new Error(`Chunk ${i} upload failed`);

        onProgress(Math.round(((i + 1) / totalChunks) * 100));
    }
}

async function calculateAllChunkChecksums(blob) {
    const checksums = [];
    const totalChunks = Math.ceil(blob.size / CHUNK_SIZE);

    for (let i = 0; i < totalChunks; i++) {
        const start = i * CHUNK_SIZE;
        const end = Math.min(start + CHUNK_SIZE, blob.size);
        const chunk = blob.slice(start, end);
        const chunkData = await chunk.arrayBuffer();
        const checksum = await calculateSHA256(chunkData);
        checksums.push(checksum);
    }

    return checksums;
}
```

**Step 2: Commit**

```bash
git add server/static/nettest.html
git commit -m "feat: implement client-side chunked upload with resume

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Phase 6: Metrics Integration

### Task 15: Integrate MetricsRecorder into Measurements

**Files:**
- Modify: `server/src/measurements.rs`
- Modify: `server/src/data_channels.rs`

**Step 1: Add metrics recording to probe stats**

Find the function that sends ProbeStats messages in `measurements.rs` and add:

```rust
// After creating probe_stats_report
if let Some(recorder) = &state.metrics_recorder {
    if let Err(e) = recorder.record_probe_stats(
        &survey_session_id,
        &conn_id,
        timestamp_ms,
        &c2s_stats,
        &s2c_stats,
    ).await {
        tracing::error!("Failed to record probe stats: {}", e);
    }
}
```

**Step 2: Update session timestamp on metrics**

Add before recording metrics:

```rust
if let Some(session_mgr) = &state.session_manager {
    if let Err(e) = session_mgr.update_session_timestamp(&survey_session_id).await {
        tracing::error!("Failed to update session timestamp: {}", e);
    }
}
```

**Step 3: Create session on survey start**

In `data_channels.rs`, find where StartSurveySession is handled and add:

```rust
if let Some(session_mgr) = &state.session_manager {
    // Get magic_key from current auth context
    let magic_key = ""; // TODO: Extract from auth context

    if let Err(e) = session_mgr.create_session(
        &survey_session_id,
        magic_key,
        None // user_login if available
    ).await {
        tracing::error!("Failed to create survey session: {}", e);
    }
}
```

**Step 4: Commit**

```bash
git add server/src/measurements.rs server/src/data_channels.rs
git commit -m "feat: integrate metrics recording into survey flow

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Phase 7: Analyst UI (Simplified for MVP)

### Task 16: Create Basic Analyst API Endpoints

**Files:**
- Create: `server/src/analyst_api.rs`
- Modify: `server/src/main.rs`

**Step 1: Create analyst API module**

Create `server/src/analyst_api.rs`:

```rust
use crate::database::DbConnection;
use axum::{extract::{State, Path, Query}, http::StatusCode, Json};
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
```

**Step 2: Add routes**

In `main.rs`:

```rust
let analyst_state = Arc::new(crate::analyst_api::AnalystState {
    db: db.clone(),
});

let analyst_routes = axum::Router::new()
    .route("/admin/api/sessions", axum::routing::get(crate::analyst_api::list_sessions))
    .with_state(analyst_state);

let app = app.merge(analyst_routes);
```

**Step 3: Commit**

```bash
git add server/src/analyst_api.rs server/src/main.rs
git commit -m "feat: add basic analyst API for listing survey sessions

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Phase 8: Testing and Documentation

### Task 17: Manual Integration Test

**Step 1: Start server**

```bash
cd server
cargo run
```

**Step 2: Test upload flow manually**

1. Open browser to nettest.html
2. Start survey ("Analyze Network")
3. Record video with sensors
4. Click upload button
5. Verify progress bar updates
6. Check upload completes successfully

**Step 3: Verify database**

```bash
sqlite3 /var/lib/netpoke/netpoke.db
SELECT * FROM survey_sessions;
SELECT * FROM recordings;
SELECT COUNT(*) FROM survey_metrics;
```

**Step 4: Verify files on disk**

```bash
ls -lR /var/lib/netpoke/uploads/
```

**Step 5: Document test results**

Create test report noting any issues found.

---

### Task 18: Update Documentation

**Files:**
- Create: `docs/SURVEY_UPLOAD.md`

**Step 1: Write usage documentation**

```markdown
# Survey Upload Feature

## For Surveyors

1. Run network survey as normal
2. Record videos with sensors during survey
3. Click "Upload" button on any recording
4. Wait for progress bar to complete
5. Recordings are stored on server

## For Analysts

Access `/admin/surveys` to:
- Browse survey sessions by magic key
- View and download recordings
- Export metrics data

## Configuration

See `server_config.toml.example` for:
- Database path
- Storage path
- Retention policies
```

**Step 2: Commit**

```bash
git add docs/SURVEY_UPLOAD.md
git commit -m "docs: add survey upload feature documentation

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Summary

This implementation plan covers:

1. **Database foundation** - SQLite schema, initialization, config
2. **Core services** - MetricsRecorder, SessionManager
3. **Upload API** - Prepare, chunk upload, finalize endpoints
4. **Client UI** - Upload button, progress bar, chunked upload logic
5. **Metrics integration** - Recording probe stats to database
6. **Analyst API** - Basic session listing endpoint
7. **Testing** - Manual integration test
8. **Documentation** - User guide

**Not included in this plan (future work):**
- Cleanup background task (simple cron job for now)
- Full analyst UI with charts (use basic API for now)
- Access control middleware (add in security pass)
- CSV/JSON export (add after MVP)
- Retention policies enforcement (manual cleanup for now)

**Estimated completion:** 3-4 days for experienced Rust developer

**Next steps after completion:**
1. Deploy to test environment
2. User acceptance testing
3. Add remaining features iteratively
