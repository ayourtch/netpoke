use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Router, Form,
};
use axum_extra::extract::cookie::{Cookie, PrivateCookieJar};
use serde::Deserialize;
use uuid::Uuid;

use crate::AuthState;
use crate::session::SessionData;
use crate::views::login_page_html;

/// Cookie name for storing OAuth temp state ID during OAuth flow
const OAUTH_STATE_COOKIE: &str = "oauth_state_id";

#[derive(Deserialize)]
struct HandleForm {
    handle: String,
}

#[derive(Deserialize)]
struct PlainLoginForm {
    username: String,
    password: String,
}

#[derive(Deserialize)]
struct AuthCallback {
    code: Option<String>,
    #[allow(dead_code)]
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

/// Create authentication routes
pub fn auth_routes() -> Router<AuthState> {
    Router::new()
        .route("/login", get(login_page))
        .route("/plain/login", post(plain_login))
        .route("/bluesky/login", post(bluesky_login))
        .route("/bluesky/callback", get(bluesky_callback))
        .route("/github/login", get(github_login))
        .route("/github/callback", get(github_callback))
        .route("/google/login", get(google_login))
        .route("/google/callback", get(google_callback))
        .route("/linkedin/login", get(linkedin_login))
        .route("/linkedin/callback", get(linkedin_callback))
        .route("/logout", post(logout))
}

async fn login_page(State(auth_state): State<AuthState>) -> Html<String> {
    let html = login_page_html(
        auth_state.config.plain_login.enabled,
        auth_state.config.oauth.enable_bluesky,
        auth_state.config.oauth.enable_github,
        auth_state.config.oauth.enable_google,
        auth_state.config.oauth.enable_linkedin,
    );
    Html(html)
}

/// Helper to create session cookie
fn create_session_cookie(auth_state: &AuthState, session_data: &SessionData) -> Cookie<'static> {
    let session_json = serde_json::to_string(session_data).unwrap_or_default();
    let cookie_name = auth_state.config.session.cookie_name.clone();
    let mut cookie = Cookie::new(cookie_name, session_json);
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_same_site(axum_extra::extract::cookie::SameSite::Lax);
    if auth_state.config.session.secure {
        cookie.set_secure(true);
    }
    // Set max age based on session timeout
    cookie.set_max_age(time::Duration::seconds(auth_state.config.session.timeout_seconds as i64));
    cookie
}

/// Helper to create OAuth state cookie (temporary, for OAuth flow)
fn create_oauth_state_cookie(state_id: &str, secure: bool) -> Cookie<'static> {
    let mut cookie = Cookie::new(OAUTH_STATE_COOKIE.to_string(), state_id.to_string());
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_same_site(axum_extra::extract::cookie::SameSite::Lax);
    if secure {
        cookie.set_secure(true);
    }
    // OAuth state expires after 10 minutes
    cookie.set_max_age(time::Duration::seconds(600));
    cookie
}

async fn plain_login(
    State(auth_state): State<AuthState>,
    jar: PrivateCookieJar,
    Form(form): Form<PlainLoginForm>,
) -> Result<(PrivateCookieJar, Response), StatusCode> {
    tracing::info!("Plain login request for username: {}", form.username);
    
    let session_data = auth_state
        .authenticate_plain_login(&form.username, &form.password)
        .await
        .map_err(|e| {
            tracing::error!("Plain login failed: {}", e);
            StatusCode::UNAUTHORIZED
        })?;
    
    let cookie = create_session_cookie(&auth_state, &session_data);
    let updated_jar = jar.add(cookie);
    
    Ok((
        updated_jar,
        (StatusCode::FOUND, [(header::LOCATION, "/".to_string())]).into_response(),
    ))
}

async fn bluesky_login(
    State(auth_state): State<AuthState>,
    jar: PrivateCookieJar,
    Form(form): Form<HandleForm>,
) -> Result<(PrivateCookieJar, Response), StatusCode> {
    tracing::info!("Bluesky login request for handle: {}", form.handle);
    
    let (auth_url, temp_state) = auth_state
        .start_bluesky_auth(&form.handle)
        .await
        .map_err(|e| {
            tracing::error!("Bluesky auth start failed: {}", e);
            StatusCode::from(e)
        })?;
    
    // Store temp state in memory with a unique ID
    let state_id = Uuid::new_v4().to_string();
    auth_state.store_oauth_temp_state(state_id.clone(), temp_state).await;
    
    // Store state ID in cookie for callback
    let state_cookie = create_oauth_state_cookie(&state_id, auth_state.config.session.secure);
    let updated_jar = jar.add(state_cookie);
    
    let response = serde_json::json!({
        "auth_url": auth_url
    });
    
    Ok((
        updated_jar,
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json".to_string())],
            serde_json::to_string(&response).unwrap(),
        ).into_response(),
    ))
}

