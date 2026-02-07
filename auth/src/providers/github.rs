use crate::config::OAuthConfig;
use crate::error::AuthError;
use crate::session::{AuthProvider, OAuthTempState, SessionData};
use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    PkceCodeChallenge, RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct GitHubUser {
    login: String,
    name: Option<String>,
    email: Option<String>,
    id: u64,
}

pub struct GitHubProvider {
    client_id: String,
    client_secret: String,
    redirect_url: String,
}

impl GitHubProvider {
    pub fn new(config: &OAuthConfig) -> Result<Self, AuthError> {
        let client_id = config
            .github_client_id
            .as_ref()
            .ok_or_else(|| AuthError::ConfigError("GitHub client_id not configured".to_string()))?
            .clone();
        let client_secret = config
            .github_client_secret
            .as_ref()
            .ok_or_else(|| {
                AuthError::ConfigError("GitHub client_secret not configured".to_string())
            })?
            .clone();
        let redirect_url = config
            .github_redirect_url
            .as_ref()
            .ok_or_else(|| {
                AuthError::ConfigError("GitHub redirect_url not configured".to_string())
            })?
            .clone();

        Ok(Self {
            client_id,
            client_secret,
            redirect_url,
        })
    }

    pub async fn start_auth(&self) -> Result<(String, OAuthTempState), AuthError> {
        let github_client = BasicClient::new(
            ClientId::new(self.client_id.clone()),
            None,
            AuthUrl::new("https://github.com/login/oauth/authorize".to_string()).unwrap(),
            Some(TokenUrl::new("https://github.com/login/oauth/access_token".to_string()).unwrap()),
        )
        .set_redirect_uri(RedirectUrl::new(self.redirect_url.clone()).unwrap());

        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
        let csrf_state = CsrfToken::new_random();

        let (auth_url, _) = github_client
            .authorize_url(|| csrf_state.clone())
            .add_scope(Scope::new("read:user".to_string()))
            .add_scope(Scope::new("user:email".to_string()))
            .set_pkce_challenge(pkce_challenge)
            .url();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let temp_state = OAuthTempState {
            auth_provider: AuthProvider::GitHub,
            handle: None,
            user_did: None,
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
        let github_client = BasicClient::new(
            ClientId::new(self.client_id.clone()),
            Some(ClientSecret::new(self.client_secret.clone())),
            AuthUrl::new("https://github.com/login/oauth/authorize".to_string()).unwrap(),
            Some(TokenUrl::new("https://github.com/login/oauth/access_token".to_string()).unwrap()),
        )
        .set_redirect_uri(RedirectUrl::new(self.redirect_url.clone()).unwrap());

        let code = AuthorizationCode::new(code.to_string());
        let pkce_verifier = temp_state
            .pkce_verifier
            .as_ref()
            .map(|v| oauth2::PkceCodeVerifier::new(v.clone()));

        let mut token_request = github_client.exchange_code(code);
        if let Some(verifier) = pkce_verifier {
            token_request = token_request.set_pkce_verifier(verifier);
        }

        let token_result = token_request
            .request_async(oauth2::reqwest::async_http_client)
            .await
            .map_err(|e| AuthError::OAuthError(format!("GitHub token exchange failed: {}", e)))?;

        let access_token = token_result.access_token().secret().clone();

        // Get user info from GitHub
        let client = reqwest::Client::new();
        let user_info = client
            .get("https://api.github.com/user")
            .header("Authorization", format!("Bearer {}", access_token))
            .header("User-Agent", "NetPoke-Auth")
            .send()
            .await?
            .json::<GitHubUser>()
            .await?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Ok(SessionData {
            auth_provider: AuthProvider::GitHub,
            user_id: format!("github:{}", user_info.id),
            handle: user_info.login,
            display_name: user_info.name,
            groups: vec![],
            created_at: now,
        })
    }
}
