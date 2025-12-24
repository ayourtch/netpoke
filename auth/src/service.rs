use crate::config::AuthConfig;
use crate::error::AuthError;
use crate::session::SessionData;
use crate::providers::{BlueskyProvider, GitHubProvider, GoogleProvider, LinkedInProvider, PlainLoginProvider};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use trust_dns_resolver::TokioAsyncResolver;

/// Main authentication service
pub struct AuthService {
    pub config: AuthConfig,
    pub sessions: Arc<RwLock<HashMap<String, SessionData>>>,
    bluesky_provider: Option<BlueskyProvider>,
    github_provider: Option<GitHubProvider>,
    google_provider: Option<GoogleProvider>,
    linkedin_provider: Option<LinkedInProvider>,
    plain_login_provider: Option<PlainLoginProvider>,
}

impl AuthService {
    pub async fn new(config: AuthConfig) -> Result<Self, AuthError> {
        let sessions = Arc::new(RwLock::new(HashMap::new()));
        
        // Initialize enabled providers
        let bluesky_provider = if config.oauth.enable_bluesky {
            let resolver = TokioAsyncResolver::tokio_from_system_conf()
                .map_err(|e| AuthError::ConfigError(format!("Failed to create DNS resolver: {}", e)))?;
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
            sessions,
            bluesky_provider,
            github_provider,
            google_provider,
            linkedin_provider,
            plain_login_provider,
        })
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
            self.config.allowed_users.iter().any(|allowed| allowed == user_handle)
        }
    }
    
    /// Start Bluesky authentication
    pub async fn start_bluesky_auth(&self, handle: &str) -> Result<(String, SessionData), AuthError> {
        let provider = self.bluesky_provider.as_ref()
            .ok_or_else(|| AuthError::ConfigError("Bluesky provider not enabled".to_string()))?;
        provider.start_auth(handle).await
    }
    
    /// Complete Bluesky authentication
    pub async fn complete_bluesky_auth(
        &self,
        code: &str,
        session_data: &SessionData,
    ) -> Result<SessionData, AuthError> {
        let provider = self.bluesky_provider.as_ref()
            .ok_or_else(|| AuthError::ConfigError("Bluesky provider not enabled".to_string()))?;
        provider.complete_auth(code, session_data).await
    }
    
    /// Start GitHub authentication
    pub async fn start_github_auth(&self) -> Result<(String, SessionData), AuthError> {
        let provider = self.github_provider.as_ref()
            .ok_or_else(|| AuthError::ConfigError("GitHub provider not enabled".to_string()))?;
        provider.start_auth().await
    }
    
    /// Complete GitHub authentication
    pub async fn complete_github_auth(
        &self,
        code: &str,
        session_data: &SessionData,
    ) -> Result<SessionData, AuthError> {
        let provider = self.github_provider.as_ref()
            .ok_or_else(|| AuthError::ConfigError("GitHub provider not enabled".to_string()))?;
        provider.complete_auth(code, session_data).await
    }
    
    /// Start Google authentication
    pub async fn start_google_auth(&self) -> Result<(String, SessionData), AuthError> {
        let provider = self.google_provider.as_ref()
            .ok_or_else(|| AuthError::ConfigError("Google provider not enabled".to_string()))?;
        provider.start_auth().await
    }
    
    /// Complete Google authentication
    pub async fn complete_google_auth(
        &self,
        code: &str,
        session_data: &SessionData,
    ) -> Result<SessionData, AuthError> {
        let provider = self.google_provider.as_ref()
            .ok_or_else(|| AuthError::ConfigError("Google provider not enabled".to_string()))?;
        provider.complete_auth(code, session_data).await
    }
    
    /// Start LinkedIn authentication
    pub async fn start_linkedin_auth(&self) -> Result<(String, SessionData), AuthError> {
        let provider = self.linkedin_provider.as_ref()
            .ok_or_else(|| AuthError::ConfigError("LinkedIn provider not enabled".to_string()))?;
        provider.start_auth().await
    }
    
    /// Complete LinkedIn authentication
    pub async fn complete_linkedin_auth(
        &self,
        code: &str,
        session_data: &SessionData,
    ) -> Result<SessionData, AuthError> {
        let provider = self.linkedin_provider.as_ref()
            .ok_or_else(|| AuthError::ConfigError("LinkedIn provider not enabled".to_string()))?;
        provider.complete_auth(code, session_data).await
    }
    
    /// Authenticate with plain login (username/password)
    pub async fn authenticate_plain_login(
        &self,
        username: &str,
        password: &str,
    ) -> Result<SessionData, AuthError> {
        let provider = self.plain_login_provider.as_ref()
            .ok_or_else(|| AuthError::ConfigError("Plain login provider not enabled".to_string()))?;
        provider.authenticate(username, password).await
    }
    
    /// Store session data
    pub async fn store_session(&self, session_id: String, session_data: SessionData) {
        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id, session_data);
    }
    
    /// Get session data
    pub async fn get_session(&self, session_id: &str) -> Option<SessionData> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned()
    }
    
    /// Remove session (logout)
    pub async fn remove_session(&self, session_id: &str) {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id);
    }
    
    /// Validate session and check expiration
    pub async fn validate_session(&self, session_id: &str) -> Result<SessionData, AuthError> {
        let session_data = self.get_session(session_id).await
            .ok_or(AuthError::SessionNotFound)?;
        
        if session_data.is_expired(self.config.session.timeout_seconds) {
            self.remove_session(session_id).await;
            return Err(AuthError::SessionExpired);
        }
        
        Ok(session_data)
    }
    
    /// Clean up expired sessions
    pub async fn cleanup_expired_sessions(&self) {
        let mut sessions = self.sessions.write().await;
        let timeout = self.config.session.timeout_seconds;
        sessions.retain(|_, session| !session.is_expired(timeout));
    }
}