async fn bluesky_callback(
    State(auth_state): State<AuthState>,
    jar: PrivateCookieJar,
    Query(query): Query<AuthCallback>,
) -> Result<(PrivateCookieJar, Response), StatusCode> {
    if let Some(error) = &query.error {
        let error_msg = query.error_description.as_deref().unwrap_or("Unknown error");
        tracing::error!("Bluesky OAuth error: {} - {}", error, error_msg);
        return Ok((jar, Redirect::to("/auth/login?error=bluesky_auth_failed").into_response()));
    }
    
    let code = query.code.as_ref().ok_or(StatusCode::BAD_REQUEST)?;
    
    // Get OAuth state ID from cookie
    let state_id = jar.get(OAUTH_STATE_COOKIE)
        .map(|c| c.value().to_string())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    let temp_state = auth_state
        .get_oauth_temp_state(&state_id)
        .await
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    let session_data = auth_state
        .complete_bluesky_auth(code, &temp_state)
        .await
        .map_err(|e| {
            tracing::error!("Bluesky auth completion failed: {}", e);
            StatusCode::from(e)
        })?;
    
    // Clean up temp state
    auth_state.remove_oauth_temp_state(&state_id).await;
    
    // Store session in private cookie
    let session_cookie = create_session_cookie(&auth_state, &session_data);
    let updated_jar = jar.remove(Cookie::from(OAUTH_STATE_COOKIE)).add(session_cookie);
    
    Ok((updated_jar, Redirect::to("/").into_response()))
}

async fn github_login(
    State(auth_state): State<AuthState>,
    jar: PrivateCookieJar,
) -> Result<(PrivateCookieJar, Response), StatusCode> {
    let (auth_url, temp_state) = auth_state
        .start_github_auth()
        .await
        .map_err(|e| StatusCode::from(e))?;
    
    // Store temp state in memory with a unique ID
    let state_id = Uuid::new_v4().to_string();
    auth_state.store_oauth_temp_state(state_id.clone(), temp_state).await;
    
    // Store state ID in cookie for callback
    let state_cookie = create_oauth_state_cookie(&state_id, auth_state.config.session.secure);
    let updated_jar = jar.add(state_cookie);
    
    Ok((
        updated_jar,
        (StatusCode::FOUND, [(header::LOCATION, auth_url)]).into_response(),
    ))
}

async fn github_callback(
    State(auth_state): State<AuthState>,
    jar: PrivateCookieJar,
    Query(query): Query<AuthCallback>,
) -> Result<(PrivateCookieJar, Response), StatusCode> {
    if let Some(error) = &query.error {
        tracing::error!("GitHub OAuth error: {}", error);
        return Ok((jar, Redirect::to("/auth/login?error=github_auth_failed").into_response()));
    }
    
    let code = query.code.as_ref().ok_or(StatusCode::BAD_REQUEST)?;
    
    // Get OAuth state ID from cookie
    let state_id = jar.get(OAUTH_STATE_COOKIE)
        .map(|c| c.value().to_string())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    let temp_state = auth_state
        .get_oauth_temp_state(&state_id)
        .await
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    let session_data = auth_state
        .complete_github_auth(code, &temp_state)
        .await
        .map_err(|e| StatusCode::from(e))?;
    
    // Clean up temp state
    auth_state.remove_oauth_temp_state(&state_id).await;
    
    // Store session in private cookie
    let session_cookie = create_session_cookie(&auth_state, &session_data);
    let updated_jar = jar.remove(Cookie::from(OAUTH_STATE_COOKIE)).add(session_cookie);
    
    Ok((updated_jar, Redirect::to("/").into_response()))
}

