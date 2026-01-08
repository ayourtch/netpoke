use serde::{Deserialize, Serialize};
use netpoke_auth::AuthConfig;

// Re-export iperf3 config for convenience
pub use iperf3_server::Iperf3Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub security: SecurityConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub capture: CaptureConfig,
    #[serde(default)]
    pub tracing: TracingConfig,
    #[serde(default)]
    pub client: ClientConfig,
    #[serde(default)]
    pub iperf3: Iperf3Config,
}

/// Tracing buffer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingConfig {
    /// Enable tracing to circular buffer
    #[serde(default)]
    pub enabled: bool,
    /// Maximum number of log entries to store in the ring buffer
    #[serde(default = "default_max_log_entries")]
    pub max_log_entries: usize,
}

fn default_max_log_entries() -> usize {
    10000
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_log_entries: default_max_log_entries(),
        }
    }
}

/// Client-side configuration settings
/// These settings are exposed to the client via the /api/config/client endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    /// Delay in milliseconds between WebRTC connection establishment attempts
    /// This helps space out connection attempts to reduce network congestion
    #[serde(default = "default_webrtc_connection_delay_ms")]
    pub webrtc_connection_delay_ms: u32,
}

fn default_webrtc_connection_delay_ms() -> u32 {
    50
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            webrtc_connection_delay_ms: default_webrtc_connection_delay_ms(),
        }
    }
}

/// Packet capture configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureConfig {
    /// Enable packet capture
    #[serde(default)]
    pub enabled: bool,
    /// Maximum number of packets to store in the ring buffer
    #[serde(default = "default_max_packets")]
    pub max_packets: usize,
    /// Maximum bytes per packet to capture (packets larger than this are truncated)
    #[serde(default = "default_snaplen")]
    pub snaplen: i32,
    /// Network interface to capture on (empty string means first available, "any" for all)
    #[serde(default)]
    pub interface: String,
    /// Enable promiscuous mode
    #[serde(default = "default_promiscuous")]
    pub promiscuous: bool,
}

fn default_max_packets() -> usize {
    10000
}

fn default_snaplen() -> i32 {
    65535
}

fn default_promiscuous() -> bool {
    true
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_packets: default_max_packets(),
            snaplen: default_snaplen(),
            interface: String::new(),
            promiscuous: default_promiscuous(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub http_port: u16,
    pub https_port: u16,
    pub enable_http: bool,
    pub enable_https: bool,
    pub ssl_cert_path: Option<String>,
    pub ssl_key_path: Option<String>,
    pub domain: Option<String>,
    #[serde(default)]
    pub auto_https: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    /// Optional filter directive for fine-grained log level control.
    /// Uses tracing-subscriber's EnvFilter syntax (e.g., "debug", "server=trace,auth=debug").
    /// When specified, this takes precedence over the `level` setting.
    #[serde(default)]
    pub filter: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    #[serde(default = "default_enable_cors")]
    pub enable_cors: bool,
    #[serde(default)]
    pub allowed_origins: Vec<String>,
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_enable_cors() -> bool {
    true
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            filter: None,
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_cors: default_enable_cors(),
            allowed_origins: vec![],
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            http_port: 3000,
            https_port: 3443,
            enable_http: true,
            enable_https: false,
            ssl_cert_path: None,
            ssl_key_path: None,
            domain: None,
            auto_https: false,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            logging: LoggingConfig::default(),
            security: SecurityConfig::default(),
            auth: AuthConfig::default(),
            capture: CaptureConfig::default(),
            tracing: TracingConfig::default(),
            client: ClientConfig::default(),
            iperf3: Iperf3Config::default(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self, config::ConfigError> {
        let config = config::Config::builder()
            .add_source(config::File::with_name("/etc/netpoke/server_config").required(false))
            .add_source(config::File::with_name("server_config").required(false))
            .add_source(config::Environment::with_prefix("NETPOKE").separator("__"))
            .build()?;

        config.try_deserialize()
    }

    pub fn load_or_default() -> Self {
        Self::load().unwrap_or_else(|e| {
            eprintln!(
                "Warning: Failed to load config file: {}. Using defaults.",
                e
            );
            Self::default()
        })
    }
}
