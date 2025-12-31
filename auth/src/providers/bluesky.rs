use crate::error::AuthError;
use crate::session::{OAuthEndpoints, SessionData, OAuthTempState, AuthProvider};
use crate::config::OAuthConfig;
use oauth2::{
    basic::BasicClient, AuthUrl, ClientId, CsrfToken,
    PkceCodeChallenge, RedirectUrl, Scope, TokenUrl,
};
use serde::Deserialize;
use trust_dns_resolver::TokioAsyncResolver;
use url::Url;
use uuid::Uuid;
use p256::ecdsa::SigningKey;
use p256::elliptic_curve::sec1::ToEncodedPoint;
use sha2::{Sha256, Digest};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

// DID document structures
#[derive(Debug, Deserialize)]
struct DidDocument {
    service: Option<Vec<DidService>>,
}

#[derive(Debug, Deserialize)]
struct DidService {
    id: String,
    #[serde(rename = "type")]
    service_type: String,
    #[serde(rename = "serviceEndpoint")]
    service_endpoint: String,
}

// OAuth protected resource metadata
#[derive(Debug, Deserialize)]
struct ProtectedResourceMetadata {
    #[serde(rename = "authorization_servers")]
    authorization_servers: Vec<String>,
}

// OAuth authorization server metadata
#[derive(Debug, Deserialize)]
struct AuthorizationServerMetadata {
    #[serde(rename = "authorization_endpoint")]
    authorization_endpoint: String,
    #[serde(rename = "token_endpoint")]
    token_endpoint: String,
}

/// Bluesky OAuth provider implementation
pub struct BlueskyProvider {
    client_id: String,
    redirect_url: String,
    dns_resolver: TokioAsyncResolver,
}

impl BlueskyProvider {
    pub fn new(config: &OAuthConfig, dns_resolver: TokioAsyncResolver) -> Result<Self, AuthError> {
        let client_id = config.bluesky_client_id.as_ref()
            .ok_or_else(|| AuthError::ConfigError("Bluesky client_id not configured".to_string()))?
            .clone();
        let redirect_url = config.bluesky_redirect_url.as_ref()
            .ok_or_else(|| AuthError::ConfigError("Bluesky redirect_url not configured".to_string()))?
            .clone();
        
        Ok(Self {
            client_id,
            redirect_url,
            dns_resolver,
        })
    }
    
    /// Start Bluesky OAuth flow
    pub async fn start_auth(&self, handle: &str) -> Result<(String, OAuthTempState), AuthError> {
        tracing::info!("Starting Bluesky OAuth for handle: {}", handle);
        
        // Discover OAuth endpoints dynamically
        let (user_did, oauth_endpoints) = self.discover_oauth_endpoints(handle).await?;
        
        // Create OAuth client
        let oauth_client = BasicClient::new(
            ClientId::new(self.client_id.clone()),
            None, // No client_secret - Bluesky uses PKCE
            AuthUrl::new(oauth_endpoints.auth_url.clone()).unwrap(),
            Some(TokenUrl::new(oauth_endpoints.token_url.clone()).unwrap()),
        )
        .set_redirect_uri(RedirectUrl::new(self.redirect_url.clone()).unwrap());
        
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
        let csrf_state = CsrfToken::new_random();
        
        let (auth_url, _) = oauth_client
            .authorize_url(|| csrf_state.clone())
            .add_scope(Scope::new("atproto".to_string()))
            .add_scope(Scope::new("rpc:app.bsky.actor.getProfile?aud=did:web:api.bsky.app#bsky_appview".to_string()))
            .add_extra_param("code_challenge", pkce_challenge.as_str())
            .add_extra_param("code_challenge_method", "S256")
            .url();
        
        // Generate DPoP key
        let (_signing_key, dpop_private_key) = generate_dpop_key();
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let temp_state = OAuthTempState {
            auth_provider: AuthProvider::Bluesky,
            handle: Some(handle.to_string()),
            user_did: Some(user_did),
            pkce_verifier: Some(pkce_verifier.secret().clone()),
            oauth_endpoints: Some(oauth_endpoints),
            dpop_private_key: Some(dpop_private_key),
            created_at: now,
        };
        
        Ok((auth_url.to_string(), temp_state))
    }
    
