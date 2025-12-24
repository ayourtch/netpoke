use axum::{
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use std::sync::Arc;
use wifi_verify_auth::AuthService;

/// Middleware to require either regular authentication OR Magic Key survey session
/// This is specifically for the network test page which can be accessed by both authenticated users
/// and surveyors with a Magic Key
pub async fn require_auth_or_survey_session(
    State(auth_service): State<Arc<AuthService>>,
    request: Request,
    next: Next,
) -> Response {
    // Skip authentication if disabled
    if !auth_service.is_enabled() {
        return next.run(request).await;
    }
    
    // Extract both session IDs from cookies
    let cookie_header = request
        .headers()
        .get("cookie")
        .and_then(|cookie| cookie.to_str().ok());
    
    if let Some(cookie_str) = cookie_header {
        // Check for regular authentication session
        if let Some(session_id) = extract_session_id(cookie_str, &auth_service.config.session.cookie_name) {
            match auth_service.validate_session(&session_id).await {
                Ok(session_data) => {
                    // Check if user is in allowed list
                    if auth_service.is_user_allowed(&session_data.handle) {
                        tracing::debug!("Access granted via regular authentication for user: {}", session_data.handle);
                        return next.run(request).await;
                    }
                }
                Err(e) => {
                    tracing::debug!("Regular session validation failed: {}", e);
                }
            }
        }
        
        // Check for survey session (Magic Key)
        if auth_service.config.magic_keys.enabled {
            if let Some(survey_session_id) = extract_session_id(cookie_str, &auth_service.config.magic_keys.survey_cookie_name) {
                // For now, just check if the survey session cookie exists
                // In the future, we'll validate it against a database of active surveys
                tracing::debug!("Access granted via survey session: {}", survey_session_id);
                return next.run(request).await;
            }
        }
    }
    
    // No valid session, redirect to landing page
    tracing::debug!("Access denied - no valid authentication or survey session");
    Redirect::to("/").into_response()
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
