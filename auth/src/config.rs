use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Enable authentication globally
    #[serde(default)]
    pub enable_auth: bool,

    /// OAuth2 providers configuration
    #[serde(default)]
    pub oauth: OAuthConfig,

    /// Future: Plain login configuration
    #[serde(default)]
    pub plain_login: PlainLoginConfig,

    /// Session configuration
    #[serde(default)]
    pub session: SessionConfig,

    /// Access control - list of allowed user handles/emails
    /// If empty, all authenticated users are allowed
    /// If not empty, only users in this list can access the application
    #[serde(default)]
    pub allowed_users: Vec<String>,

    /// Magic Key configuration for surveyors
    #[serde(default)]
    pub magic_keys: MagicKeyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    /// Enable Bluesky OAuth
    #[serde(default)]
    pub enable_bluesky: bool,

    /// Enable GitHub OAuth
    #[serde(default)]
    pub enable_github: bool,

    /// Enable Google OAuth
    #[serde(default)]
    pub enable_google: bool,

    /// Enable LinkedIn OAuth
    #[serde(default)]
    pub enable_linkedin: bool,

    /// Bluesky client ID (URL to client-metadata.json)
    pub bluesky_client_id: Option<String>,

    /// Bluesky redirect URL
    pub bluesky_redirect_url: Option<String>,

    /// GitHub OAuth credentials
    pub github_client_id: Option<String>,
    pub github_client_secret: Option<String>,
    pub github_redirect_url: Option<String>,

    /// Google OAuth credentials
    pub google_client_id: Option<String>,
    pub google_client_secret: Option<String>,
    pub google_redirect_url: Option<String>,

    /// LinkedIn OAuth credentials
    pub linkedin_client_id: Option<String>,
    pub linkedin_client_secret: Option<String>,
    pub linkedin_redirect_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlainLoginConfig {
    /// Enable plain login (username/password)
    #[serde(default)]
    pub enabled: bool,

    /// List of allowed users with passwords
    #[serde(default)]
    pub users: Vec<UserCredentials>,
}

/// User credentials for file-based authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCredentials {
    /// Username
    pub username: String,

    /// Password (plain text in config, hashed in memory)
    /// In production, use bcrypt or argon2 hashed passwords
    pub password: String,

    /// Optional display name
    #[serde(default)]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Session cookie name
    #[serde(default = "default_session_cookie_name")]
    pub cookie_name: String,

    /// Session timeout in seconds (default: 24 hours)
    #[serde(default = "default_session_timeout")]
    pub timeout_seconds: u64,

    /// Secure cookie (HTTPS only)
    #[serde(default)]
    pub secure: bool,

    /// Cookie secret for encrypting session data (base64 encoded, 64 bytes)
    /// If not provided, a random secret is generated on startup
    /// WARNING: If not set, sessions will not persist across server restarts
    #[serde(default)]
    pub cookie_secret: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MagicKeyConfig {
    /// Enable Magic Key authentication
    #[serde(default)]
    pub enabled: bool,

    /// List of valid Magic Keys
    #[serde(default)]
    pub magic_keys: Vec<String>,

    /// Survey session cookie name
    #[serde(default = "default_survey_cookie_name")]
    pub survey_cookie_name: String,

    /// Survey session timeout in seconds (default: 8 hours)
    #[serde(default = "default_survey_timeout")]
    pub survey_timeout_seconds: u64,

    /// Default maximum measuring time in seconds for all magic keys (default: 3600 = 1 hour)
    #[serde(default = "default_max_measuring_time")]
    pub max_measuring_time_seconds: u64,

    /// Per-magic-key maximum measuring time overrides in seconds.
    /// Keys not in this map use max_measuring_time_seconds as the default.
    /// Example: { "DEMO" = 120 } limits DEMO key to 120 seconds.
    #[serde(default)]
    pub magic_key_max_measuring_time: HashMap<String, u64>,
}

fn default_survey_cookie_name() -> String {
    "survey_session_id".to_string()
}

fn default_survey_timeout() -> u64 {
    28800 // 8 hours
}

