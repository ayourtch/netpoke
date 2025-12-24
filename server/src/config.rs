use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub security: SecurityConfig,
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
        }
    }
}

impl Config {
    pub fn load() -> Result<Self, config::ConfigError> {
        let config = config::Config::builder()
            .add_source(config::File::with_name("server_config").required(false))
            .add_source(config::Environment::with_prefix("WIFI_VERIFY").separator("__"))
            .build()?;

        config.try_deserialize()
    }

    pub fn load_or_default() -> Self {
        Self::load().unwrap_or_else(|e| {
            eprintln!("Warning: Failed to load config file: {}. Using defaults.", e);
            Self::default()
        })
    }
}