# Issue 043: Initialize Services in main.rs

## Summary
Initialize the database, MetricsRecorder, and SessionManager services in the server's main function and add them to AppState.

## Location
- File: `server/src/main.rs`
- File: `server/src/state.rs`

## Current Behavior
No database or survey-related services are initialized at startup.

## Expected Behavior
On server startup:
1. Create database directory if needed
2. Initialize SQLite database connection
3. Create MetricsRecorder and SessionManager service instances
4. Store service references in AppState for use by handlers

## Impact
Required for all survey features to have access to the database and services.

## Suggested Implementation

### Step 1: Add fields to AppState

In `server/src/state.rs`, add to the `AppState` struct:

```rust
use crate::database::DbConnection;
use crate::metrics_recorder::MetricsRecorder;
use crate::session_manager::SessionManager;

pub struct AppState {
    // ... existing fields ...
    pub db: Option<DbConnection>,
    pub metrics_recorder: Option<Arc<MetricsRecorder>>,
    pub session_manager: Option<Arc<SessionManager>>,
}
```

### Step 2: Add initialization to main.rs

In the main function, after config loading but before route setup:

```rust
// Initialize database
let db_path = std::path::PathBuf::from(&config.database.path);
if let Some(parent) = db_path.parent() {
    if !parent.exists() {
        tokio::fs::create_dir_all(parent).await?;
    }
}
let db = match crate::database::init_database(&db_path).await {
    Ok(db) => {
        tracing::info!("Database initialized at {:?}", db_path);
        Some(db)
    }
    Err(e) => {
        tracing::warn!("Failed to initialize database: {}. Survey features disabled.", e);
        None
    }
};

// Initialize services (only if database is available)
let (metrics_recorder, session_manager) = if let Some(ref db) = db {
    (
        Some(Arc::new(crate::metrics_recorder::MetricsRecorder::new(db.clone()))),
        Some(Arc::new(crate::session_manager::SessionManager::new(db.clone()))),
    )
} else {
    (None, None)
};

// Add to AppState
let app_state = AppState {
    // ... existing fields ...
    db: db.clone(),
    metrics_recorder,
    session_manager,
};
```

### Step 3: Initialize default values

Update the `AppState::default()` or `new()` implementation:

```rust
impl AppState {
    pub fn new() -> Self {
        Self {
            // ... existing fields ...
            db: None,
            metrics_recorder: None,
            session_manager: None,
        }
    }
}
```

## Testing
- Build succeeds: `cargo build`
- Server starts without errors when database directory is writable
- Server starts with warning when database cannot be created (graceful degradation)

## Dependencies
- Issue 035: Implement database module
- Issue 037: Implement MetricsRecorder service
- Issue 038: Create SessionManager service
- Issue 036: Add database configuration

## Reference
See `docs/plans/2026-02-05-survey-upload-implementation.md` - Task 11 for full details.

---
*Created: 2026-02-05*
