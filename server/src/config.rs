use serde::{Deserialize, Serialize};

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
    pub auto_https: bool,
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

impl ServerConfig {
    pub fn load() -> Result<Self, config::ConfigError> {
        let config = config::Config::builder()
            .add_source(config::File::with_name("server_config"))
            .add_source(config::Environment::with_prefix("WIFI_VERIFY"))
            .build()?;

        config.try_deserialize()
    }
}