# 052 - Database initialization fails due to PRAGMA journal_mode returning rows

## Summary

When the database is configured, the SQLite file gets auto-created but remains empty (no tables).
The uploads directory is not auto-created at startup. Attempting to upload returns
"Upload feature is unavailable - database not configured".

## Location

- `server/src/database.rs`: `init_database()` function, line with `PRAGMA journal_mode = WAL`
- `server/src/main.rs`: Storage path initialization (missing uploads directory auto-creation)

## Current Behavior

1. `Connection::open()` creates the database file successfully
2. `conn.execute("PRAGMA journal_mode = WAL", [])` fails with `ExecuteReturnedResults` error
   because `PRAGMA journal_mode` returns a result row and rusqlite 0.30's `execute()` rejects
   statements that return rows
3. `init_database()` returns `Err`, so `db` becomes `None` in main.rs
4. The database file exists but is empty (schema migrations never ran)
5. The uploads base directory is never created (no code to create it at startup)
6. Upload API endpoints check `state.db` which is `None` → "database not configured" error

## Expected Behavior

1. Database initializes successfully with all tables created
2. Uploads base directory is auto-created at startup when database is available
3. Upload API endpoints work when database is configured

## Impact

All upload functionality is completely broken. Users cannot upload survey recordings.

## Root Cause Analysis

In rusqlite 0.30, `Connection::execute()` calls `Statement::execute()` which returns
`Err(Error::ExecuteReturnedResults)` when the SQL statement returns rows. Unlike most
setter PRAGMAs (e.g., `PRAGMA foreign_keys = ON`), `PRAGMA journal_mode = WAL` always
returns a result row containing the new journal mode. The correct API to use is
`Connection::pragma_update()` which is designed to handle this.

Additionally, there was no code to auto-create the uploads storage base directory at
startup. The per-session directory creation in `prepare_upload` uses `create_dir_all`,
which would also create the base directory, but this code is never reached because the
database error causes uploads to be disabled entirely.

## Resolution

### Changes Made

1. **`server/src/database.rs`**: Changed `conn.execute("PRAGMA journal_mode = WAL", [])` to
   `conn.pragma_update(None, "journal_mode", "WAL")` which correctly handles the result row.

2. **`server/src/main.rs`**: Added auto-creation of the uploads base directory at startup
   when the database is successfully initialized, using `std::fs::create_dir_all()`.

### Files Modified

- `server/src/database.rs`
- `server/src/main.rs`

### Verified

- Confirmed `execute()` returns `ExecuteReturnedResults` for `PRAGMA journal_mode = WAL` in rusqlite 0.30
- Confirmed `pragma_update()` works correctly for setting WAL mode
- Database initialization now succeeds with all tables created
