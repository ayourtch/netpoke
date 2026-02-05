//! Session manager service for survey session lifecycle management
//!
//! Provides database operations for creating, updating, and querying survey sessions.

use crate::database::DbConnection;
use rusqlite::params;

/// Service for managing survey session lifecycle
pub struct SessionManager {
    db: DbConnection,
}

impl SessionManager {
    /// Create a new SessionManager with the given database connection
    pub fn new(db: DbConnection) -> Self {
        Self { db }
    }

    /// Create a new survey session in the database
    ///
    /// # Arguments
    /// * `session_id` - Unique session identifier (UUID)
    /// * `magic_key` - Magic key used for authentication
    /// * `user_login` - Optional user login (if authenticated via OAuth)
    pub async fn create_session(
        &self,
        session_id: &str,
        magic_key: &str,
        user_login: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
    ///
    /// Called periodically when metrics are recorded to indicate session activity.
    pub async fn update_session_timestamp(
        &self,
        session_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let db = self.db.lock().await;
        let now_ms = chrono::Utc::now().timestamp_millis() as u64;

        db.execute(
            "UPDATE survey_sessions SET last_update_time = ? WHERE session_id = ?",
            params![now_ms, session_id],
        )?;

        Ok(())
    }

    /// Update PCAP and keylog paths for a session
    ///
    /// Called when a survey session is explicitly stopped to record file locations.
    pub async fn update_session_files(
        &self,
        session_id: &str,
        pcap_path: Option<&str>,
        keylog_path: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let db = self.db.lock().await;

        db.execute(
            "UPDATE survey_sessions SET pcap_path = ?, keylog_path = ? WHERE session_id = ?",
            params![pcap_path, keylog_path, session_id],
        )?;

        Ok(())
    }

    /// Check if session exists and is not deleted
    pub async fn session_exists(
        &self,
        session_id: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
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
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::init_database;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_create_and_check_session() {
        let temp_file = NamedTempFile::new().unwrap();
        let db = init_database(temp_file.path()).unwrap();
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
    async fn test_create_session_with_user() {
        let temp_file = NamedTempFile::new().unwrap();
        let db = init_database(temp_file.path()).unwrap();
        let manager = SessionManager::new(db.clone());

        manager
            .create_session("test-session", "SURVEY-001", Some("user@example.com"))
            .await
            .unwrap();

        // Verify user_login was stored
        let conn = db.lock().await;
        let user_login: Option<String> = conn
            .query_row(
                "SELECT user_login FROM survey_sessions WHERE session_id = ?",
                params!["test-session"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(user_login, Some("user@example.com".to_string()));
    }

    #[tokio::test]
    async fn test_update_session_timestamp() {
        let temp_file = NamedTempFile::new().unwrap();
        let db = init_database(temp_file.path()).unwrap();
        let manager = SessionManager::new(db.clone());

        manager
            .create_session("test-session", "SURVEY-001", None)
            .await
            .unwrap();

        // Wait a small amount of time
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        manager
            .update_session_timestamp("test-session")
            .await
            .unwrap();

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

    #[tokio::test]
    async fn test_get_session_magic_key() {
        let temp_file = NamedTempFile::new().unwrap();
        let db = init_database(temp_file.path()).unwrap();
        let manager = SessionManager::new(db);

        manager
            .create_session("test-session", "SURVEY-001", None)
            .await
            .unwrap();

        let magic_key = manager
            .get_session_magic_key("test-session")
            .await
            .unwrap();
        assert_eq!(magic_key, Some("SURVEY-001".to_string()));

        let no_key = manager
            .get_session_magic_key("nonexistent")
            .await
            .unwrap();
        assert!(no_key.is_none());
    }

    #[tokio::test]
    async fn test_update_session_files() {
        let temp_file = NamedTempFile::new().unwrap();
        let db = init_database(temp_file.path()).unwrap();
        let manager = SessionManager::new(db.clone());

        manager
            .create_session("test-session", "SURVEY-001", None)
            .await
            .unwrap();

        manager
            .update_session_files(
                "test-session",
                Some("/var/lib/netpoke/test.pcap"),
                Some("/var/lib/netpoke/test.keylog"),
            )
            .await
            .unwrap();

        // Verify paths were stored
        let conn = db.lock().await;
        let paths: (Option<String>, Option<String>) = conn
            .query_row(
                "SELECT pcap_path, keylog_path FROM survey_sessions WHERE session_id = ?",
                params!["test-session"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(paths.0, Some("/var/lib/netpoke/test.pcap".to_string()));
        assert_eq!(paths.1, Some("/var/lib/netpoke/test.keylog".to_string()));
    }
}
