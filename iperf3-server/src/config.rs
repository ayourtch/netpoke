//! Configuration for the iperf3 server.

use serde::{Deserialize, Serialize};

/// Configuration for the iperf3 server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Iperf3Config {
    /// Whether the iperf3 server is enabled
    #[serde(default)]
    pub enabled: bool,

    /// Host/IP address to bind to
    #[serde(default = "default_host")]
    pub host: String,

    /// Control port (default: 5201)
    #[serde(default = "default_port")]
    pub port: u16,

    /// Maximum number of concurrent test sessions
    #[serde(default = "default_max_sessions")]
    pub max_sessions: usize,

    /// Maximum test duration in seconds (0 = unlimited)
    #[serde(default = "default_max_duration")]
    pub max_duration_secs: u64,

    /// Whether to require IP-based authentication
    /// When true, only IPs in the allowed list can connect
    #[serde(default)]
    pub require_auth: bool,

    /// Maximum bandwidth per stream in bits/second (0 = unlimited)
    #[serde(default)]
    pub max_bandwidth: u64,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    5201
}

fn default_max_sessions() -> usize {
    10
}

fn default_max_duration() -> u64 {
    3600 // 1 hour max
}

impl Default for Iperf3Config {
    fn default() -> Self {
        Self {
            enabled: false,
            host: default_host(),
            port: default_port(),
            max_sessions: default_max_sessions(),
            max_duration_secs: default_max_duration(),
            require_auth: false,
            max_bandwidth: 0,
        }
    }
}
