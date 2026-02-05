# Issue 035: Implement Database Module

## Summary
Create a database module that initializes SQLite, runs migrations, and provides a connection type for use by other modules.

## Location
- File: `server/src/database.rs` (new file)
- File: `server/src/main.rs` (add mod declaration)

## Current Behavior
No database initialization or connection management exists.

## Expected Behavior
A `database.rs` module that:
1. Provides a `DbConnection` type alias for `Arc<Mutex<Connection>>`
2. Implements `init_database()` to open/create the database
3. Runs schema migrations on startup
4. Enables foreign keys and WAL mode for better concurrency

## Impact
This module is the foundation for all database operations in the survey upload feature.

## Suggested Implementation

### Step 1: Create database module

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

### Step 2: Add module declaration to main.rs

Add to `server/src/main.rs` near the top with other `mod` declarations:

```rust
mod database;
```

### Step 3: Run tests

```bash
cd server
cargo test database::tests::test_database_initialization
```

Expected: Test passes.

## Testing
- Unit test verifies all three tables are created
- Test uses tempfile to avoid polluting the filesystem

## Dependencies
- Issue 033: Add database dependencies
- Issue 034: Create database schema migration

## Reference
See `docs/plans/2026-02-05-survey-upload-implementation.md` - Task 3 for full details.

---
*Created: 2026-02-05*
---
*Resolved: 2026-02-05*

## Resolution

Implemented as part of the survey upload feature implementation.
