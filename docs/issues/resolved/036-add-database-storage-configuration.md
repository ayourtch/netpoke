# Issue 036: Add Database and Storage Configuration

## Summary
Add configuration options for database path and storage settings to the server configuration system.

## Location
- File: `server/src/config.rs`
- File: `server_config.toml.example`

## Current Behavior
The server configuration does not include database or storage settings needed for the survey upload feature.

## Expected Behavior
Configuration should include:
1. `DatabaseConfig` - database file path
2. `StorageConfig` - upload storage path, max file sizes, chunk size

## Impact
Required for configurable database and storage locations in different deployment environments.

## Suggested Implementation

### Step 1: Add DatabaseConfig struct

Add to `server/src/config.rs`:

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

### Step 2: Add StorageConfig struct

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

### Step 3: Add fields to Config struct

Add to the main `Config` struct:

```rust
#[serde(default)]
pub database: DatabaseConfig,
#[serde(default)]
pub storage: StorageConfig,
```

### Step 4: Update example config file

Add to `server_config.toml.example`:

```toml
[database]
path = "/var/lib/netpoke/netpoke.db"

[storage]
base_path = "/var/lib/netpoke/uploads"
max_video_size_bytes = 1073741824  # 1 GB
chunk_size_bytes = 1048576          # 1 MB
```

### Step 5: Build to verify

```bash
cd server
cargo build
```

## Testing
- Build succeeds
- Config loads with default values when not specified
- Config loads with custom values when specified in TOML

## Dependencies
- None (can be done independently)

## Reference
See `docs/plans/2026-02-05-survey-upload-implementation.md` - Task 4 for full details.

---
*Created: 2026-02-05*
