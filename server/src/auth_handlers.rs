use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use axum_extra::extract::cookie::PrivateCookieJar;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use wifi_verify_auth::{AuthState, SessionData};

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

/// Extract session data from PrivateCookieJar
fn extract_session_from_jar(jar: &PrivateCookieJar, cookie_name: &str) -> Option<SessionData> {
    jar.get(cookie_name)
        .and_then(|cookie| {
            serde_json::from_str(cookie.value()).ok()
        })
}

/// Check authentication status
pub async fn auth_status(
    State(auth_state): State<AuthState>,
    jar: PrivateCookieJar,
) -> Json<AuthStatusResponse> {
    // Skip if auth is disabled
    if !auth_state.is_enabled() {
        return Json(AuthStatusResponse {
            authenticated: false,
            user: None,
            stats: None,
        });
    }
    
    // Try to extract session from private cookie
    if let Some(session_data) = extract_session_from_jar(&jar, &auth_state.config.session.cookie_name) {
        // Validate session
        if auth_state.validate_session(&session_data).is_ok() {
            // Check if user is allowed
            if auth_state.is_user_allowed(&session_data.handle) {
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
    State(auth_state): State<AuthState>,
    Json(payload): Json<MagicKeyRequest>,
) -> Result<Response, StatusCode> {
    // Check if Magic Key authentication is enabled
    let magic_keys = auth_state.config.magic_keys.magic_keys.clone();
    
    if !auth_state.config.magic_keys.enabled || magic_keys.is_empty() {
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
                    auth_state.config.magic_keys.survey_cookie_name,
                    survey_session_id,
                    auth_state.config.magic_keys.survey_timeout_seconds
                ),
            ),
        ],
        Json(serde_json::json!({
            "message": "Magic Key validated successfully"
        })),
    ).into_response())
}
