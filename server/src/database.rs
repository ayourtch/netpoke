//! Database module for survey data persistence
//!
//! Provides SQLite database initialization and connection management for
//! storing survey sessions, metrics, and recording metadata.

use rusqlite::Connection;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Thread-safe database connection type
pub type DbConnection = Arc<Mutex<Connection>>;

/// Initialize the SQLite database
///
/// Opens (or creates) a SQLite database at the specified path, enables
/// foreign key constraints and WAL mode for better concurrency, and runs
/// schema migrations.
///
/// # Arguments
/// * `db_path` - Path to the SQLite database file
///
/// # Returns
/// * `Ok(DbConnection)` - Thread-safe connection on success
/// * `Err` - Database initialization error
pub fn init_database(db_path: &Path) -> Result<DbConnection, Box<dyn std::error::Error + Send + Sync>> {
    let conn = Connection::open(db_path)?;

    // Enable foreign keys
    conn.execute("PRAGMA foreign_keys = ON", [])?;

    // Set WAL mode for better concurrency
    // Note: pragma_update must be used instead of execute because
    // PRAGMA journal_mode returns a result row, and rusqlite's execute()
    // returns an error for statements that return rows.
    conn.pragma_update(None, "journal_mode", "WAL")?;

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

        let db = init_database(db_path).unwrap();

        // Verify tables were created
        let conn = db.lock().await;
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap();
        let tables: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert!(tables.contains(&"survey_sessions".to_string()));
        assert!(tables.contains(&"survey_metrics".to_string()));
        assert!(tables.contains(&"recordings".to_string()));
    }

    #[tokio::test]
    async fn test_foreign_keys_enabled() {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = temp_file.path();

        let db = init_database(db_path).unwrap();
        let conn = db.lock().await;

        // Check that foreign keys are enabled
        let fk_enabled: i32 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(fk_enabled, 1);
    }
}
