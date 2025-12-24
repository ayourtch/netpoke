use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Router,
    Form,
};
use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    PkceCodeChallenge, RedirectUrl, Scope, TokenUrl, TokenResponse,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tower_http::cors::CorsLayer;
use trust_dns_resolver::TokioAsyncResolver;
use thiserror::Error;
use url::Url;
use uuid::Uuid;
use p256::ecdsa::SigningKey;
use p256::elliptic_curve::sec1::ToEncodedPoint;
use sha2::{Sha256, Digest};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

#[derive(Error, Debug)]
pub enum BlueSkyError {
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
}

#[derive(Clone)]
struct AppState {
    sessions: Arc<tokio::sync::RwLock<HashMap<String, SessionData>>>,
    dns_resolver: TokioAsyncResolver,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SessionData {
    auth_provider: AuthProvider,
    user_did: String,
    handle: String,
    display_name: Option<String>,
    access_token: String,
    pkce_verifier: Option<String>,
    oauth_endpoints: Option<OAuthEndpoints>,
    dpop_private_key: Option<String>, // Base64-encoded private key for DPoP
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum AuthProvider {
    Bluesky,
    GitHub,
    Google,
    LinkedIn,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct OAuthEndpoints {
    auth_url: String,
    token_url: String,
    service_endpoint: String,  // The PDS service endpoint
}

#[derive(Deserialize)]
struct AuthCallback {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Deserialize)]
struct HandleForm {
    handle: String,
}

#[derive(Serialize)]
struct AuthUrlResponse {
    auth_url: String,
    state: String,
    pkce_verifier: String,
}

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

impl From<BlueSkyError> for StatusCode {
    fn from(error: BlueSkyError) -> StatusCode {
        match error {
            BlueSkyError::InvalidHandleFormat => StatusCode::BAD_REQUEST,
            BlueSkyError::DnsError(_) | BlueSkyError::DidResolutionError(_)
            | BlueSkyError::ServiceMetadataError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            BlueSkyError::NetworkError(_) | BlueSkyError::JsonError(_) | BlueSkyError::UrlError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // Load .env file
    dotenvy::dotenv().ok();

    let client_id = std::env::var("BLUESKY_CLIENT_ID")
        .expect("BLUESKY_CLIENT_ID must be set");
    let redirect_url = std::env::var("BLUESKY_REDIRECT_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:3000/auth/callback".to_string());

    // Store client credentials for dynamic OAuth client creation
    std::env::set_var("OAUTH_CLIENT_ID", client_id);
    std::env::set_var("OAUTH_REDIRECT_URL", redirect_url);

    // Initialize DNS resolver
    let resolver = TokioAsyncResolver::tokio_from_system_conf()
        .expect("Failed to create DNS resolver");

    let state = AppState {
        sessions: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        dns_resolver: resolver,
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/protected", get(protected_page))
        .route("/auth/login", post(auth_login))
        .route("/auth/callback", get(auth_callback))
        .route("/auth/github/login", get(github_login))
        .route("/auth/github/callback", get(github_callback))
        .route("/auth/google/login", get(google_login))
        .route("/auth/google/callback", get(google_callback))
        .route("/auth/linkedin/login", get(linkedin_login))
        .route("/auth/linkedin/callback", get(linkedin_callback))
        .route("/auth/logout", post(auth_logout))
        .route("/client-metadata.json", get(client_metadata))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    tracing::info!("Server running on http://0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
}

async fn client_metadata() -> Response {
    let metadata = include_str!("../client-metadata.json");
    (StatusCode::OK, [
        (header::CONTENT_TYPE, "application/json"),
    ], metadata).into_response()
}

async fn index() -> Html<&'static str> {
    Html(r#"
<!DOCTYPE html>
<html>
<head>
    <title>Bluesky Auth Demo</title>
    <style>
        body { font-family: Arial, sans-serif; max-width: 800px; margin: 0 auto; padding: 20px; }
        .container { text-align: center; margin-top: 50px; }
        .form-group { margin: 20px 0; }
        input[type="text"] {
            padding: 10px;
            border: 2px solid #ddd;
            border-radius: 6px;
            width: 300px;
            font-size: 16px;
        }
        input[type="text"]:focus {
            border-color: #0085ff;
            outline: none;
        }
        .btn {
            background-color: #0085ff;
            color: white;
            padding: 12px 24px;
            text-decoration: none;
            border-radius: 6px;
            display: inline-block;
            margin: 10px;
            border: none;
            cursor: pointer;
            font-size: 16px;
        }
        .btn:hover { background-color: #0066cc; }
        .error { color: #dc3545; margin-top: 10px; }
    </style>
</head>
<body>
    <div class="container">
        <h1>Multi-Provider Authentication Demo</h1>
        <p>Welcome! Choose your authentication method:</p>

        <h3>Login with Bluesky</h3>
        <p>Enter your Bluesky handle to authenticate:</p>
        <form id="loginForm" method="post" action="/auth/login">
            <div class="form-group">
                <input type="text"
                       name="handle"
                       placeholder="@alice.bsky.social"
                       pattern="@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$"
                       title="Enter a valid Bluesky handle (e.g., @alice.bsky.social)"
                       required>
            </div>
            <button type="submit" class="btn">Login with Bluesky</button>
        </form>
        <div id="error-message" class="error" style="display: none;"></div>

        <hr style="margin: 30px 0;">

        <h3>Login with GitHub</h3>
        <a href="/auth/github/login" class="btn" style="background-color: #24292e;">
            <svg height="16" width="16" style="vertical-align: text-bottom; margin-right: 8px;" viewBox="0 0 16 16" fill="white">
                <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"></path>
            </svg>
            Login with GitHub
        </a>

        <hr style="margin: 30px 0;">

        <h3>Login with Google</h3>
        <a href="/auth/google/login" class="btn" style="background-color: #4285f4;">
            <svg height="16" width="16" style="vertical-align: text-bottom; margin-right: 8px;" viewBox="0 0 24 24">
                <path fill="white" d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"/>
                <path fill="white" d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"/>
                <path fill="white" d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"/>
                <path fill="white" d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"/>
            </svg>
            Login with Google
        </a>

        <hr style="margin: 30px 0;">

        <h3>Login with LinkedIn</h3>
        <a href="/auth/linkedin/login" class="btn" style="background-color: #0077b5;">
            <svg height="16" width="16" style="vertical-align: text-bottom; margin-right: 8px;" viewBox="0 0 24 24" fill="white">
                <path d="M20.447 20.452h-3.554v-5.569c0-1.328-.027-3.037-1.852-3.037-1.853 0-2.136 1.445-2.136 2.939v5.667H9.351V9h3.414v1.561h.046c.477-.9 1.637-1.85 3.37-1.85 3.601 0 4.267 2.37 4.267 5.455v6.286zM5.337 7.433c-1.144 0-2.063-.926-2.063-2.065 0-1.138.92-2.063 2.063-2.063 1.14 0 2.064.925 2.064 2.063 0 1.139-.925 2.065-2.064 2.065zm1.782 13.019H3.555V9h3.564v11.452zM22.225 0H1.771C.792 0 0 .774 0 1.729v20.542C0 23.227.792 24 1.771 24h20.451C23.2 24 24 23.227 24 22.271V1.729C24 .774 23.2 0 22.222 0h.003z"/>
            </svg>
            Login with LinkedIn
        </a>

        <hr style="margin: 30px 0;">

        <a href="/protected" class="btn">Access Protected Page</a>
    </div>
    <script>
        document.getElementById('loginForm').addEventListener('submit', async function(e) {
            e.preventDefault();
            const formData = new FormData(this);
            const errorDiv = document.getElementById('error-message');

            try {
                const response = await fetch('/auth/login', {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/x-www-form-urlencoded',
                    },
                    body: new URLSearchParams(formData)
                });

                if (!response.ok) {
                    const errorText = await response.text();
                    errorDiv.textContent = errorText || 'Login failed';
                    errorDiv.style.display = 'block';
                    return;
                }

                const data = await response.json();
                window.location.href = data.auth_url;

            } catch (error) {
                errorDiv.textContent = 'Network error: ' + error.message;
                errorDiv.style.display = 'block';
            }
        });
    </script>
</body>
</html>
    "#)
}

async fn protected_page(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Response, StatusCode> {
    if let Some(session_cookie) = headers.get("cookie") {
        if let Ok(cookie_str) = session_cookie.to_str() {
            if let Some(session_id) = extract_session_id(cookie_str) {
                let sessions = state.sessions.read().await;
                if let Some(session_data) = sessions.get(&session_id) {
                    let provider_name = match session_data.auth_provider {
                        AuthProvider::Bluesky => "Bluesky",
                        AuthProvider::GitHub => "GitHub",
                        AuthProvider::Google => "Google",
                        AuthProvider::LinkedIn => "LinkedIn",
                    };
                    let auth_endpoint = session_data.oauth_endpoints.as_ref()
                        .map(|e| e.auth_url.as_str())
                        .unwrap_or("N/A");

                    let html = format!(r#"
<!DOCTYPE html>
<html>
<head>
    <title>Protected Page</title>
    <style>
        body {{ font-family: Arial, sans-serif; max-width: 800px; margin: 0 auto; padding: 20px; }}
        .user-info {{ background: #f5f5f5; padding: 20px; border-radius: 8px; margin: 20px 0; }}
        .btn {{
            background-color: #0085ff;
            color: white;
            padding: 12px 24px;
            text-decoration: none;
            border-radius: 6px;
            display: inline-block;
            margin: 10px;
        }}
        .btn-danger {{ background-color: #dc3545; }}
        .btn-danger:hover {{ background-color: #c82333; }}
    </style>
</head>
<body>
    <div class="user-info">
        <h2>Welcome to the Protected Page!</h2>
        <p><strong>Provider:</strong> {}</p>
        <p><strong>DID:</strong> {}</p>
        <p><strong>Handle:</strong> {}</p>
        <p><strong>Display Name:</strong> {}</p>
        <p><strong>Auth Endpoint:</strong> {}</p>
    </div>
    <p>This page is only accessible to authenticated users.</p>
    <a href="/" class="btn">Back to Home</a>
    <form action="/auth/logout" method="post" style="display: inline;">
        <button type="submit" class="btn btn-danger">Logout</button>
    </form>
</body>
</html>
                    "#,
                        provider_name,
                        session_data.user_did,
                        session_data.handle,
                        session_data.display_name.as_deref().unwrap_or("N/A"),
                        auth_endpoint
                    );
                    return Ok(Html(html).into_response());
                }
            }
        }
    }

    let html = r#"
<!DOCTYPE html>
<html>
<head>
    <title>Access Denied</title>
    <style>
        body { font-family: Arial, sans-serif; max-width: 800px; margin: 0 auto; padding: 20px; }
        .error { color: #dc3545; text-align: center; }
        .btn {
            background-color: #0085ff;
            color: white;
            padding: 12px 24px;
            text-decoration: none;
            border-radius: 6px;
            display: inline-block;
            margin: 10px;
        }
    </style>
</head>
<body>
    <div class="error">
        <h1>Access Denied</h1>
        <p>You need to be authenticated to access this page.</p>
        <a href="/" class="btn">Back to Home</a>
    </div>
</body>
</html>
    "#;
    Ok(Html(html).into_response())
}

// Step 1: Convert Handle to DID via DNS TXT record or HTTPS fallback
async fn resolve_handle_to_did(
    handle: &str,
    resolver: &TokioAsyncResolver,
) -> Result<String, BlueSkyError> {
    if !handle.starts_with('@') {
        return Err(BlueSkyError::InvalidHandleFormat);
    }

    let domain = &handle[1..]; // Remove @ prefix

    // Try DNS TXT record first
    let txt_domain = format!("_atproto.{}.", domain);
    tracing::info!("Resolving DNS TXT record for: {}", txt_domain);

    match resolver.txt_lookup(&txt_domain).await {
        Ok(lookup) => {
            for record in lookup {
                let txt_data = record.to_string();
                tracing::debug!("Found TXT record: {}", txt_data);

                // Parse TXT record for "did=did:..." format
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

    // Fallback to HTTPS well-known endpoint
    tracing::info!("Using HTTPS fallback for handle resolution: https://{}/.well-known/atproto-did", domain);
    let client = reqwest::Client::new();
    let url = format!("https://{}/.well-known/atproto-did", domain);

    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Err(BlueSkyError::DnsError(
            format!("Failed to resolve handle via HTTPS: {}", response.status())
        ));
    }

    let did = response.text().await?;
    let did = did.trim();

    if did.starts_with("did:") {
        tracing::info!("Resolved DID via HTTPS: {}", did);
        Ok(did.to_string())
    } else {
        Err(BlueSkyError::DnsError(
            format!("Invalid DID format from HTTPS endpoint: {}", did)
        ))
    }
}

// Step 2: Resolve DID to get service endpoint
async fn resolve_did(did: &str) -> Result<String, BlueSkyError> {
    let url = format!("https://plc.directory/{}", did);
    tracing::info!("Resolving DID from: {}", url);

    let client = reqwest::Client::new();
    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Err(BlueSkyError::DidResolutionError(
            format!("Failed to resolve DID: {}", response.status())
        ));
    }

    let did_doc: DidDocument = response.json().await?;

    // Find the ATProto service
    if let Some(services) = did_doc.service {
        for service in services {
            if service.service_type == "AtprotoPersonalDataServer" {
                tracing::info!("Found PDS service endpoint: {}", service.service_endpoint);
                return Ok(service.service_endpoint);
            }
        }
    }

    Err(BlueSkyError::DidResolutionError(
        "No ATProto PDS service found in DID document".to_string()
    ))
}

// Step 3: Fetch OAuth protected resource metadata
async fn get_protected_resource_metadata(
    service_endpoint: &str,
) -> Result<String, BlueSkyError> {
    let url = Url::parse(service_endpoint)?;
    let path = url.path().trim_end_matches('/');
    let metadata_url = format!("{}://{}{}/.well-known/oauth-protected-resource",
                               url.scheme(), url.host_str().unwrap(), path);

    tracing::info!("Fetching protected resource metadata from: {}", metadata_url);

    let client = reqwest::Client::new();
    let response = client.get(&metadata_url).send().await?;

    if !response.status().is_success() {
        return Err(BlueSkyError::ServiceMetadataError(
            format!("Failed to fetch resource metadata: {}", response.status())
        ));
    }

    let metadata: ProtectedResourceMetadata = response.json().await?;

    if let Some(auth_server) = metadata.authorization_servers.first() {
        Ok(auth_server.clone())
    } else {
        Err(BlueSkyError::ServiceMetadataError(
            "No authorization server found in protected resource metadata".to_string()
        ))
    }
}

// Step 4: Fetch OAuth authorization server metadata
async fn get_authorization_server_metadata(
    auth_server: &str,
) -> Result<OAuthEndpoints, BlueSkyError> {
    let metadata_url = format!("{}/.well-known/oauth-authorization-server", auth_server);

    tracing::info!("Fetching authorization server metadata from: {}", metadata_url);

    let client = reqwest::Client::new();
    let response = client.get(&metadata_url).send().await?;

    if !response.status().is_success() {
        return Err(BlueSkyError::ServiceMetadataError(
            format!("Failed to fetch authorization server metadata: {}", response.status())
        ));
    }

    let metadata: AuthorizationServerMetadata = response.json().await?;

    Ok(OAuthEndpoints {
        auth_url: metadata.authorization_endpoint,
        token_url: metadata.token_endpoint,
        service_endpoint: String::new(),  // Will be filled in by caller
    })
}

// Complete service discovery flow - returns (DID, OAuthEndpoints)
async fn discover_oauth_endpoints(
    handle: &str,
    resolver: &TokioAsyncResolver,
) -> Result<(String, OAuthEndpoints), BlueSkyError> {
    tracing::info!("Starting OAuth service discovery for handle: {}", handle);

    // Step 1: Handle to DID
    let did = resolve_handle_to_did(handle, resolver).await?;
    tracing::info!("Resolved to DID: {}", did);

    // Step 2: DID to service endpoint
    let service_endpoint = resolve_did(&did).await?;
    tracing::info!("Resolved service endpoint: {}", service_endpoint);

    // Step 3: Get authorization server URL
    let auth_server = get_protected_resource_metadata(&service_endpoint).await?;
    tracing::info!("Found authorization server: {}", auth_server);

    // Step 4: Get OAuth endpoints
    let mut oauth_endpoints = get_authorization_server_metadata(&auth_server).await?;
    oauth_endpoints.service_endpoint = service_endpoint;
    tracing::info!("OAuth endpoints discovered: auth={}, token={}, service={}",
                   oauth_endpoints.auth_url, oauth_endpoints.token_url, oauth_endpoints.service_endpoint);

    Ok((did, oauth_endpoints))
}

async fn auth_login(
    State(state): State<AppState>,
    Form(form): Form<HandleForm>,
) -> Result<Response, StatusCode> {
    tracing::info!("Processing login request for handle: {}", form.handle);

    // Discover OAuth endpoints dynamically
    let (user_did, oauth_endpoints) = discover_oauth_endpoints(&form.handle, &state.dns_resolver)
        .await
        .map_err(|e| {
            tracing::error!("OAuth service discovery failed: {}", e);
            StatusCode::from(e)
        })?;

    // Create OAuth client with discovered endpoints (no client_secret for public clients)
    let client_id = std::env::var("OAUTH_CLIENT_ID").expect("OAUTH_CLIENT_ID must be set");
    let redirect_url = std::env::var("OAUTH_REDIRECT_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:3000/auth/callback".to_string());

    let oauth_client = BasicClient::new(
        ClientId::new(client_id),
        None, // No client_secret - Bluesky OAuth uses PKCE
        AuthUrl::new(oauth_endpoints.auth_url.clone()).unwrap(),
        Some(TokenUrl::new(oauth_endpoints.token_url.clone()).unwrap()),
    )
    .set_redirect_uri(RedirectUrl::new(redirect_url).unwrap());

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let csrf_state = CsrfToken::new_random();

    let (auth_url, _) = oauth_client
        .authorize_url(|| csrf_state.clone())
        .add_scope(Scope::new("atproto".to_string()))
        .add_scope(Scope::new("rpc:app.bsky.actor.getProfile?aud=did:web:api.bsky.app#bsky_appview".to_string()))
        .add_extra_param("code_challenge", pkce_challenge.as_str())
        .add_extra_param("code_challenge_method", "S256")
        .url();

    // Generate DPoP key for Bluesky
    let (_signing_key, dpop_private_key) = generate_dpop_key();

    let session_id = Uuid::new_v4().to_string();
    let mut sessions = state.sessions.write().await;
    sessions.insert(session_id.clone(), SessionData {
        auth_provider: AuthProvider::Bluesky,
        user_did: user_did,
        handle: form.handle,
        display_name: None,
        access_token: "".to_string(),
        pkce_verifier: Some(pkce_verifier.secret().clone()),
        oauth_endpoints: Some(oauth_endpoints),
        dpop_private_key: Some(dpop_private_key),
    });

    let response = AuthUrlResponse {
        auth_url: auth_url.to_string(),
        state: csrf_state.secret().clone(),
        pkce_verifier: pkce_verifier.secret().clone(),
    };

    let json = serde_json::to_string(&response)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((StatusCode::OK, [
        (header::SET_COOKIE, format!("session_id={}; Path=/; HttpOnly; SameSite=Lax", session_id)),
        (header::CONTENT_TYPE, "application/json".to_string()),
    ], json).into_response())
}

async fn auth_callback(
    State(state): State<AppState>,
    Query(query): Query<AuthCallback>,
    headers: axum::http::HeaderMap,
) -> Result<Response, StatusCode> {
    // Check for OAuth errors first
    if let Some(error) = &query.error {
        let error_msg = query.error_description.as_deref().unwrap_or("Unknown error");
        tracing::error!("OAuth callback error: {} - {}", error, error_msg);
        let html = format!(r#"
<!DOCTYPE html>
<html>
<head>
    <title>Authentication Error</title>
    <style>
        body {{ font-family: Arial, sans-serif; max-width: 800px; margin: 0 auto; padding: 20px; }}
        .error {{ color: #dc3545; }}
        .btn {{ background-color: #0085ff; color: white; padding: 12px 24px; text-decoration: none; border-radius: 6px; display: inline-block; margin: 10px; }}
    </style>
</head>
<body>
    <div class="error">
        <h1>Authentication Failed</h1>
        <p><strong>Error:</strong> {}</p>
        <p><strong>Description:</strong> {}</p>
        <a href="/" class="btn">Try Again</a>
    </div>
</body>
</html>
        "#, error, error_msg);
        return Ok(Html(html).into_response());
    }

    let code = query.code.as_ref().ok_or(StatusCode::BAD_REQUEST)?;

    if let Some(session_cookie) = headers.get("cookie") {
        if let Ok(cookie_str) = session_cookie.to_str() {
            if let Some(session_id) = extract_session_id(cookie_str) {
                // First read to get session data
                let session_data = {
                    let sessions = state.sessions.read().await;
                    sessions.get(&session_id).cloned()
                };

                if let Some(session_data) = session_data {
                    let pkce_verifier = session_data.pkce_verifier.as_ref()
                        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
                    let dpop_key = session_data.dpop_private_key.as_ref()
                        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

                    let client_id = std::env::var("OAUTH_CLIENT_ID").expect("OAUTH_CLIENT_ID must be set");
                    let redirect_url = std::env::var("OAUTH_REDIRECT_URL")
                        .unwrap_or_else(|_| "http://127.0.0.1:3000/auth/callback".to_string());

                    let oauth_endpoints = session_data.oauth_endpoints.as_ref()
                        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

                    // Try token exchange with DPoP (may need nonce retry)
                    let client = reqwest::Client::new();
                    let mut dpop_nonce: Option<String> = None;

                    // First attempt without nonce
                    let dpop_proof = create_dpop_proof(
                        dpop_key,
                        "POST",
                        &oauth_endpoints.token_url,
                        None,
                    ).map_err(|e| {
                        tracing::error!("Failed to create DPoP proof: {:?}", e);
                        StatusCode::INTERNAL_SERVER_ERROR
                    })?;

                    let mut token_response = client
                        .post(&oauth_endpoints.token_url)
                        .header("DPoP", &dpop_proof)
                        .form(&[
                            ("grant_type", "authorization_code"),
                            ("code", code),
                            ("redirect_uri", &redirect_url),
                            ("client_id", &client_id),
                            ("code_verifier", pkce_verifier),
                        ])
                        .send()
                        .await
                        .map_err(|e| {
                            tracing::error!("Bluesky token request failed: {:?}", e);
                            StatusCode::INTERNAL_SERVER_ERROR
                        })?;

                    let token_status = token_response.status();

                    // Check if we need a nonce
                    if token_status.as_u16() == 400 {
                        // Extract nonce from DPoP-Nonce header if present
                        if let Some(nonce_header) = token_response.headers().get("DPoP-Nonce") {
                            if let Ok(nonce_str) = nonce_header.to_str() {
                                dpop_nonce = Some(nonce_str.to_string());
                                tracing::info!("Got DPoP nonce, retrying: {}", nonce_str);

                                // Retry with nonce
                                let dpop_proof_with_nonce = create_dpop_proof(
                                    dpop_key,
                                    "POST",
                                    &oauth_endpoints.token_url,
                                    Some(nonce_str),
                                ).map_err(|e| {
                                    tracing::error!("Failed to create DPoP proof with nonce: {:?}", e);
                                    StatusCode::INTERNAL_SERVER_ERROR
                                })?;

                                token_response = client
                                    .post(&oauth_endpoints.token_url)
                                    .header("DPoP", dpop_proof_with_nonce)
                                    .form(&[
                                        ("grant_type", "authorization_code"),
                                        ("code", code),
                                        ("redirect_uri", &redirect_url),
                                        ("client_id", &client_id),
                                        ("code_verifier", pkce_verifier),
                                    ])
                                    .send()
                                    .await
                                    .map_err(|e| {
                                        tracing::error!("Bluesky token request (with nonce) failed: {:?}", e);
                                        StatusCode::INTERNAL_SERVER_ERROR
                                    })?;
                            }
                        }
                    }

                    let token_status = token_response.status();
                    let token_text = token_response.text().await.map_err(|e| {
                        tracing::error!("Failed to read token response: {:?}", e);
                        StatusCode::INTERNAL_SERVER_ERROR
                    })?;

                    tracing::info!("Bluesky token response (status {}): {}", token_status, token_text);

                    if !token_status.is_success() {
                        tracing::error!("Token exchange failed with status {}: {}", token_status, token_text);
                        return Err(StatusCode::INTERNAL_SERVER_ERROR);
                    }

                    let token_data: serde_json::Value = serde_json::from_str(&token_text)
                        .map_err(|e| {
                            tracing::error!("Failed to parse token response: {:?}", e);
                            StatusCode::INTERNAL_SERVER_ERROR
                        })?;

                    let access_token = token_data
                        .get("access_token")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            tracing::error!("No access_token in response: {}", token_text);
                            StatusCode::INTERNAL_SERVER_ERROR
                        })?
                        .to_string();

                    tracing::info!("Bluesky token exchange successful");

                    // Use the DID and handle from the session (already resolved during login)
                    let user_did_from_session = session_data.user_did.clone();
                    let handle_from_session = session_data.handle.clone();

                    // Get the PDS service endpoint from oauth_endpoints
                    let pds_url = if let Some(endpoints) = &session_data.oauth_endpoints {
                        &endpoints.service_endpoint
                    } else {
                        "https://bsky.social"
                    };

                    // Fetch profile from PDS with DPoP authentication
                    let api_url = format!("{}/xrpc/app.bsky.actor.getProfile", pds_url);
                    tracing::info!("Fetching profile from: {} with actor: {}", api_url, user_did_from_session);

                    // Try profile fetch with DPoP (may need nonce retry)
                    let dpop_proof_api = create_dpop_proof_with_ath(
                        dpop_key,
                        "GET",
                        &api_url,
                        None,  // Try without nonce first
                        Some(&access_token),  // Include access token hash
                    ).map_err(|e| {
                        tracing::error!("Failed to create DPoP proof for profile API: {:?}", e);
                        StatusCode::INTERNAL_SERVER_ERROR
                    })?;

                    let mut profile_response = client
                        .get(&api_url)
                        .query(&[("actor", user_did_from_session.as_str())])
                        .header("Authorization", format!("DPoP {}", access_token))
                        .header("DPoP", &dpop_proof_api)
                        .send()
                        .await;

                    // Check if we need a nonce and retry
                    if let Ok(ref resp) = profile_response {
                        if resp.status().as_u16() == 401 {
                            if let Some(nonce_header) = resp.headers().get("DPoP-Nonce") {
                                if let Ok(nonce_str) = nonce_header.to_str() {
                                    tracing::info!("Got DPoP nonce for API, retrying: {}", nonce_str);

                                    let dpop_proof_with_nonce = create_dpop_proof_with_ath(
                                        dpop_key,
                                        "GET",
                                        &api_url,
                                        Some(nonce_str),
                                        Some(&access_token),  // Include access token hash
                                    ).map_err(|e| {
                                        tracing::error!("Failed to create DPoP proof with nonce for API: {:?}", e);
                                        StatusCode::INTERNAL_SERVER_ERROR
                                    })?;

                                    profile_response = client
                                        .get(&api_url)
                                        .query(&[("actor", user_did_from_session.as_str())])
                                        .header("Authorization", format!("DPoP {}", access_token))
                                        .header("DPoP", dpop_proof_with_nonce)
                                        .send()
                                        .await;
                                }
                            }
                        }
                    }

                    // Try to get display name from profile, fall back to handle if it fails
                    let display_name = if let Ok(response) = profile_response {
                        let status = response.status();
                        if status.is_success() {
                            if let Ok(profile_json) = response.json::<serde_json::Value>().await {
                                tracing::info!("Successfully fetched profile from PDS");
                                profile_json.get("displayName")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string())
                                    .or_else(|| Some(handle_from_session.clone()))
                            } else {
                                tracing::warn!("Failed to parse profile JSON, using handle");
                                Some(handle_from_session.clone())
                            }
                        } else {
                            let response_text = response.text().await.unwrap_or_else(|_| "unable to read".to_string());
                            tracing::warn!("Profile fetch returned status {}: {}, using handle as display name", status, response_text);
                            Some(handle_from_session.clone())
                        }
                    } else {
                        tracing::warn!("Failed to fetch profile from PDS, using handle as display name");
                        Some(handle_from_session.clone())
                    };

                    // Update session with user data
                    let mut sessions = state.sessions.write().await;
                    sessions.insert(session_id.clone(), SessionData {
                        auth_provider: AuthProvider::Bluesky,
                        user_did: user_did_from_session,
                        handle: handle_from_session,
                        display_name,
                        access_token,
                        pkce_verifier: None,
                        oauth_endpoints: session_data.oauth_endpoints,
                        dpop_private_key: session_data.dpop_private_key,
                    });

                    return Ok(Redirect::to("/protected").into_response());
                }
            }
        }
    }

    Ok(Redirect::to("/").into_response())
}

async fn auth_logout(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Response, StatusCode> {
    if let Some(session_cookie) = headers.get("cookie") {
        if let Ok(cookie_str) = session_cookie.to_str() {
            if let Some(session_id) = extract_session_id(cookie_str) {
                let mut sessions = state.sessions.write().await;
                sessions.remove(&session_id);
            }
        }
    }

    Ok((StatusCode::FOUND, [
        (header::SET_COOKIE, "session_id=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"),
        (header::LOCATION, "/"),
    ]).into_response())
}

fn extract_session_id(cookie_str: &str) -> Option<String> {
    cookie_str
        .split(';')
        .find_map(|cookie| {
            let parts: Vec<&str> = cookie.trim().split('=').collect();
            if parts.len() == 2 && parts[0] == "session_id" {
                Some(parts[1].to_string())
            } else {
                None
            }
        })
}

// Generate ES256 keypair for DPoP
fn generate_dpop_key() -> (SigningKey, String) {
    use rand::rngs::OsRng;
    let signing_key = SigningKey::random(&mut OsRng);
    let key_bytes = signing_key.to_bytes();
    let key_b64 = URL_SAFE_NO_PAD.encode(key_bytes);
    (signing_key, key_b64)
}

// Create DPoP proof JWT
fn create_dpop_proof(
    private_key_b64: &str,
    htm: &str,
    htu: &str,
    nonce: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    create_dpop_proof_with_ath(private_key_b64, htm, htu, nonce, None)
}

// Create DPoP proof JWT with optional access token hash
fn create_dpop_proof_with_ath(
    private_key_b64: &str,
    htm: &str,
    htu: &str,
    nonce: Option<&str>,
    access_token: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    let key_bytes = URL_SAFE_NO_PAD.decode(private_key_b64)?;
    let key_array: [u8; 32] = key_bytes.as_slice().try_into()?;
    let signing_key = SigningKey::from_bytes(&key_array.into())?;
    let verifying_key = signing_key.verifying_key();
    let point = verifying_key.to_encoded_point(false);

    let x = URL_SAFE_NO_PAD.encode(&point.x().unwrap());
    let y = URL_SAFE_NO_PAD.encode(&point.y().unwrap());

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
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs(),
    });

    if let Some(n) = nonce {
        claims["nonce"] = serde_json::Value::String(n.to_string());
    }

    // Add access token hash (ath) if token is provided
    if let Some(token) = access_token {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        let hash = hasher.finalize();
        let ath = URL_SAFE_NO_PAD.encode(&hash);
        claims["ath"] = serde_json::Value::String(ath);
    }

    let header_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_string(&header)?);
    let claims_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_string(&claims)?);
    let message = format!("{}.{}", header_b64, claims_b64);

    use p256::ecdsa::{Signature, signature::Signer};
    let signature: Signature = signing_key.sign(message.as_bytes());
    let sig_b64 = URL_SAFE_NO_PAD.encode(signature.to_bytes());

    Ok(format!("{}.{}", message, sig_b64))
}

// GitHub OAuth handlers
async fn github_login(State(state): State<AppState>) -> Result<Response, StatusCode> {
    let client_id = std::env::var("GITHUB_CLIENT_ID")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let redirect_url = std::env::var("GITHUB_REDIRECT_URL")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let github_client = BasicClient::new(
        ClientId::new(client_id),
        None,
        AuthUrl::new("https://github.com/login/oauth/authorize".to_string()).unwrap(),
        Some(TokenUrl::new("https://github.com/login/oauth/access_token".to_string()).unwrap()),
    )
    .set_redirect_uri(RedirectUrl::new(redirect_url).unwrap());

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let csrf_state = CsrfToken::new_random();

    let (auth_url, _) = github_client
        .authorize_url(|| csrf_state.clone())
        .add_scope(Scope::new("read:user".to_string()))
        .add_scope(Scope::new("user:email".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    let session_id = Uuid::new_v4().to_string();
    let mut sessions = state.sessions.write().await;
    sessions.insert(session_id.clone(), SessionData {
        auth_provider: AuthProvider::GitHub,
        user_did: "".to_string(),
        handle: "".to_string(),
        display_name: None,
        access_token: "".to_string(),
        pkce_verifier: Some(pkce_verifier.secret().clone()),
        oauth_endpoints: None,
        dpop_private_key: None,
    });

    Ok((StatusCode::FOUND, [
        (header::SET_COOKIE, format!("session_id={}; Path=/; HttpOnly; SameSite=Lax", session_id)),
        (header::LOCATION, auth_url.to_string()),
    ]).into_response())
}

#[derive(Deserialize)]
struct GitHubUser {
    login: String,
    name: Option<String>,
    email: Option<String>,
    id: u64,
}

async fn github_callback(
    State(state): State<AppState>,
    Query(query): Query<AuthCallback>,
    headers: axum::http::HeaderMap,
) -> Result<Response, StatusCode> {
    // Check for OAuth errors
    if let Some(error) = &query.error {
        let error_msg = query.error_description.as_deref().unwrap_or("Unknown error");
        tracing::error!("GitHub OAuth callback error: {} - {}", error, error_msg);
        return Ok(Redirect::to("/?error=github_auth_failed").into_response());
    }

    let code = query.code.as_ref().ok_or(StatusCode::BAD_REQUEST)?;

    if let Some(session_cookie) = headers.get("cookie") {
        if let Ok(cookie_str) = session_cookie.to_str() {
            if let Some(session_id) = extract_session_id(cookie_str) {
                let session_data = {
                    let sessions = state.sessions.read().await;
                    sessions.get(&session_id).cloned()
                };

                if let Some(session_data) = session_data {
                    let client_id = std::env::var("GITHUB_CLIENT_ID")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    let client_secret = std::env::var("GITHUB_CLIENT_SECRET")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    let redirect_url = std::env::var("GITHUB_REDIRECT_URL")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                    let github_client = BasicClient::new(
                        ClientId::new(client_id),
                        Some(ClientSecret::new(client_secret)),
                        AuthUrl::new("https://github.com/login/oauth/authorize".to_string()).unwrap(),
                        Some(TokenUrl::new("https://github.com/login/oauth/access_token".to_string()).unwrap()),
                    )
                    .set_redirect_uri(RedirectUrl::new(redirect_url).unwrap());

                    let code = AuthorizationCode::new(code.clone());
                    let pkce_verifier = session_data.pkce_verifier.as_ref()
                        .map(|v| oauth2::PkceCodeVerifier::new(v.clone()));

                    let mut token_request = github_client.exchange_code(code);
                    if let Some(verifier) = pkce_verifier {
                        token_request = token_request.set_pkce_verifier(verifier);
                    }

                    let token_result = token_request
                        .request_async(oauth2::reqwest::async_http_client)
                        .await
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                    let access_token = token_result.access_token().secret().clone();

                    // Get user info from GitHub
                    let client = reqwest::Client::new();
                    let user_info = client
                        .get("https://api.github.com/user")
                        .header("Authorization", format!("Bearer {}", access_token))
                        .header("User-Agent", "Blinkout")
                        .send()
                        .await
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                        .json::<GitHubUser>()
                        .await
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                    // Update session with user data
                    let mut sessions = state.sessions.write().await;
                    sessions.insert(session_id.clone(), SessionData {
                        auth_provider: AuthProvider::GitHub,
                        user_did: format!("github:{}", user_info.id),
                        handle: user_info.login,
                        display_name: user_info.name,
                        access_token,
                        pkce_verifier: None,
                        oauth_endpoints: None,
                        dpop_private_key: None,
                    });

                    return Ok(Redirect::to("/protected").into_response());
                }
            }
        }
    }

    Ok(Redirect::to("/").into_response())
}

// Google OAuth handlers
async fn google_login(State(state): State<AppState>) -> Result<Response, StatusCode> {
    let client_id = std::env::var("GOOGLE_CLIENT_ID")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let redirect_url = std::env::var("GOOGLE_REDIRECT_URL")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let google_client = BasicClient::new(
        ClientId::new(client_id),
        None,
        AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string()).unwrap(),
        Some(TokenUrl::new("https://oauth2.googleapis.com/token".to_string()).unwrap()),
    )
    .set_redirect_uri(RedirectUrl::new(redirect_url).unwrap());

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let csrf_state = CsrfToken::new_random();

    let (auth_url, _) = google_client
        .authorize_url(|| csrf_state.clone())
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    let session_id = Uuid::new_v4().to_string();
    let mut sessions = state.sessions.write().await;
    sessions.insert(session_id.clone(), SessionData {
        auth_provider: AuthProvider::Google,
        user_did: "".to_string(),
        handle: "".to_string(),
        display_name: None,
        access_token: "".to_string(),
        pkce_verifier: Some(pkce_verifier.secret().clone()),
        oauth_endpoints: None,
        dpop_private_key: None,
    });

    Ok((StatusCode::FOUND, [
        (header::SET_COOKIE, format!("session_id={}; Path=/; HttpOnly; SameSite=Lax", session_id)),
        (header::LOCATION, auth_url.to_string()),
    ]).into_response())
}

#[derive(Deserialize)]
struct GoogleUserInfo {
    sub: String,
    name: Option<String>,
    email: Option<String>,
}

async fn google_callback(
    State(state): State<AppState>,
    Query(query): Query<AuthCallback>,
    headers: axum::http::HeaderMap,
) -> Result<Response, StatusCode> {
    // Check for OAuth errors
    if let Some(error) = &query.error {
        let error_msg = query.error_description.as_deref().unwrap_or("Unknown error");
        tracing::error!("Google OAuth callback error: {} - {}", error, error_msg);
        return Ok(Redirect::to("/?error=google_auth_failed").into_response());
    }

    let code = query.code.as_ref().ok_or(StatusCode::BAD_REQUEST)?;

    if let Some(session_cookie) = headers.get("cookie") {
        if let Ok(cookie_str) = session_cookie.to_str() {
            if let Some(session_id) = extract_session_id(cookie_str) {
                let session_data = {
                    let sessions = state.sessions.read().await;
                    sessions.get(&session_id).cloned()
                };

                if let Some(session_data) = session_data {
                    let client_id = std::env::var("GOOGLE_CLIENT_ID")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    let client_secret = std::env::var("GOOGLE_CLIENT_SECRET")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    let redirect_url = std::env::var("GOOGLE_REDIRECT_URL")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                    let google_client = BasicClient::new(
                        ClientId::new(client_id),
                        Some(ClientSecret::new(client_secret)),
                        AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string()).unwrap(),
                        Some(TokenUrl::new("https://oauth2.googleapis.com/token".to_string()).unwrap()),
                    )
                    .set_redirect_uri(RedirectUrl::new(redirect_url).unwrap());

                    let code = AuthorizationCode::new(code.clone());
                    let pkce_verifier = session_data.pkce_verifier.as_ref()
                        .map(|v| oauth2::PkceCodeVerifier::new(v.clone()));

                    let mut token_request = google_client.exchange_code(code);
                    if let Some(verifier) = pkce_verifier {
                        token_request = token_request.set_pkce_verifier(verifier);
                    }

                    let token_result = token_request
                        .request_async(oauth2::reqwest::async_http_client)
                        .await
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                    let access_token = token_result.access_token().secret().clone();

                    // Get user info from Google
                    let client = reqwest::Client::new();
                    let user_info = client
                        .get("https://www.googleapis.com/oauth2/v3/userinfo")
                        .header("Authorization", format!("Bearer {}", access_token))
                        .send()
                        .await
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                        .json::<GoogleUserInfo>()
                        .await
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                    // Update session with user data
                    let mut sessions = state.sessions.write().await;
                    sessions.insert(session_id.clone(), SessionData {
                        auth_provider: AuthProvider::Google,
                        user_did: format!("google:{}", user_info.sub),
                        handle: user_info.email.unwrap_or_else(|| user_info.sub.clone()),
                        display_name: user_info.name,
                        access_token,
                        pkce_verifier: None,
                        oauth_endpoints: None,
                        dpop_private_key: None,
                    });

                    return Ok(Redirect::to("/protected").into_response());
                }
            }
        }
    }

    Ok(Redirect::to("/").into_response())
}

// LinkedIn OAuth handlers
async fn linkedin_login(State(state): State<AppState>) -> Result<Response, StatusCode> {
    let client_id = std::env::var("LINKEDIN_CLIENT_ID")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let redirect_url = std::env::var("LINKEDIN_REDIRECT_URL")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let linkedin_client = BasicClient::new(
        ClientId::new(client_id),
        None,
        AuthUrl::new("https://www.linkedin.com/oauth/v2/authorization".to_string()).unwrap(),
        Some(TokenUrl::new("https://www.linkedin.com/oauth/v2/accessToken".to_string()).unwrap()),
    )
    .set_redirect_uri(RedirectUrl::new(redirect_url).unwrap());

    let csrf_state = CsrfToken::new_random();

    let (auth_url, _) = linkedin_client
        .authorize_url(|| csrf_state.clone())
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .url();

    let session_id = Uuid::new_v4().to_string();
    let mut sessions = state.sessions.write().await;
    sessions.insert(session_id.clone(), SessionData {
        auth_provider: AuthProvider::LinkedIn,
        user_did: "".to_string(),
        handle: "".to_string(),
        display_name: None,
        access_token: "".to_string(),
        pkce_verifier: None,
        oauth_endpoints: None,
        dpop_private_key: None,
    });

    Ok((StatusCode::FOUND, [
        (header::SET_COOKIE, format!("session_id={}; Path=/; HttpOnly; SameSite=Lax", session_id)),
        (header::LOCATION, auth_url.to_string()),
    ]).into_response())
}

#[derive(Deserialize)]
struct LinkedInUserInfo {
    sub: String,
    name: Option<String>,
    email: Option<String>,
}

async fn linkedin_callback(
    State(state): State<AppState>,
    Query(query): Query<AuthCallback>,
    headers: axum::http::HeaderMap,
) -> Result<Response, StatusCode> {
    // Check for OAuth errors
    if let Some(error) = &query.error {
        let error_msg = query.error_description.as_deref().unwrap_or("Unknown error");
        tracing::error!("LinkedIn OAuth callback error: {} - {}", error, error_msg);
        return Ok(Redirect::to("/?error=linkedin_auth_failed").into_response());
    }

    let code = query.code.as_ref().ok_or(StatusCode::BAD_REQUEST)?;

    if let Some(session_cookie) = headers.get("cookie") {
        if let Ok(cookie_str) = session_cookie.to_str() {
            if let Some(session_id) = extract_session_id(cookie_str) {
                let session_data = {
                    let sessions = state.sessions.read().await;
                    sessions.get(&session_id).cloned()
                };

                if let Some(session_data) = session_data {
                    let client_id = std::env::var("LINKEDIN_CLIENT_ID")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    let client_secret = std::env::var("LINKEDIN_CLIENT_SECRET")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    let redirect_url = std::env::var("LINKEDIN_REDIRECT_URL")
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                    // LinkedIn requires manual token exchange with explicit parameters
                    let token_params = [
                        ("grant_type", "authorization_code"),
                        ("code", code.as_str()),
                        ("redirect_uri", redirect_url.as_str()),
                        ("client_id", client_id.as_str()),
                        ("client_secret", client_secret.as_str()),
                    ];

                    let client = reqwest::Client::new();
                    let token_response = client
                        .post("https://www.linkedin.com/oauth/v2/accessToken")
                        .form(&token_params)
                        .send()
                        .await
                        .map_err(|e| {
                            tracing::error!("LinkedIn token request failed: {:?}", e);
                            StatusCode::INTERNAL_SERVER_ERROR
                        })?;

                    let token_status = token_response.status();
                    let token_text = token_response.text().await.map_err(|e| {
                        tracing::error!("Failed to read token response: {:?}", e);
                        StatusCode::INTERNAL_SERVER_ERROR
                    })?;

                    tracing::info!("LinkedIn token response (status {}): {}", token_status, token_text);

                    let token_data: serde_json::Value = serde_json::from_str(&token_text)
                        .map_err(|e| {
                            tracing::error!("Failed to parse token response: {:?}", e);
                            StatusCode::INTERNAL_SERVER_ERROR
                        })?;

                    let access_token = token_data
                        .get("access_token")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            tracing::error!("No access_token in response: {}", token_text);
                            StatusCode::INTERNAL_SERVER_ERROR
                        })?
                        .to_string();

                    // Get user info from LinkedIn
                    let client = reqwest::Client::new();
                    let response = client
                        .get("https://api.linkedin.com/v2/userinfo")
                        .header("Authorization", format!("Bearer {}", access_token))
                        .send()
                        .await
                        .map_err(|e| {
                            tracing::error!("LinkedIn userinfo request failed: {:?}", e);
                            StatusCode::INTERNAL_SERVER_ERROR
                        })?;

                    let status = response.status();
                    let response_text = response.text().await.map_err(|e| {
                        tracing::error!("Failed to read LinkedIn response: {:?}", e);
                        StatusCode::INTERNAL_SERVER_ERROR
                    })?;

                    tracing::info!("LinkedIn userinfo response (status {}): {}", status, response_text);

                    let user_info: LinkedInUserInfo = serde_json::from_str(&response_text)
                        .map_err(|e| {
                            tracing::error!("Failed to parse LinkedIn userinfo: {:?}", e);
                            StatusCode::INTERNAL_SERVER_ERROR
                        })?;

                    // Update session with user data
                    let mut sessions = state.sessions.write().await;
                    sessions.insert(session_id.clone(), SessionData {
                        auth_provider: AuthProvider::LinkedIn,
                        user_did: format!("linkedin:{}", user_info.sub),
                        handle: user_info.email.clone().unwrap_or_else(|| user_info.sub.clone()),
                        display_name: user_info.name,
                        access_token,
                        pkce_verifier: None,
                        oauth_endpoints: None,
                        dpop_private_key: None,
                    });

                    return Ok(Redirect::to("/protected").into_response());
                }
            }
        }
    }

    Ok(Redirect::to("/").into_response())
}