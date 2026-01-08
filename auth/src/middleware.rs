use axum::{
    extract::{FromRequestParts, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::PrivateCookieJar;

use crate::session::SessionData;
use crate::views::access_denied_page_html;
use crate::AuthState;

/// Extract session data from PrivateCookieJar
fn extract_session_from_jar(jar: &PrivateCookieJar, cookie_name: &str) -> Option<SessionData> {
    jar.get(cookie_name)
        .and_then(|cookie| serde_json::from_str(cookie.value()).ok())
}

/// Middleware to require authentication
pub async fn require_auth(
    State(auth_state): State<AuthState>,
    request: Request,
    next: Next,
) -> Response {
    // Skip authentication if disabled
    if !auth_state.is_enabled() {
        return next.run(request).await;
    }

    // Extract PrivateCookieJar from request
    let (mut parts, body) = request.into_parts();
    let jar = match PrivateCookieJar::from_request_parts(&mut parts, &auth_state).await {
        Ok(jar) => jar,
        Err(_) => {
            return Redirect::to("/auth/login").into_response();
        }
    };

    // Try to extract session from private cookie
    if let Some(session_data) =
        extract_session_from_jar(&jar, &auth_state.config.session.cookie_name)
    {
        // Validate session (check expiration)
        if auth_state.validate_session(&session_data).is_ok() {
            // Check if user is in allowed list
            if !auth_state.is_user_allowed(&session_data.handle) {
                tracing::warn!("Access denied for user: {}", session_data.handle);
                let html = access_denied_page_html(&session_data.handle);
                return (StatusCode::FORBIDDEN, Html(html)).into_response();
            }

            // Session is valid and user is allowed, continue
            let mut request = Request::from_parts(parts, body);
            request.extensions_mut().insert(session_data);
            return next.run(request).await;
        }
    }

    // No valid session, redirect to login
    Redirect::to("/auth/login").into_response()
}

/// Middleware to optionally extract authentication info without requiring it
pub async fn optional_auth(
    State(auth_state): State<AuthState>,
    request: Request,
    next: Next,
) -> Response {
    // Skip if authentication is disabled
    if !auth_state.is_enabled() {
        return next.run(request).await;
    }

    // Extract PrivateCookieJar from request
    let (mut parts, body) = request.into_parts();
    let jar = match PrivateCookieJar::from_request_parts(&mut parts, &auth_state).await {
        Ok(jar) => jar,
        Err(_) => {
            let request = Request::from_parts(parts, body);
            return next.run(request).await;
        }
    };

    // Try to extract session from private cookie
    let mut request = Request::from_parts(parts, body);
    if let Some(session_data) =
        extract_session_from_jar(&jar, &auth_state.config.session.cookie_name)
    {
        // Validate session (check expiration)
        if auth_state.validate_session(&session_data).is_ok() {
            // Store session data in request extensions for handlers to use
            request.extensions_mut().insert(session_data);
        }
    }

    next.run(request).await
}
