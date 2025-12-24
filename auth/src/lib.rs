//! WiFi Verify Authentication Library
//!
//! A reusable authentication crate for WiFi Verify that supports multiple OAuth2 providers
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
//! use wifi_verify_auth::{AuthConfig, AuthService};
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() {
//!     let config = AuthConfig::default();
//!     let auth_service = Arc::new(AuthService::new(config).await.unwrap());
//!     // Use auth_service in your application
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

// Re-export commonly used types
pub use config::AuthConfig;
pub use error::AuthError;
pub use middleware::{optional_auth, require_auth};
pub use routes::auth_routes;
pub use service::AuthService;
pub use session::{AuthProvider, SessionData};
