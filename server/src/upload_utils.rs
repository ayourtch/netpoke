//! Upload utilities for chunked file uploads
//!
//! Provides SHA-256 checksum calculation for upload verification and resume capability.

use sha2::{Digest, Sha256};
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

/// Default chunk size for uploads: 1 MB
pub const CHUNK_SIZE: usize = 1_048_576;

/// Calculate SHA-256 checksum of a byte slice
///
/// # Arguments
/// * `data` - Byte slice to hash
///
/// # Returns
/// Lowercase hex-encoded SHA-256 hash string
pub fn calculate_checksum(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    hex::encode(result)
}

/// Calculate checksums for all chunks in a file
///
/// Reads the file in chunks and calculates SHA-256 checksums for each chunk.
/// Returns empty vec if file doesn't exist.
///
/// # Arguments
/// * `file_path` - Path to the file to process
/// * `total_size` - Expected total size of the file (used to calculate number of chunks)
///
/// # Returns
/// Vector of optional checksums, one per chunk. None if chunk not yet written.
pub async fn calculate_file_checksums(
    file_path: &Path,
    total_size: u64,
) -> Result<Vec<Option<String>>, Box<dyn std::error::Error + Send + Sync>> {
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
///
/// Concatenates all chunk checksums and hashes the result to produce
/// a single verification checksum for the entire file.
///
/// # Arguments
/// * `checksums` - List of individual chunk checksums
///
/// # Returns
/// SHA-256 hash of the concatenated checksums
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

    #[test]
    fn test_calculate_checksum_empty() {
        let data = b"";
        let checksum = calculate_checksum(data);
        // SHA-256 of empty string
        assert_eq!(
            checksum,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
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

        // Verify chunk1 checksum matches expected
        let expected_chunk1_checksum = calculate_checksum(&chunk1);
        assert_eq!(checksums[0].as_ref().unwrap(), &expected_chunk1_checksum);
    }

    #[tokio::test]
    async fn test_calculate_file_checksums_nonexistent() {
        let checksums = calculate_file_checksums(Path::new("/nonexistent/file"), 1000)
            .await
            .unwrap();

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

    #[test]
    fn test_calculate_combined_checksum_deterministic() {
        let checksums = vec!["a".to_string(), "b".to_string()];
        let combined1 = calculate_combined_checksum(&checksums);
        let combined2 = calculate_combined_checksum(&checksums);
        assert_eq!(combined1, combined2);
    }
}