    /// Complete Bluesky OAuth flow
    pub async fn complete_auth(
        &self,
        code: &str,
        temp_state: &OAuthTempState,
    ) -> Result<SessionData, AuthError> {
        let pkce_verifier = temp_state.pkce_verifier.as_ref()
            .ok_or_else(|| AuthError::InvalidSession)?;
        let dpop_key = temp_state.dpop_private_key.as_ref()
            .ok_or_else(|| AuthError::InvalidSession)?;
        let oauth_endpoints = temp_state.oauth_endpoints.as_ref()
            .ok_or_else(|| AuthError::InvalidSession)?;
        let user_did = temp_state.user_did.as_ref()
            .ok_or_else(|| AuthError::InvalidSession)?;
        let handle = temp_state.handle.as_ref()
            .ok_or_else(|| AuthError::InvalidSession)?;
        
        let client = reqwest::Client::new();
        
        // Try token exchange with DPoP
        let dpop_proof = create_dpop_proof(
            dpop_key,
            "POST",
            &oauth_endpoints.token_url,
            None,
        )?;
        
        let mut token_response = client
            .post(&oauth_endpoints.token_url)
            .header("DPoP", &dpop_proof)
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", &self.redirect_url),
                ("client_id", &self.client_id),
                ("code_verifier", pkce_verifier),
            ])
            .send()
            .await?;
        
        let token_status = token_response.status();
        
        // Check if we need a nonce
        if token_status.as_u16() == 400 {
            if let Some(nonce_header) = token_response.headers().get("DPoP-Nonce") {
                if let Ok(nonce_str) = nonce_header.to_str() {
                    tracing::info!("Got DPoP nonce, retrying: {}", nonce_str);
                    
                    let dpop_proof_with_nonce = create_dpop_proof(
                        dpop_key,
                        "POST",
                        &oauth_endpoints.token_url,
                        Some(nonce_str),
                    )?;
                    
                    token_response = client
                        .post(&oauth_endpoints.token_url)
                        .header("DPoP", dpop_proof_with_nonce)
                        .form(&[
                            ("grant_type", "authorization_code"),
                            ("code", code),
                            ("redirect_uri", &self.redirect_url),
                            ("client_id", &self.client_id),
                            ("code_verifier", pkce_verifier),
                        ])
                        .send()
                        .await?;
                }
            }
        }
        
        let token_status = token_response.status();
        let token_text = token_response.text().await?;
        
        tracing::info!("Bluesky token response (status {}): {}", token_status, token_text);
        
        if !token_status.is_success() {
            return Err(AuthError::OAuthError(format!("Token exchange failed: {}", token_text)));
        }
        
        let token_data: serde_json::Value = serde_json::from_str(&token_text)?;
        let access_token = token_data
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AuthError::OAuthError("No access_token in response".to_string()))?
            .to_string();
        
        // Fetch profile
        let display_name = self.fetch_profile(
            user_did,
            &access_token,
            dpop_key,
            &oauth_endpoints.service_endpoint,
        ).await.ok();
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Ok(SessionData {
            auth_provider: AuthProvider::Bluesky,
            user_id: user_did.clone(),
            handle: handle.clone(),
            display_name,
            groups: vec![],
            created_at: now,
        })
    }
    
    async fn fetch_profile(
        &self,
        user_did: &str,
        access_token: &str,
        dpop_key: &str,
        pds_url: &str,
    ) -> Result<String, AuthError> {
        let api_url = format!("{}/xrpc/app.bsky.actor.getProfile", pds_url);
        let client = reqwest::Client::new();
        
        let dpop_proof_api = create_dpop_proof_with_ath(
            dpop_key,
            "GET",
            &api_url,
            None,
            Some(access_token),
        )?;
        
        let mut profile_response = client
            .get(&api_url)
            .query(&[("actor", user_did)])
            .header("Authorization", format!("DPoP {}", access_token))
            .header("DPoP", &dpop_proof_api)
            .send()
            .await?;
        
        // Retry with nonce if needed
        if profile_response.status().as_u16() == 401 {
            if let Some(nonce_header) = profile_response.headers().get("DPoP-Nonce") {
                if let Ok(nonce_str) = nonce_header.to_str() {
                    let dpop_proof_with_nonce = create_dpop_proof_with_ath(
                        dpop_key,
                        "GET",
                        &api_url,
                        Some(nonce_str),
                        Some(access_token),
                    )?;
                    
                    profile_response = client
                        .get(&api_url)
                        .query(&[("actor", user_did)])
                        .header("Authorization", format!("DPoP {}", access_token))
                        .header("DPoP", dpop_proof_with_nonce)
                        .send()
                        .await?;
                }
            }
        }
        
        if profile_response.status().is_success() {
            let profile_json: serde_json::Value = profile_response.json().await?;
            Ok(profile_json
                .get("displayName")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string())
        } else {
            Err(AuthError::NetworkError(
                reqwest::Error::from(profile_response.error_for_status().unwrap_err())
            ))
        }
    }
    
    async fn discover_oauth_endpoints(
        &self,
        handle: &str,
    ) -> Result<(String, OAuthEndpoints), AuthError> {
        tracing::info!("Starting OAuth service discovery for handle: {}", handle);
        
        let did = self.resolve_handle_to_did(handle).await?;
        tracing::info!("Resolved to DID: {}", did);
        
        let service_endpoint = self.resolve_did(&did).await?;
        tracing::info!("Resolved service endpoint: {}", service_endpoint);
        
        let auth_server = self.get_protected_resource_metadata(&service_endpoint).await?;
        tracing::info!("Found authorization server: {}", auth_server);
        
        let mut oauth_endpoints = self.get_authorization_server_metadata(&auth_server).await?;
        oauth_endpoints.service_endpoint = service_endpoint;
        
        Ok((did, oauth_endpoints))
    }
    
    async fn resolve_handle_to_did(&self, handle: &str) -> Result<String, AuthError> {
        if !handle.starts_with('@') {
            return Err(AuthError::InvalidHandleFormat);
        }
        
        let domain = &handle[1..];
        let txt_domain = format!("_atproto.{}.", domain);
        
        tracing::info!("Resolving DNS TXT record for: {}", txt_domain);
        
        match self.dns_resolver.txt_lookup(&txt_domain).await {
            Ok(lookup) => {
                for record in lookup {
                    let txt_data = record.to_string();
                    if txt_data.starts_with("did=") {
                        let did = txt_data[4..].trim_matches('"');
                        if did.starts_with("did:") {
                            tracing::info!("Resolved DID via DNS: {}", did);
                            return Ok(did.to_string());
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!("DNS TXT lookup failed: {}, trying HTTPS fallback", e);
            }
        }
        
        // HTTPS fallback
        let client = reqwest::Client::new();
        let url = format!("https://{}/.well-known/atproto-did", domain);
        let response = client.get(&url).send().await?;
        
        if !response.status().is_success() {
            return Err(AuthError::DnsError(format!(
                "Failed to resolve handle via HTTPS: {}",
                response.status()
            )));
        }
        
        let did = response.text().await?.trim().to_string();
        
        if did.starts_with("did:") {
            tracing::info!("Resolved DID via HTTPS: {}", did);
            Ok(did)
        } else {
            Err(AuthError::DnsError(format!(
                "Invalid DID format from HTTPS endpoint: {}",
                did
            )))
        }
    }
    
    async fn resolve_did(&self, did: &str) -> Result<String, AuthError> {
        let url = format!("https://plc.directory/{}", did);
        let client = reqwest::Client::new();
        let response = client.get(&url).send().await?;
        
        if !response.status().is_success() {
            return Err(AuthError::DidResolutionError(format!(
                "Failed to resolve DID: {}",
                response.status()
            )));
        }
        
        let did_doc: DidDocument = response.json().await?;
        
        if let Some(services) = did_doc.service {
            for service in services {
                if service.service_type == "AtprotoPersonalDataServer" {
                    return Ok(service.service_endpoint);
                }
            }
        }
        
        Err(AuthError::DidResolutionError(
            "No ATProto PDS service found in DID document".to_string()
        ))
    }
    
    async fn get_protected_resource_metadata(
        &self,
        service_endpoint: &str,
    ) -> Result<String, AuthError> {
        let url = Url::parse(service_endpoint)?;
        let path = url.path().trim_end_matches('/');
        let metadata_url = format!(
            "{}://{}{}/.well-known/oauth-protected-resource",
            url.scheme(),
            url.host_str().unwrap(),
            path
        );
        
        let client = reqwest::Client::new();
        let response = client.get(&metadata_url).send().await?;
        
        if !response.status().is_success() {
            return Err(AuthError::ServiceMetadataError(format!(
                "Failed to fetch resource metadata: {}",
                response.status()
            )));
        }
        
        let metadata: ProtectedResourceMetadata = response.json().await?;
        
        metadata
            .authorization_servers
            .first()
            .cloned()
            .ok_or_else(|| {
                AuthError::ServiceMetadataError(
                    "No authorization server found in protected resource metadata".to_string()
                )
            })
    }
    
    async fn get_authorization_server_metadata(
        &self,
        auth_server: &str,
    ) -> Result<OAuthEndpoints, AuthError> {
        let metadata_url = format!("{}/.well-known/oauth-authorization-server", auth_server);
        
        let client = reqwest::Client::new();
        let response = client.get(&metadata_url).send().await?;
        
        if !response.status().is_success() {
            return Err(AuthError::ServiceMetadataError(format!(
                "Failed to fetch authorization server metadata: {}",
                response.status()
            )));
        }
        
        let metadata: AuthorizationServerMetadata = response.json().await?;
        
        Ok(OAuthEndpoints {
            auth_url: metadata.authorization_endpoint,
            token_url: metadata.token_endpoint,
            service_endpoint: String::new(),
        })
    }
}

// DPoP helper functions
fn generate_dpop_key() -> (SigningKey, String) {
    use rand::rngs::OsRng;
    let signing_key = SigningKey::random(&mut OsRng);
    let key_bytes = signing_key.to_bytes();
    let key_b64 = URL_SAFE_NO_PAD.encode(key_bytes);
    (signing_key, key_b64)
}

fn create_dpop_proof(
    private_key_b64: &str,
    htm: &str,
    htu: &str,
    nonce: Option<&str>,
) -> Result<String, AuthError> {
    create_dpop_proof_with_ath(private_key_b64, htm, htu, nonce, None)
}

fn create_dpop_proof_with_ath(
    private_key_b64: &str,
    htm: &str,
    htu: &str,
    nonce: Option<&str>,
    access_token: Option<&str>,
) -> Result<String, AuthError> {
    let key_bytes = URL_SAFE_NO_PAD
        .decode(private_key_b64)
        .map_err(|e| AuthError::OAuthError(format!("Failed to decode DPoP key: {}", e)))?;
    let key_array: [u8; 32] = key_bytes
        .as_slice()
        .try_into()
        .map_err(|e| AuthError::OAuthError(format!("Invalid DPoP key length: {:?}", e)))?;
    let signing_key = SigningKey::from_bytes(&key_array.into())
        .map_err(|e| AuthError::OAuthError(format!("Invalid DPoP key: {}", e)))?;
    let verifying_key = signing_key.verifying_key();
    let point = verifying_key.to_encoded_point(false);
    
    let x = URL_SAFE_NO_PAD.encode(point.x().unwrap());
    let y = URL_SAFE_NO_PAD.encode(point.y().unwrap());
    
    let jwk = serde_json::json!({
        "kty": "EC",
        "crv": "P-256",
        "x": x,
        "y": y,
    });
    
    let header = serde_json::json!({
        "typ": "dpop+jwt",
        "alg": "ES256",
        "jwk": jwk,
    });
    
    let mut claims = serde_json::json!({
        "htm": htm,
        "htu": htu,
        "jti": Uuid::new_v4().to_string(),
        "iat": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| AuthError::OAuthError(format!("Time error: {}", e)))?
            .as_secs(),
    });
    
    if let Some(n) = nonce {
        claims["nonce"] = serde_json::Value::String(n.to_string());
    }
    
    if let Some(token) = access_token {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        let hash = hasher.finalize();
        let ath = URL_SAFE_NO_PAD.encode(&hash);
        claims["ath"] = serde_json::Value::String(ath);
    }
    
    let header_b64 = URL_SAFE_NO_PAD.encode(
        serde_json::to_string(&header)
            .map_err(|e| AuthError::JsonError(e))?
    );
    let claims_b64 = URL_SAFE_NO_PAD.encode(
        serde_json::to_string(&claims)
            .map_err(|e| AuthError::JsonError(e))?
    );
    let message = format!("{}.{}", header_b64, claims_b64);
    
    use p256::ecdsa::{Signature, signature::Signer};
    let signature: Signature = signing_key.sign(message.as_bytes());
    let sig_b64 = URL_SAFE_NO_PAD.encode(signature.to_bytes());
    
    Ok(format!("{}.{}", message, sig_b64))
}
