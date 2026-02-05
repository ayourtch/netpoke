# Issue 039: Implement Chunk Checksum Utilities

## Summary
Create utility functions for calculating SHA-256 checksums of upload chunks, supporting the resumable chunked upload protocol.

## Location
- File: `server/src/upload_utils.rs` (new file)
- File: `server/src/main.rs` (add mod declaration)

## Current Behavior
No checksum calculation utilities exist for upload verification.

## Expected Behavior
Utility functions that:
1. Calculate SHA-256 checksum of a byte slice
2. Calculate checksums for all 1MB chunks in a file
3. Calculate a combined checksum from a list of chunk checksums

## Impact
Provides the cryptographic foundation for verifying upload integrity and enabling resume capability.

## Suggested Implementation

### Step 1: Create upload utilities module

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
```

### Step 2: Add unit tests

```rust
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

    #[tokio::test]
    async fn test_calculate_file_checksums_nonexistent() {
        let checksums = calculate_file_checksums(
            Path::new("/nonexistent/file"),
            1000,
        ).await.unwrap();
        
        assert!(checksums.is_empty());
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

### Step 3: Add module declaration

Add to `server/src/main.rs`:
```rust
mod upload_utils;
```

## Testing

```bash
cd server
cargo test upload_utils::tests
```

## Dependencies
- Issue 033: Add database dependencies (for sha2 and hex crates)

## Reference
See `docs/plans/2026-02-05-survey-upload-implementation.md` - Task 7 for full details.
See `docs/plans/2026-02-05-survey-upload-feature-design.md` - Chunked Upload Mechanism section.

---
*Created: 2026-02-05*
