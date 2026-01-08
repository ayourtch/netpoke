//! Project Raindrops Authentication Library
//!
//! A reusable authentication crate for Project Raindrops that supports multiple OAuth2 providers
//! (Bluesky, GitHub, Google, LinkedIn) and is designed to be extensible for future
//! authentication methods like plain login.
//!
//! # Features
//!
//! - OAuth2 authentication with multiple providers
//! - Session management with configurable timeouts
//! - Professional login page with "Project Raindrops" branding
//! - Middleware for protecting routes
//! - Configurable via TOML configuration files
//! - Designed for easy portability to other projects
//!
//! # Example
//!
//! ```no_run
//! use netpoke_auth::{AuthConfig, AuthService, AuthState};
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() {
//!     let config = AuthConfig::default();
//!     let auth_service = Arc::new(AuthService::new(config).await.unwrap());
//!     let auth_state = AuthState::new(auth_service);
//!     // Use auth_state in your application
//! }
//! ```

pub mod config;
pub mod error;
pub mod middleware;
pub mod providers;
pub mod routes;
pub mod service;
pub mod session;
pub mod views;

use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use std::ops::Deref;
use std::sync::Arc;

// Re-export commonly used types
pub use config::AuthConfig;
pub use error::AuthError;
pub use middleware::{optional_auth, require_auth};
pub use routes::auth_routes;
pub use service::AuthService;
pub use session::{AuthProvider, OAuthTempState, SessionData};

/// State wrapper for AuthService that implements FromRef for Key
/// This allows PrivateCookieJar to extract the cookie key from state
#[derive(Clone)]
pub struct AuthState {
    inner: Arc<AuthService>,
}

impl AuthState {
    pub fn new(auth_service: Arc<AuthService>) -> Self {
        Self {
            inner: auth_service,
        }
    }

    /// Get the inner Arc<AuthService>
    pub fn into_inner(self) -> Arc<AuthService> {
        self.inner
    }
}

impl Deref for AuthState {
    type Target = AuthService;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl AsRef<AuthService> for AuthState {
    fn as_ref(&self) -> &AuthService {
        &self.inner
    }
}

impl From<Arc<AuthService>> for AuthState {
    fn from(service: Arc<AuthService>) -> Self {
        Self::new(service)
    }
}

/// Implement FromRef to allow PrivateCookieJar to extract Key from AuthState
impl FromRef<AuthState> for Key {
    fn from_ref(state: &AuthState) -> Self {
        state.cookie_key()
    }
}
