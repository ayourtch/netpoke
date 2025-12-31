use crate::error::AuthError;
use crate::session::{SessionData, AuthProvider};
use crate::config::{PlainLoginConfig, UserCredentials};
use bcrypt;
use std::collections::HashMap;

/// Plain login (username/password) provider
pub struct PlainLoginProvider {
    users: HashMap<String, UserCredentials>,
}

impl PlainLoginProvider {
    pub fn new(config: &PlainLoginConfig) -> Result<Self, AuthError> {
        if config.users.is_empty() {
            return Err(AuthError::ConfigError(
                "Plain login enabled but no users configured".to_string()
            ));
        }
        
        let mut users = HashMap::new();
        for user in &config.users {
            users.insert(user.username.clone(), user.clone());
        }
        
        Ok(Self { users })
    }
    
    /// Authenticate a user with username and password
    pub async fn authenticate(
        &self,
        username: &str,
        password: &str,
    ) -> Result<SessionData, AuthError> {
        let user = self.users.get(username)
            .ok_or_else(|| AuthError::OAuthError("Invalid username or password".to_string()))?;
        
        // Check if password is bcrypt hashed (starts with $2)
        let password_valid = if user.password.starts_with("$2") {
            // Already hashed - verify with bcrypt
            bcrypt::verify(password, &user.password)
                .map_err(|e| AuthError::OAuthError(format!("Password verification failed: {}", e)))?
        } else {
            // Plain text password - direct comparison (not recommended for production)
            tracing::warn!(
                "Plain text password used for user '{}'. Consider using bcrypt hashed passwords.",
                username
            );
            password == user.password
        };
        
        if !password_valid {
            return Err(AuthError::OAuthError("Invalid username or password".to_string()));
        }
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Ok(SessionData {
            auth_provider: AuthProvider::PlainLogin,
            user_id: format!("local:{}", username),
            handle: username.to_string(),
            display_name: user.display_name.clone(),
            groups: vec![],
            created_at: now,
        })
    }
    
    /// Check if a username exists
    pub fn user_exists(&self, username: &str) -> bool {
        self.users.contains_key(username)
    }
}

/// Helper function to hash a password with bcrypt
pub fn hash_password(password: &str) -> Result<String, AuthError> {
    bcrypt::hash(password, bcrypt::DEFAULT_COST)
        .map_err(|e| AuthError::ConfigError(format!("Failed to hash password: {}", e)))
}
