use crate::config::AuthConfig;
use crate::error::AuthError;
use crate::providers::{
    BlueskyProvider, GitHubProvider, GoogleProvider, LinkedInProvider, PlainLoginProvider,
};
use crate::session::{OAuthTempState, SessionData};
use axum_extra::extract::cookie::Key;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use trust_dns_resolver::TokioAsyncResolver;

/// Main authentication service
pub struct AuthService {
    pub config: AuthConfig,
    /// Temporary OAuth state storage (PKCE verifiers, etc.) - cleared after token exchange
    /// This is still needed for OAuth flows since we can't store sensitive PKCE data in cookies
    oauth_temp_states: Arc<RwLock<HashMap<String, OAuthTempState>>>,
    /// Cookie encryption key for PrivateCookieJar
    cookie_key: Key,
    bluesky_provider: Option<BlueskyProvider>,
    github_provider: Option<GitHubProvider>,
    google_provider: Option<GoogleProvider>,
    linkedin_provider: Option<LinkedInProvider>,
    plain_login_provider: Option<PlainLoginProvider>,
}

impl AuthService {
    pub async fn new(config: AuthConfig) -> Result<Self, AuthError> {
        let oauth_temp_states = Arc::new(RwLock::new(HashMap::new()));

        // Initialize cookie key from config or generate randomly
        let cookie_key = if let Some(ref secret) = config.session.cookie_secret {
            // Decode base64 secret
            let secret_bytes =
                base64::Engine::decode(&base64::engine::general_purpose::STANDARD, secret)
                    .map_err(|e| {
                        AuthError::ConfigError(format!("Invalid cookie_secret base64: {}", e))
                    })?;

            if secret_bytes.len() < 64 {
                return Err(AuthError::ConfigError(format!(
                    "cookie_secret must be at least 64 bytes when decoded, got {}",
                    secret_bytes.len()
                )));
            }

            Key::try_from(&secret_bytes[..])
                .map_err(|e| AuthError::ConfigError(format!("Invalid cookie_secret: {}", e)))?
        } else {
            tracing::warn!("No cookie_secret configured, generating random key. Sessions will not persist across server restarts.");
            Key::generate()
        };

        // Initialize enabled providers
        let bluesky_provider = if config.oauth.enable_bluesky {
            let resolver = TokioAsyncResolver::tokio_from_system_conf().map_err(|e| {
                AuthError::ConfigError(format!("Failed to create DNS resolver: {}", e))
            })?;
            Some(BlueskyProvider::new(&config.oauth, resolver)?)
        } else {
            None
        };

        let github_provider = if config.oauth.enable_github {
            Some(GitHubProvider::new(&config.oauth)?)
        } else {
            None
        };

        let google_provider = if config.oauth.enable_google {
            Some(GoogleProvider::new(&config.oauth)?)
        } else {
            None
        };

        let linkedin_provider = if config.oauth.enable_linkedin {
            Some(LinkedInProvider::new(&config.oauth)?)
        } else {
            None
        };

        let plain_login_provider = if config.plain_login.enabled {
            Some(PlainLoginProvider::new(&config.plain_login)?)
        } else {
            None
        };

        Ok(Self {
            config,
            oauth_temp_states,
            cookie_key,
            bluesky_provider,
            github_provider,
            google_provider,
            linkedin_provider,
            plain_login_provider,
        })
    }

    /// Get the cookie key for PrivateCookieJar
    pub fn cookie_key(&self) -> Key {
        self.cookie_key.clone()
    }

