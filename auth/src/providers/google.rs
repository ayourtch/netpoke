use crate::error::AuthError;
use crate::session::{SessionData, OAuthTempState, AuthProvider};
use crate::config::OAuthConfig;
use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    PkceCodeChallenge, RedirectUrl, Scope, TokenUrl, TokenResponse,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct GoogleUserInfo {
    sub: String,
    name: Option<String>,
    email: Option<String>,
}

pub struct GoogleProvider {
    client_id: String,
    client_secret: String,
    redirect_url: String,
}

impl GoogleProvider {
    pub fn new(config: &OAuthConfig) -> Result<Self, AuthError> {
        let client_id = config.google_client_id.as_ref()
            .ok_or_else(|| AuthError::ConfigError("Google client_id not configured".to_string()))?
            .clone();
        let client_secret = config.google_client_secret.as_ref()
            .ok_or_else(|| AuthError::ConfigError("Google client_secret not configured".to_string()))?
            .clone();
        let redirect_url = config.google_redirect_url.as_ref()
            .ok_or_else(|| AuthError::ConfigError("Google redirect_url not configured".to_string()))?
            .clone();
        
        Ok(Self {
            client_id,
            client_secret,
            redirect_url,
        })
    }
    
    pub async fn start_auth(&self) -> Result<(String, OAuthTempState), AuthError> {
        let google_client = BasicClient::new(
            ClientId::new(self.client_id.clone()),
            None,
            AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string()).unwrap(),
            Some(TokenUrl::new("https://oauth2.googleapis.com/token".to_string()).unwrap()),
        )
        .set_redirect_uri(RedirectUrl::new(self.redirect_url.clone()).unwrap());
        
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
        let csrf_state = CsrfToken::new_random();
        
        let (auth_url, _) = google_client
            .authorize_url(|| csrf_state.clone())
            .add_scope(Scope::new("openid".to_string()))
            .add_scope(Scope::new("profile".to_string()))
            .add_scope(Scope::new("email".to_string()))
            .set_pkce_challenge(pkce_challenge)
            .url();
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let temp_state = OAuthTempState {
            auth_provider: AuthProvider::Google,
            handle: None,
            access_token: None,
            pkce_verifier: Some(pkce_verifier.secret().clone()),
            oauth_endpoints: None,
            dpop_private_key: None,
            created_at: now,
        };
        
        Ok((auth_url.to_string(), temp_state))
    }
    
    pub async fn complete_auth(
        &self,
        code: &str,
        temp_state: &OAuthTempState,
    ) -> Result<SessionData, AuthError> {
        let google_client = BasicClient::new(
            ClientId::new(self.client_id.clone()),
            Some(ClientSecret::new(self.client_secret.clone())),
            AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string()).unwrap(),
            Some(TokenUrl::new("https://oauth2.googleapis.com/token".to_string()).unwrap()),
        )
        .set_redirect_uri(RedirectUrl::new(self.redirect_url.clone()).unwrap());
        
        let code = AuthorizationCode::new(code.to_string());
        let pkce_verifier = temp_state.pkce_verifier.as_ref()
            .map(|v| oauth2::PkceCodeVerifier::new(v.clone()));
        
        let mut token_request = google_client.exchange_code(code);
        if let Some(verifier) = pkce_verifier {
            token_request = token_request.set_pkce_verifier(verifier);
        }
        
        let token_result = token_request
            .request_async(oauth2::reqwest::async_http_client)
            .await
            .map_err(|e| AuthError::OAuthError(format!("Google token exchange failed: {}", e)))?;
        
        let access_token = token_result.access_token().secret().clone();
        
        // Get user info from Google
        let client = reqwest::Client::new();
        let user_info = client
            .get("https://www.googleapis.com/oauth2/v3/userinfo")
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await?
            .json::<GoogleUserInfo>()
            .await?;
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Ok(SessionData {
            auth_provider: AuthProvider::Google,
            user_id: format!("google:{}", user_info.sub),
            handle: user_info.email.unwrap_or_else(|| user_info.sub.clone()),
            display_name: user_info.name,
            groups: vec![],
            created_at: now,
        })
    }
}