fn default_max_measuring_time() -> u64 {
    3600 // 1 hour
}

fn default_session_cookie_name() -> String {
    "session_id".to_string()
}

fn default_session_timeout() -> u64 {
    86400 // 24 hours
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enable_auth: false,
            oauth: OAuthConfig::default(),
            plain_login: PlainLoginConfig::default(),
            session: SessionConfig::default(),
            allowed_users: vec![],
            magic_keys: MagicKeyConfig::default(),
        }
    }
}

impl Default for OAuthConfig {
    fn default() -> Self {
        Self {
            enable_bluesky: false,
            enable_github: false,
            enable_google: false,
            enable_linkedin: false,
            bluesky_client_id: None,
            bluesky_redirect_url: None,
            github_client_id: None,
            github_client_secret: None,
            github_redirect_url: None,
            google_client_id: None,
            google_client_secret: None,
            google_redirect_url: None,
            linkedin_client_id: None,
            linkedin_client_secret: None,
            linkedin_redirect_url: None,
        }
    }
}

impl Default for PlainLoginConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            users: vec![],
        }
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            cookie_name: default_session_cookie_name(),
            timeout_seconds: default_session_timeout(),
            secure: false,
            cookie_secret: None,
        }
    }
}

impl Default for MagicKeyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            magic_keys: vec![],
            survey_cookie_name: default_survey_cookie_name(),
            survey_timeout_seconds: default_survey_timeout(),
            max_measuring_time_seconds: default_max_measuring_time(),
            magic_key_max_measuring_time: HashMap::new(),
        }
    }
}

impl MagicKeyConfig {
    /// Get the maximum measuring time in seconds for a specific magic key.
    /// Returns the per-key override if configured, otherwise the global default.
    /// For the "DEMO" key, defaults to 120 seconds if no override is configured.
    pub fn get_max_measuring_time_seconds(&self, magic_key: &str) -> u64 {
        if let Some(&override_seconds) = self.magic_key_max_measuring_time.get(magic_key) {
            return override_seconds;
        }
        // Built-in default for DEMO key: 120 seconds
        if magic_key == "DEMO" {
            return 120;
        }
        self.max_measuring_time_seconds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_magic_key_config() {
        let config = MagicKeyConfig::default();
        assert_eq!(config.max_measuring_time_seconds, 3600);
        assert!(config.magic_key_max_measuring_time.is_empty());
    }

    #[test]
    fn test_demo_key_default_120_seconds() {
        let config = MagicKeyConfig::default();
        assert_eq!(config.get_max_measuring_time_seconds("DEMO"), 120);
    }

    #[test]
    fn test_regular_key_uses_global_default() {
        let config = MagicKeyConfig::default();
        // Regular keys should use the global default (3600)
        assert_eq!(config.get_max_measuring_time_seconds("SURVEY-001"), 3600);
        assert_eq!(config.get_max_measuring_time_seconds("MY-KEY"), 3600);
    }

    #[test]
    fn test_per_key_override() {
        let mut config = MagicKeyConfig::default();
        config
            .magic_key_max_measuring_time
            .insert("SURVEY-001".to_string(), 7200);
        assert_eq!(config.get_max_measuring_time_seconds("SURVEY-001"), 7200);
        // Other keys still use global default
        assert_eq!(config.get_max_measuring_time_seconds("SURVEY-002"), 3600);
    }

    #[test]
    fn test_demo_key_override() {
        let mut config = MagicKeyConfig::default();
        // Override the DEMO key's built-in default
        config
            .magic_key_max_measuring_time
            .insert("DEMO".to_string(), 60);
        assert_eq!(config.get_max_measuring_time_seconds("DEMO"), 60);
    }

    #[test]
    fn test_custom_global_default() {
        let mut config = MagicKeyConfig::default();
        config.max_measuring_time_seconds = 1800;
        assert_eq!(config.get_max_measuring_time_seconds("ANY-KEY"), 1800);
        // DEMO still has its built-in default
        assert_eq!(config.get_max_measuring_time_seconds("DEMO"), 120);
    }
}
