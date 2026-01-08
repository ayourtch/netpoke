use serde::{Deserialize, Serialize};

/// Authentication provider types
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuthProvider {
    Bluesky,
    GitHub,
    Google,
    LinkedIn,
    PlainLogin, // For future username/password auth
}

/// Session data stored for authenticated users (stored in encrypted private cookie)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionData {
    /// Authentication provider used
    pub auth_provider: AuthProvider,

    /// User's unique identifier (DID for Bluesky, provider-specific ID for others)
    pub user_id: String,

    /// User's handle/username
    pub handle: String,

    /// Display name
    pub display_name: Option<String>,

    /// User's groups (for authorization)
    #[serde(default)]
    pub groups: Vec<String>,

    /// Session creation timestamp (Unix timestamp)
    pub created_at: u64,
}

/// Temporary OAuth state stored during authentication flow
/// This is stored server-side in memory (not in cookies) because:
/// 1. It's only needed during the brief OAuth flow
/// 2. It contains sensitive PKCE verifier
/// 3. It's cleared after token exchange
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OAuthTempState {
    /// Authentication provider used
    pub auth_provider: AuthProvider,

    /// User's handle/username (for Bluesky)
    pub handle: Option<String>,

    /// User's DID (for Bluesky - resolved from handle)
    pub user_did: Option<String>,

    /// PKCE verifier (temporary, cleared after token exchange)
    pub pkce_verifier: Option<String>,

    /// OAuth endpoints (for Bluesky dynamic discovery)
    pub oauth_endpoints: Option<OAuthEndpoints>,

    /// DPoP private key (for Bluesky)
    pub dpop_private_key: Option<String>,

    /// State creation timestamp (Unix timestamp)
    pub created_at: u64,
}

/// OAuth endpoints (primarily for Bluesky dynamic discovery)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OAuthEndpoints {
    pub auth_url: String,
    pub token_url: String,
    pub service_endpoint: String,
}

impl SessionData {
    pub fn is_expired(&self, timeout_seconds: u64) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        now - self.created_at > timeout_seconds
    }
}

impl OAuthTempState {
    pub fn is_expired(&self, timeout_seconds: u64) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        now - self.created_at > timeout_seconds
    }
}
