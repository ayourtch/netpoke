use serde::{Deserialize, Serialize};

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
        }
    }
}
