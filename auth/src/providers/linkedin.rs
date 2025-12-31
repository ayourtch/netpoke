use crate::error::AuthError;
use crate::session::{SessionData, OAuthTempState, AuthProvider};
use crate::config::OAuthConfig;
use oauth2::{
    basic::BasicClient, AuthUrl, ClientId, CsrfToken,
    RedirectUrl, Scope, TokenUrl,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct LinkedInUserInfo {
    sub: String,
    name: Option<String>,
    email: Option<String>,
}

pub struct LinkedInProvider {
    client_id: String,
    client_secret: String,
    redirect_url: String,
}

impl LinkedInProvider {
    pub fn new(config: &OAuthConfig) -> Result<Self, AuthError> {
        let client_id = config.linkedin_client_id.as_ref()
            .ok_or_else(|| AuthError::ConfigError("LinkedIn client_id not configured".to_string()))?
            .clone();
        let client_secret = config.linkedin_client_secret.as_ref()
            .ok_or_else(|| AuthError::ConfigError("LinkedIn client_secret not configured".to_string()))?
            .clone();
        let redirect_url = config.linkedin_redirect_url.as_ref()
            .ok_or_else(|| AuthError::ConfigError("LinkedIn redirect_url not configured".to_string()))?
            .clone();
        
        Ok(Self {
            client_id,
            client_secret,
            redirect_url,
        })
    }
    
    pub async fn start_auth(&self) -> Result<(String, OAuthTempState), AuthError> {
        let linkedin_client = BasicClient::new(
            ClientId::new(self.client_id.clone()),
            None,
            AuthUrl::new("https://www.linkedin.com/oauth/v2/authorization".to_string()).unwrap(),
            Some(TokenUrl::new("https://www.linkedin.com/oauth/v2/accessToken".to_string()).unwrap()),
        )
        .set_redirect_uri(RedirectUrl::new(self.redirect_url.clone()).unwrap());
        
        let csrf_state = CsrfToken::new_random();
        
        let (auth_url, _) = linkedin_client
            .authorize_url(|| csrf_state.clone())
            .add_scope(Scope::new("openid".to_string()))
            .add_scope(Scope::new("profile".to_string()))
            .add_scope(Scope::new("email".to_string()))
            .url();
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let temp_state = OAuthTempState {
            auth_provider: AuthProvider::LinkedIn,
            handle: None,
            user_did: None,
            pkce_verifier: None,
            oauth_endpoints: None,
            dpop_private_key: None,
            created_at: now,
        };
        
        Ok((auth_url.to_string(), temp_state))
    }
    
    pub async fn complete_auth(
        &self,
        code: &str,
        _temp_state: &OAuthTempState,
    ) -> Result<SessionData, AuthError> {
        // LinkedIn requires manual token exchange
        let token_params = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", self.redirect_url.as_str()),
            ("client_id", self.client_id.as_str()),
            ("client_secret", self.client_secret.as_str()),
        ];
        
        let client = reqwest::Client::new();
        let token_response = client
            .post("https://www.linkedin.com/oauth/v2/accessToken")
            .form(&token_params)
            .send()
            .await?;
        
        let token_status = token_response.status();
        let token_text = token_response.text().await?;
        
        if !token_status.is_success() {
            return Err(AuthError::OAuthError(format!(
                "LinkedIn token exchange failed: {}",
                token_text
            )));
        }
        
        let token_data: serde_json::Value = serde_json::from_str(&token_text)?;
        let access_token = token_data
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AuthError::OAuthError("No access_token in response".to_string()))?
            .to_string();
        
        // Get user info from LinkedIn
        let user_info = client
            .get("https://api.linkedin.com/v2/userinfo")
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await?
            .json::<LinkedInUserInfo>()
            .await?;
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Ok(SessionData {
            auth_provider: AuthProvider::LinkedIn,
            user_id: format!("linkedin:{}", user_info.sub),
            handle: user_info.email.clone().unwrap_or_else(|| user_info.sub.clone()),
            display_name: user_info.name,
            groups: vec![],
            created_at: now,
        })
    }
}
