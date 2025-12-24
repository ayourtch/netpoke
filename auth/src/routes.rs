use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Router, Form,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::service::AuthService;
use crate::views::login_page_html;

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
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

/// Create authentication routes
pub fn auth_routes() -> Router<Arc<AuthService>> {
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

async fn login_page(State(auth_service): State<Arc<AuthService>>) -> Html<String> {
    let html = login_page_html(
        auth_service.config.plain_login.enabled,
        auth_service.config.oauth.enable_bluesky,
        auth_service.config.oauth.enable_github,
        auth_service.config.oauth.enable_google,
        auth_service.config.oauth.enable_linkedin,
    );
    Html(html)
}

async fn plain_login(
    State(auth_service): State<Arc<AuthService>>,
    Form(form): Form<PlainLoginForm>,
) -> Result<Response, StatusCode> {
    tracing::info!("Plain login request for username: {}", form.username);
    
    let session_data = auth_service
        .authenticate_plain_login(&form.username, &form.password)
        .await
        .map_err(|e| {
            tracing::error!("Plain login failed: {}", e);
            StatusCode::UNAUTHORIZED
        })?;
    
    let session_id = Uuid::new_v4().to_string();
    auth_service.store_session(session_id.clone(), session_data).await;
    
    Ok((
        StatusCode::FOUND,
        [
            (
                header::SET_COOKIE,
                format!(
                    "{}={}; Path=/; HttpOnly; SameSite=Lax{}",
                    auth_service.config.session.cookie_name,
                    session_id,
                    if auth_service.config.session.secure { "; Secure" } else { "" }
                ),
            ),
            (header::LOCATION, "/".to_string()),
        ],
    )
        .into_response())
}

async fn bluesky_login(
    State(auth_service): State<Arc<AuthService>>,
    Form(form): Form<HandleForm>,
) -> Result<Response, StatusCode> {
    tracing::info!("Bluesky login request for handle: {}", form.handle);
    
    let (auth_url, session_data) = auth_service
        .start_bluesky_auth(&form.handle)
        .await
        .map_err(|e| {
            tracing::error!("Bluesky auth start failed: {}", e);
            StatusCode::from(e)
        })?;
    
    let session_id = Uuid::new_v4().to_string();
    auth_service.store_session(session_id.clone(), session_data).await;
    
    let response = serde_json::json!({
        "auth_url": auth_url
    });
    
    Ok((
        StatusCode::OK,
        [
            (
                header::SET_COOKIE,
                format!(
                    "{}={}; Path=/; HttpOnly; SameSite=Lax{}",
                    auth_service.config.session.cookie_name,
                    session_id,
                    if auth_service.config.session.secure { "; Secure" } else { "" }
                ),
            ),
            (header::CONTENT_TYPE, "application/json".to_string()),
        ],
        serde_json::to_string(&response).unwrap(),
    )
        .into_response())
}

async fn bluesky_callback(
    State(auth_service): State<Arc<AuthService>>,
    Query(query): Query<AuthCallback>,
    headers: axum::http::HeaderMap,
) -> Result<Response, StatusCode> {
    if let Some(error) = &query.error {
        let error_msg = query.error_description.as_deref().unwrap_or("Unknown error");
        tracing::error!("Bluesky OAuth error: {} - {}", error, error_msg);
        return Ok(Redirect::to("/auth/login?error=bluesky_auth_failed").into_response());
    }
    
    let code = query.code.as_ref().ok_or(StatusCode::BAD_REQUEST)?;
    
    let session_id = extract_session_id(&headers, &auth_service.config.session.cookie_name)
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    let session_data = auth_service
        .get_session(&session_id)
        .await
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    let updated_session = auth_service
        .complete_bluesky_auth(code, &session_data)
        .await
        .map_err(|e| {
            tracing::error!("Bluesky auth completion failed: {}", e);
            StatusCode::from(e)
        })?;
    
    auth_service.store_session(session_id, updated_session).await;
    
    Ok(Redirect::to("/").into_response())
}

async fn github_login(
    State(auth_service): State<Arc<AuthService>>,
) -> Result<Response, StatusCode> {
    let (auth_url, session_data) = auth_service
        .start_github_auth()
        .await
        .map_err(|e| StatusCode::from(e))?;
    
    let session_id = Uuid::new_v4().to_string();
    auth_service.store_session(session_id.clone(), session_data).await;
    
    Ok((
        StatusCode::FOUND,
        [
            (
                header::SET_COOKIE,
                format!(
                    "{}={}; Path=/; HttpOnly; SameSite=Lax{}",
                    auth_service.config.session.cookie_name,
                    session_id,
                    if auth_service.config.session.secure { "; Secure" } else { "" }
                ),
            ),
            (header::LOCATION, auth_url),
        ],
    )
        .into_response())
}

async fn github_callback(
    State(auth_service): State<Arc<AuthService>>,
    Query(query): Query<AuthCallback>,
    headers: axum::http::HeaderMap,
) -> Result<Response, StatusCode> {
    if let Some(error) = &query.error {
        tracing::error!("GitHub OAuth error: {}", error);
        return Ok(Redirect::to("/auth/login?error=github_auth_failed").into_response());
    }
    
    let code = query.code.as_ref().ok_or(StatusCode::BAD_REQUEST)?;
    
    let session_id = extract_session_id(&headers, &auth_service.config.session.cookie_name)
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    let session_data = auth_service
        .get_session(&session_id)
        .await
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    let updated_session = auth_service
        .complete_github_auth(code, &session_data)
        .await
        .map_err(|e| StatusCode::from(e))?;
    
    auth_service.store_session(session_id, updated_session).await;
    
    Ok(Redirect::to("/").into_response())
}

async fn google_login(
    State(auth_service): State<Arc<AuthService>>,
) -> Result<Response, StatusCode> {
    let (auth_url, session_data) = auth_service
        .start_google_auth()
        .await
        .map_err(|e| StatusCode::from(e))?;
    
    let session_id = Uuid::new_v4().to_string();
    auth_service.store_session(session_id.clone(), session_data).await;
    
    Ok((
        StatusCode::FOUND,
        [
            (
                header::SET_COOKIE,
                format!(
                    "{}={}; Path=/; HttpOnly; SameSite=Lax{}",
                    auth_service.config.session.cookie_name,
                    session_id,
                    if auth_service.config.session.secure { "; Secure" } else { "" }
                ),
            ),
            (header::LOCATION, auth_url),
        ],
    )
        .into_response())
}

async fn google_callback(
    State(auth_service): State<Arc<AuthService>>,
    Query(query): Query<AuthCallback>,
    headers: axum::http::HeaderMap,
) -> Result<Response, StatusCode> {
    if let Some(error) = &query.error {
        tracing::error!("Google OAuth error: {}", error);
        return Ok(Redirect::to("/auth/login?error=google_auth_failed").into_response());
    }
    
    let code = query.code.as_ref().ok_or(StatusCode::BAD_REQUEST)?;
    
    let session_id = extract_session_id(&headers, &auth_service.config.session.cookie_name)
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    let session_data = auth_service
        .get_session(&session_id)
        .await
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    let updated_session = auth_service
        .complete_google_auth(code, &session_data)
        .await
        .map_err(|e| StatusCode::from(e))?;
    
    auth_service.store_session(session_id, updated_session).await;
    
    Ok(Redirect::to("/").into_response())
}

async fn linkedin_login(
    State(auth_service): State<Arc<AuthService>>,
) -> Result<Response, StatusCode> {
    let (auth_url, session_data) = auth_service
        .start_linkedin_auth()
        .await
        .map_err(|e| StatusCode::from(e))?;
    
    let session_id = Uuid::new_v4().to_string();
    auth_service.store_session(session_id.clone(), session_data).await;
    
    Ok((
        StatusCode::FOUND,
        [
            (
                header::SET_COOKIE,
                format!(
                    "{}={}; Path=/; HttpOnly; SameSite=Lax{}",
                    auth_service.config.session.cookie_name,
                    session_id,
                    if auth_service.config.session.secure { "; Secure" } else { "" }
                ),
            ),
            (header::LOCATION, auth_url),
        ],
    )
        .into_response())
}

async fn linkedin_callback(
    State(auth_service): State<Arc<AuthService>>,
    Query(query): Query<AuthCallback>,
    headers: axum::http::HeaderMap,
) -> Result<Response, StatusCode> {
    if let Some(error) = &query.error {
        tracing::error!("LinkedIn OAuth error: {}", error);
        return Ok(Redirect::to("/auth/login?error=linkedin_auth_failed").into_response());
    }
    
    let code = query.code.as_ref().ok_or(StatusCode::BAD_REQUEST)?;
    
    let session_id = extract_session_id(&headers, &auth_service.config.session.cookie_name)
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    let session_data = auth_service
        .get_session(&session_id)
        .await
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    let updated_session = auth_service
        .complete_linkedin_auth(code, &session_data)
        .await
        .map_err(|e| StatusCode::from(e))?;
    
    auth_service.store_session(session_id, updated_session).await;
    
    Ok(Redirect::to("/").into_response())
}

async fn logout(
    State(auth_service): State<Arc<AuthService>>,
    headers: axum::http::HeaderMap,
) -> Result<Response, StatusCode> {
    if let Some(session_id) = extract_session_id(&headers, &auth_service.config.session.cookie_name) {
        auth_service.remove_session(&session_id).await;
    }
    
    Ok((
        StatusCode::FOUND,
        [
            (
                header::SET_COOKIE,
                format!(
                    "{}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0",
                    auth_service.config.session.cookie_name
                ),
            ),
            (header::LOCATION, "/auth/login".to_string()),
        ],
    )
        .into_response())
}

fn extract_session_id(headers: &axum::http::HeaderMap, cookie_name: &str) -> Option<String> {
    headers
        .get("cookie")?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|cookie| {
            let parts: Vec<&str> = cookie.trim().split('=').collect();
            if parts.len() == 2 && parts[0] == cookie_name {
                Some(parts[1].to_string())
            } else {
                None
            }
        })
}