    /// Check if authentication is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enable_auth
    }

    /// Check if any provider is enabled
    pub fn has_enabled_providers(&self) -> bool {
        self.config.oauth.enable_bluesky
            || self.config.oauth.enable_github
            || self.config.oauth.enable_google
            || self.config.oauth.enable_linkedin
            || self.config.plain_login.enabled
    }

    /// Check if a user is in the allowed users list
    /// Returns true if the allowed_users list is empty (all users allowed)
    /// or if the user's handle is in the list
    pub fn is_user_allowed(&self, user_handle: &str) -> bool {
        if self.config.allowed_users.is_empty() {
            // If no allowed users configured, allow all authenticated users
            true
        } else {
            // Check if user is in the allowed list
            self.config
                .allowed_users
                .iter()
                .any(|allowed| allowed == user_handle)
        }
    }

    /// Start Bluesky authentication
    pub async fn start_bluesky_auth(
        &self,
        handle: &str,
    ) -> Result<(String, OAuthTempState), AuthError> {
        let provider = self
            .bluesky_provider
            .as_ref()
            .ok_or_else(|| AuthError::ConfigError("Bluesky provider not enabled".to_string()))?;
        provider.start_auth(handle).await
    }

    /// Complete Bluesky authentication
    pub async fn complete_bluesky_auth(
        &self,
        code: &str,
        temp_state: &OAuthTempState,
    ) -> Result<SessionData, AuthError> {
        let provider = self
            .bluesky_provider
            .as_ref()
            .ok_or_else(|| AuthError::ConfigError("Bluesky provider not enabled".to_string()))?;
        provider.complete_auth(code, temp_state).await
    }

    /// Start GitHub authentication
    pub async fn start_github_auth(&self) -> Result<(String, OAuthTempState), AuthError> {
        let provider = self
            .github_provider
            .as_ref()
            .ok_or_else(|| AuthError::ConfigError("GitHub provider not enabled".to_string()))?;
        provider.start_auth().await
    }

    /// Complete GitHub authentication
    pub async fn complete_github_auth(
        &self,
        code: &str,
        temp_state: &OAuthTempState,
    ) -> Result<SessionData, AuthError> {
        let provider = self
            .github_provider
            .as_ref()
            .ok_or_else(|| AuthError::ConfigError("GitHub provider not enabled".to_string()))?;
        provider.complete_auth(code, temp_state).await
    }

    /// Start Google authentication
    pub async fn start_google_auth(&self) -> Result<(String, OAuthTempState), AuthError> {
        let provider = self
            .google_provider
            .as_ref()
            .ok_or_else(|| AuthError::ConfigError("Google provider not enabled".to_string()))?;
        provider.start_auth().await
    }

    /// Complete Google authentication
    pub async fn complete_google_auth(
        &self,
        code: &str,
        temp_state: &OAuthTempState,
    ) -> Result<SessionData, AuthError> {
        let provider = self
            .google_provider
            .as_ref()
            .ok_or_else(|| AuthError::ConfigError("Google provider not enabled".to_string()))?;
        provider.complete_auth(code, temp_state).await
    }

    /// Start LinkedIn authentication
    pub async fn start_linkedin_auth(&self) -> Result<(String, OAuthTempState), AuthError> {
        let provider = self
            .linkedin_provider
            .as_ref()
            .ok_or_else(|| AuthError::ConfigError("LinkedIn provider not enabled".to_string()))?;
        provider.start_auth().await
    }

    /// Complete LinkedIn authentication
    pub async fn complete_linkedin_auth(
        &self,
        code: &str,
        temp_state: &OAuthTempState,
    ) -> Result<SessionData, AuthError> {
        let provider = self
            .linkedin_provider
            .as_ref()
            .ok_or_else(|| AuthError::ConfigError("LinkedIn provider not enabled".to_string()))?;
        provider.complete_auth(code, temp_state).await
    }

    /// Authenticate with plain login (username/password)
    pub async fn authenticate_plain_login(
        &self,
        username: &str,
        password: &str,
    ) -> Result<SessionData, AuthError> {
        let provider = self.plain_login_provider.as_ref().ok_or_else(|| {
            AuthError::ConfigError("Plain login provider not enabled".to_string())
        })?;
        provider.authenticate(username, password).await
    }

    /// Store temporary OAuth state (PKCE verifier, etc.)
    pub async fn store_oauth_temp_state(&self, state_id: String, temp_state: OAuthTempState) {
        let mut states = self.oauth_temp_states.write().await;
        states.insert(state_id, temp_state);
    }

    /// Get temporary OAuth state
    pub async fn get_oauth_temp_state(&self, state_id: &str) -> Option<OAuthTempState> {
        let states = self.oauth_temp_states.read().await;
        states.get(state_id).cloned()
    }

    /// Remove temporary OAuth state (after token exchange)
    pub async fn remove_oauth_temp_state(&self, state_id: &str) {
        let mut states = self.oauth_temp_states.write().await;
        states.remove(state_id);
    }

    /// Validate session data from cookie (check expiration)
    pub fn validate_session(&self, session_data: &SessionData) -> Result<(), AuthError> {
        if session_data.is_expired(self.config.session.timeout_seconds) {
            return Err(AuthError::SessionExpired);
        }
        Ok(())
    }

    /// Clean up expired OAuth temp states (called periodically or on access)
    pub async fn cleanup_expired_oauth_states(&self) {
        let mut states = self.oauth_temp_states.write().await;
        // OAuth temp states expire after 10 minutes (should complete auth flow by then)
        let timeout = 600u64;
        states.retain(|_, state| !state.is_expired(timeout));
    }
}
