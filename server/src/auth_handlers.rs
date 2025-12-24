use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use wifi_verify_auth::AuthService;

#[derive(Deserialize)]
pub struct MagicKeyRequest {
    magic_key: String,
}

#[derive(Serialize)]
pub struct AuthStatusResponse {
    authenticated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<UserInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stats: Option<StatsInfo>,
}

#[derive(Serialize)]
pub struct UserInfo {
    name: String,
}

#[derive(Serialize)]
pub struct StatsInfo {
    active_surveys: u32,
    total_measurements: u32,
    active_surveyors: u32,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    message: String,
}

/// Check authentication status
pub async fn auth_status(
    State(auth_service): State<Arc<AuthService>>,
    headers: axum::http::HeaderMap,
) -> Json<AuthStatusResponse> {
    // Skip if auth is disabled
    if !auth_service.is_enabled() {
        return Json(AuthStatusResponse {
            authenticated: false,
            user: None,
            stats: None,
        });
    }
    
    // Extract session ID from cookies
    let session_id = extract_session_id(&headers, &auth_service.config.session.cookie_name);
    
    if let Some(session_id) = session_id {
        // Validate session
        if let Ok(session_data) = auth_service.validate_session(&session_id).await {
            // Check if user is allowed
            if auth_service.is_user_allowed(&session_data.handle) {
                return Json(AuthStatusResponse {
                    authenticated: true,
                    user: Some(UserInfo {
                        name: session_data.handle.clone(),
                    }),
                    stats: Some(StatsInfo {
                        active_surveys: 0, // TODO: Implement real stats
                        total_measurements: 0,
                        active_surveyors: 0,
                    }),
                });
            }
        }
    }
    
    Json(AuthStatusResponse {
        authenticated: false,
        user: None,
        stats: None,
    })
}

/// Validate Magic Key and create survey session
pub async fn magic_key_auth(
    State(auth_service): State<Arc<AuthService>>,
    Json(payload): Json<MagicKeyRequest>,
) -> Result<Response, StatusCode> {
    // Check if Magic Key authentication is enabled
    let magic_keys = auth_service.config.magic_keys.magic_keys.clone();
    
    if !auth_service.config.magic_keys.enabled || magic_keys.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }
    
    // Validate the Magic Key
    if !magic_keys.contains(&payload.magic_key) {
        let error = ErrorResponse {
            message: "Invalid Magic Key. Please check your key and try again.".to_string(),
        };
        return Ok((
            StatusCode::UNAUTHORIZED,
            Json(error),
        ).into_response());
    }
    
    // Create a survey session with the Magic Key stored in the session ID format
    // Format: "survey_{magic_key}_{timestamp}_{uuid}"
    // This allows us to validate the session later without a database
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let uuid = Uuid::new_v4();
    let survey_session_id = format!("survey_{}_{}_{}",
        payload.magic_key.replace("-", "_"),
        timestamp,
        uuid
    );
    
    tracing::info!("Magic Key validated: {}", payload.magic_key);
    
    Ok((
        StatusCode::OK,
        [
            (
                header::SET_COOKIE,
                format!(
                    "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
                    auth_service.config.magic_keys.survey_cookie_name,
                    survey_session_id,
                    auth_service.config.magic_keys.survey_timeout_seconds
                ),
            ),
        ],
        Json(serde_json::json!({
            "message": "Magic Key validated successfully"
        })),
    ).into_response())
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
