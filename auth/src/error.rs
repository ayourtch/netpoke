use thiserror::Error;
use axum::http::StatusCode;

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("DNS resolution failed: {0}")]
    DnsError(String),
    
    #[error("DID resolution failed: {0}")]
    DidResolutionError(String),
    
    #[error("Service metadata error: {0}")]
    ServiceMetadataError(String),
    
    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),
    
    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),
    
    #[error("URL parsing error: {0}")]
    UrlError(#[from] url::ParseError),
    
    #[error("Invalid handle format")]
    InvalidHandleFormat,
    
    #[error("Authentication required")]
    AuthenticationRequired,
    
    #[error("Session not found")]
    SessionNotFound,
    
    #[error("Session expired")]
    SessionExpired,
    
    #[error("Invalid session")]
    InvalidSession,
    
    #[error("OAuth error: {0}")]
    OAuthError(String),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("Access denied: User not in allowed list")]
    AccessDenied,
}

impl From<AuthError> for StatusCode {
    fn from(error: AuthError) -> StatusCode {
        match error {
            AuthError::InvalidHandleFormat => StatusCode::BAD_REQUEST,
            AuthError::AuthenticationRequired => StatusCode::UNAUTHORIZED,
            AuthError::AccessDenied => StatusCode::FORBIDDEN,
            AuthError::SessionNotFound | AuthError::SessionExpired | AuthError::InvalidSession => {
                StatusCode::UNAUTHORIZED
            }
            AuthError::DnsError(_)
            | AuthError::DidResolutionError(_)
            | AuthError::ServiceMetadataError(_)
            | AuthError::NetworkError(_)
            | AuthError::JsonError(_)
            | AuthError::UrlError(_)
            | AuthError::OAuthError(_)
            | AuthError::ConfigError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
