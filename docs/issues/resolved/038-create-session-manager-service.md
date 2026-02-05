# Issue 038: Create SessionManager Service

## Summary
Create a service that manages survey session lifecycle in the database, including creation, timestamp updates, and PCAP/keylog path storage.

## Location
- File: `server/src/session_manager.rs` (new file)
- File: `server/src/main.rs` (add mod declaration)

## Current Behavior
Survey sessions are tracked in memory only with no persistent storage.

## Expected Behavior
A `SessionManager` service that:
1. Creates new survey sessions in the database
2. Updates session timestamps when metrics arrive
3. Records PCAP and keylog file paths when surveys stop
4. Checks if sessions exist and are not deleted

## Impact
Enables persistent survey session tracking that survives server restarts and supports upload feature.

## Suggested Implementation

### Step 1: Create session manager module

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

    /// Update PCAP and keylog paths for a session (called when survey stops)
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

    /// Check if session exists and is not deleted
    pub async fn session_exists(&self, session_id: &str) -> Result<bool, Box<dyn std::error::Error>> {
        let db = self.db.lock().await;

        let count: i64 = db.query_row(
            "SELECT COUNT(*) FROM survey_sessions WHERE session_id = ? AND deleted = 0",
            params![session_id],
            |row| row.get(0),
        )?;

        Ok(count > 0)
    }
    
    /// Get session's magic key
    pub async fn get_session_magic_key(
        &self,
        session_id: &str,
    ) -> Result<Option<String>, Box<dyn std::error::Error>> {
        let db = self.db.lock().await;

        let result: Result<String, _> = db.query_row(
            "SELECT magic_key FROM survey_sessions WHERE session_id = ? AND deleted = 0",
            params![session_id],
            |row| row.get(0),
        );

        match result {
            Ok(magic_key) => Ok(Some(magic_key)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}
```

### Step 2: Add unit tests

```rust
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

### Step 3: Add module declaration

Add to `server/src/main.rs`:
```rust
mod session_manager;
```

## Testing

```bash
cd server
cargo test session_manager::tests
```

## Dependencies
- Issue 033: Add database dependencies
- Issue 034: Create database schema migration
- Issue 035: Implement database module

## Reference
See `docs/plans/2026-02-05-survey-upload-implementation.md` - Task 6 for full details.

---
*Created: 2026-02-05*
