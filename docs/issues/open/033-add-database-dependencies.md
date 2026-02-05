# Issue 033: Add Database Dependencies

## Summary
Add SQLite and related dependencies to the server's Cargo.toml to support the survey upload feature.

## Location
- File: `server/Cargo.toml`

## Current Behavior
The server does not have SQLite or CSV dependencies required for storing survey sessions, metrics, and recordings.

## Expected Behavior
The server should have rusqlite, tokio-rusqlite, sha2, and csv dependencies available for the survey upload feature implementation.

## Impact
This is a foundational change required before any database-related survey upload features can be implemented.

## Suggested Implementation

### Step 1: Add dependencies to server/Cargo.toml

Add to `[dependencies]` section:

```toml
rusqlite = { version = "0.30", features = ["bundled"] }
tokio-rusqlite = "0.5"
sha2 = "0.10"
csv = "1.3"
```

### Step 2: Add test dependency

Add to `[dev-dependencies]` section:

```toml
tempfile = "3.10"
```

### Step 3: Build to verify

```bash
cd server
cargo build
```

Expected: Successful build with new dependencies downloaded.

## Testing
- Run `cargo build` to verify dependencies compile successfully
- Verify no dependency conflicts with existing crates

## Reference
See `docs/plans/2026-02-05-survey-upload-implementation.md` - Task 1 for full details.

---
*Created: 2026-02-05*