async fn google_login(
    State(auth_state): State<AuthState>,
    jar: PrivateCookieJar,
) -> Result<(PrivateCookieJar, Response), StatusCode> {
    let (auth_url, temp_state) = auth_state
        .start_google_auth()
        .await
        .map_err(|e| StatusCode::from(e))?;
    
    // Store temp state in memory with a unique ID
    let state_id = Uuid::new_v4().to_string();
    auth_state.store_oauth_temp_state(state_id.clone(), temp_state).await;
    
    // Store state ID in cookie for callback
    let state_cookie = create_oauth_state_cookie(&state_id, auth_state.config.session.secure);
    let updated_jar = jar.add(state_cookie);
    
    Ok((
        updated_jar,
        (StatusCode::FOUND, [(header::LOCATION, auth_url)]).into_response(),
    ))
}

async fn google_callback(
    State(auth_state): State<AuthState>,
    jar: PrivateCookieJar,
    Query(query): Query<AuthCallback>,
) -> Result<(PrivateCookieJar, Response), StatusCode> {
    if let Some(error) = &query.error {
        tracing::error!("Google OAuth error: {}", error);
        return Ok((jar, Redirect::to("/auth/login?error=google_auth_failed").into_response()));
    }
    
    let code = query.code.as_ref().ok_or(StatusCode::BAD_REQUEST)?;
    
    // Get OAuth state ID from cookie
    let state_id = jar.get(OAUTH_STATE_COOKIE)
        .map(|c| c.value().to_string())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    let temp_state = auth_state
        .get_oauth_temp_state(&state_id)
        .await
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    let session_data = auth_state
        .complete_google_auth(code, &temp_state)
        .await
        .map_err(|e| StatusCode::from(e))?;
    
    // Clean up temp state
    auth_state.remove_oauth_temp_state(&state_id).await;
    
    // Store session in private cookie
    let session_cookie = create_session_cookie(&auth_state, &session_data);
    let updated_jar = jar.remove(Cookie::from(OAUTH_STATE_COOKIE)).add(session_cookie);
    
    Ok((updated_jar, Redirect::to("/").into_response()))
}

async fn linkedin_login(
    State(auth_state): State<AuthState>,
    jar: PrivateCookieJar,
) -> Result<(PrivateCookieJar, Response), StatusCode> {
    let (auth_url, temp_state) = auth_state
        .start_linkedin_auth()
        .await
        .map_err(|e| StatusCode::from(e))?;
    
    // Store temp state in memory with a unique ID
    let state_id = Uuid::new_v4().to_string();
    auth_state.store_oauth_temp_state(state_id.clone(), temp_state).await;
    
    // Store state ID in cookie for callback
    let state_cookie = create_oauth_state_cookie(&state_id, auth_state.config.session.secure);
    let updated_jar = jar.add(state_cookie);
    
    Ok((
        updated_jar,
        (StatusCode::FOUND, [(header::LOCATION, auth_url)]).into_response(),
    ))
}

async fn linkedin_callback(
    State(auth_state): State<AuthState>,
    jar: PrivateCookieJar,
    Query(query): Query<AuthCallback>,
) -> Result<(PrivateCookieJar, Response), StatusCode> {
    if let Some(error) = &query.error {
        tracing::error!("LinkedIn OAuth error: {}", error);
        return Ok((jar, Redirect::to("/auth/login?error=linkedin_auth_failed").into_response()));
    }
    
    let code = query.code.as_ref().ok_or(StatusCode::BAD_REQUEST)?;
    
    // Get OAuth state ID from cookie
    let state_id = jar.get(OAUTH_STATE_COOKIE)
        .map(|c| c.value().to_string())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    let temp_state = auth_state
        .get_oauth_temp_state(&state_id)
        .await
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    let session_data = auth_state
        .complete_linkedin_auth(code, &temp_state)
        .await
        .map_err(|e| StatusCode::from(e))?;
    
    // Clean up temp state
    auth_state.remove_oauth_temp_state(&state_id).await;
    
    // Store session in private cookie
    let session_cookie = create_session_cookie(&auth_state, &session_data);
    let updated_jar = jar.remove(Cookie::from(OAUTH_STATE_COOKIE)).add(session_cookie);
    
    Ok((updated_jar, Redirect::to("/").into_response()))
}

async fn logout(
    State(auth_state): State<AuthState>,
    jar: PrivateCookieJar,
) -> Result<(PrivateCookieJar, Response), StatusCode> {
    // Remove session cookie
    let cookie_name = auth_state.config.session.cookie_name.clone();
    let updated_jar = jar.remove(Cookie::from(cookie_name));
    
    Ok((
        updated_jar,
        (StatusCode::FOUND, [(header::LOCATION, "/auth/login".to_string())]).into_response(),
    ))
}
