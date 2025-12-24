use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use std::sync::Arc;

use crate::service::AuthService;

/// Middleware to require authentication
pub async fn require_auth(
    State(auth_service): State<Arc<AuthService>>,
    request: Request,
    next: Next,
) -> Response {
    // Skip authentication if disabled
    if !auth_service.is_enabled() {
        return next.run(request).await;
    }
    
    // Extract session ID from cookies
    let session_id = request
        .headers()
        .get("cookie")
        .and_then(|cookie| cookie.to_str().ok())
        .and_then(|cookie_str| extract_session_id(cookie_str, &auth_service.config.session.cookie_name));
    
    if let Some(session_id) = session_id {
        // Validate session
        match auth_service.validate_session(&session_id).await {
            Ok(_session_data) => {
                // Session is valid, continue
                return next.run(request).await;
            }
            Err(e) => {
                tracing::debug!("Session validation failed: {}", e);
            }
        }
    }
    
    // No valid session, redirect to login
    Redirect::to("/auth/login").into_response()
}

/// Middleware to optionally extract authentication info without requiring it
pub async fn optional_auth(
    State(auth_service): State<Arc<AuthService>>,
    mut request: Request,
    next: Next,
) -> Response {
    // Skip if authentication is disabled
    if !auth_service.is_enabled() {
        return next.run(request).await;
    }
    
    // Try to extract and validate session, but don't fail if it doesn't exist
    let session_id = request
        .headers()
        .get("cookie")
        .and_then(|cookie| cookie.to_str().ok())
        .and_then(|cookie_str| extract_session_id(cookie_str, &auth_service.config.session.cookie_name));
    
    if let Some(session_id) = session_id {
        if let Ok(session_data) = auth_service.validate_session(&session_id).await {
            // Store session data in request extensions for handlers to use
            request.extensions_mut().insert(session_data);
        }
    }
    
    next.run(request).await
}

fn extract_session_id(cookie_str: &str, cookie_name: &str) -> Option<String> {
    cookie_str
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
