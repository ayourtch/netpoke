//! Error types for the iperf3 server.

use thiserror::Error;

/// Errors that can occur in the iperf3 server
#[derive(Error, Debug)]
pub enum Iperf3Error {
    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON parsing error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Protocol error
    #[error("Protocol error: {0}")]
    Protocol(String),

    /// Authentication error
    #[error("Authentication error: IP {0} is not allowed")]
    Unauthorized(std::net::IpAddr),

    /// Session limit reached
    #[error("Session limit reached: maximum {0} concurrent sessions")]
    SessionLimitReached(usize),

    /// Invalid parameter
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    /// Test timeout
    #[error("Test timeout after {0} seconds")]
    Timeout(u64),

    /// Server is shutting down
    #[error("Server is shutting down")]
    Shutdown,
}

/// Result type for iperf3 operations
pub type Result<T> = std::result::Result<T, Iperf3Error>;
