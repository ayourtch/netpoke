use crate::auth_cache::SharedAuthAddressCache;
use axum::{
    extract::{ConnectInfo, FromRef, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use axum_extra::extract::cookie::{Key, PrivateCookieJar};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use uuid::Uuid;
use netpoke_auth::{AuthState, SessionData};

/// Combined state for auth handlers that need both AuthState and auth cache
#[derive(Clone)]
pub struct AuthHandlerState {
    pub auth_state: AuthState,
    pub auth_cache: Option<SharedAuthAddressCache>,
}

/// Implement FromRef to allow PrivateCookieJar to extract Key from AuthHandlerState
impl FromRef<AuthHandlerState> for Key {
    fn from_ref(state: &AuthHandlerState) -> Self {
        state.auth_state.cookie_key()
    }
}

#[derive(Deserialize)]
pub struct MagicKeyRequest {
    magic_key: String,
}

#[derive(Serialize)]
pub struct AuthStatusResponse {
    authenticated: bool,
    /// Type of authentication: "full" for OAuth/password login, "magic_key" for Magic Key access
    #[serde(skip_serializing_if = "Option::is_none")]
    auth_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<UserInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stats: Option<StatsInfo>,
    /// The magic key used for authentication (only present when auth_type is "magic_key")
    #[serde(skip_serializing_if = "Option::is_none")]
    magic_key: Option<String>,
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
        .and_then(|cookie| serde_json::from_str(cookie.value()).ok())
}

/// Collect all cookies from potentially multiple cookie headers
fn collect_all_cookies(headers: &HeaderMap) -> String {
    headers
        .get_all("cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .fold(String::new(), |mut acc, s| {
            if !acc.is_empty() {
                acc.push_str("; ");
            }
            acc.push_str(s);
            acc
        })
}

/// Extract a session ID from a cookie string
fn extract_session_id(cookie_str: &str, cookie_name: &str) -> Option<String> {
    cookie_str.split(';').find_map(|cookie| {
        let parts: Vec<&str> = cookie.trim().split('=').collect();
        if parts.len() == 2 && parts[0] == cookie_name {
            Some(parts[1].to_string())
        } else {
            None
        }
    })
}

/// Validate survey session format and check expiration
/// Session format: "survey_{magic_key}_{timestamp}_{uuid}"
fn validate_survey_session(
    session_id: &str,
    config: &netpoke_auth::config::MagicKeyConfig,
) -> bool {
    // Check if it starts with "survey_"
    if !session_id.starts_with("survey_") {
        return false;
    }

    // Parse the session format
    let parts: Vec<&str> = session_id.split('_').collect();
    if parts.len() < 4 {
        return false;
    }

    // Extract timestamp (second-to-last part before UUID)
    let timestamp_str = parts[parts.len() - 2];
    let timestamp = match timestamp_str.parse::<u64>() {
        Ok(t) => t,
        Err(_) => return false,
    };

    // Check if session has expired
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let elapsed = current_time.saturating_sub(timestamp);
    if elapsed > config.survey_timeout_seconds {
        return false;
    }

    // Extract and validate the Magic Key is still in the allowed list
    let magic_key_parts: Vec<String> = parts[1..parts.len() - 2]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let magic_key = magic_key_parts.join("-");

    if !config.magic_keys.contains(&magic_key) {
        return false;
    }

    true
}

/// Extract the magic key from a survey session ID.
/// Session format: "survey_{magic_key}_{timestamp}_{uuid}"
/// Magic key hyphens are encoded as underscores in the session ID.
fn extract_magic_key_from_session(session_id: &str) -> Option<String> {
    if !session_id.starts_with("survey_") {
        return None;
    }
    let parts: Vec<&str> = session_id.split('_').collect();
    if parts.len() < 4 {
        return None;
    }
    let key_parts = &parts[1..parts.len() - 2];
    if key_parts.is_empty() {
        return None;
    }
    Some(key_parts.join("-"))
}

/// Check authentication status
pub async fn auth_status(
    State(auth_state): State<AuthState>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
) -> Json<AuthStatusResponse> {
    // Skip if auth is disabled
    if !auth_state.is_enabled() {
        return Json(AuthStatusResponse {
            authenticated: false,
            auth_type: None,
            user: None,
            stats: None,
            magic_key: None,
        });
    }

    // Try to extract session from private cookie (full auth takes precedence)
    if let Some(session_data) =
        extract_session_from_jar(&jar, &auth_state.config.session.cookie_name)
    {
        // Validate session
        if auth_state.validate_session(&session_data).is_ok() {
            // Check if user is allowed
            if auth_state.is_user_allowed(&session_data.handle) {
                return Json(AuthStatusResponse {
                    authenticated: true,
                    auth_type: Some("full".to_string()),
                    user: Some(UserInfo {
                        name: session_data.handle.clone(),
                    }),
                    stats: Some(StatsInfo {
                        active_surveys: 0, // TODO: Implement real stats
                        total_measurements: 0,
                        active_surveyors: 0,
                    }),
                    magic_key: None,
                });
            }
        }
    }

    // Check for magic key session (fallback)
    if auth_state.config.magic_keys.enabled {
        let cookie_str = collect_all_cookies(&headers);
        if let Some(survey_session_id) =
            extract_session_id(&cookie_str, &auth_state.config.magic_keys.survey_cookie_name)
        {
            if validate_survey_session(&survey_session_id, &auth_state.config.magic_keys) {
                return Json(AuthStatusResponse {
                    authenticated: true,
                    auth_type: Some("magic_key".to_string()),
                    user: None,
                    stats: None,
                    magic_key: extract_magic_key_from_session(&survey_session_id),
                });
            }
        }
    }

    Json(AuthStatusResponse {
        authenticated: false,
        auth_type: None,
        user: None,
        stats: None,
        magic_key: None,
    })
}
pub async fn auth_status_with_cache(
    State(handler_state): State<AuthHandlerState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    jar: PrivateCookieJar,
) -> Json<AuthStatusResponse> {
    let auth_state = &handler_state.auth_state;

    // Skip if auth is disabled
    if !auth_state.is_enabled() {
        return Json(AuthStatusResponse {
            authenticated: false,
            auth_type: None,
            user: None,
            stats: None,
            magic_key: None,
        });
    }

    // Try to extract session from private cookie (full auth takes precedence)
    if let Some(session_data) =
        extract_session_from_jar(&jar, &auth_state.config.session.cookie_name)
    {
        // Validate session
        if auth_state.validate_session(&session_data).is_ok() {
            // Check if user is allowed
            if auth_state.is_user_allowed(&session_data.handle) {
                // Record the authenticated address to the cache
                if let Some(cache) = &handler_state.auth_cache {
                    cache.record_auth(
                        addr.ip(),
                        session_data.handle.clone(),
                        session_data.display_name.clone(),
                        format!("{:?}", session_data.auth_provider),
                    );
                }

                return Json(AuthStatusResponse {
                    authenticated: true,
                    auth_type: Some("full".to_string()),
                    user: Some(UserInfo {
                        name: session_data.handle.clone(),
                    }),
                    stats: Some(StatsInfo {
                        active_surveys: 0, // TODO: Implement real stats
                        total_measurements: 0,
                        active_surveyors: 0,
                    }),
                    magic_key: None,
                });
            }
        }
    }

    // Check for magic key session (fallback)
    if auth_state.config.magic_keys.enabled {
        let cookie_str = collect_all_cookies(&headers);
        if let Some(survey_session_id) =
            extract_session_id(&cookie_str, &auth_state.config.magic_keys.survey_cookie_name)
        {
            if validate_survey_session(&survey_session_id, &auth_state.config.magic_keys) {
                // Record magic key auth to cache
                if let Some(cache) = &handler_state.auth_cache {
                    cache.record_auth(
                        addr.ip(),
                        "magic_key_user".to_string(),
                        None,
                        "magic_key".to_string(),
                    );
                }

                return Json(AuthStatusResponse {
                    authenticated: true,
                    auth_type: Some("magic_key".to_string()),
                    user: None,
                    stats: None,
                    magic_key: extract_magic_key_from_session(&survey_session_id),
                });
            }
        }
    }

    Json(AuthStatusResponse {
        authenticated: false,
        auth_type: None,
        user: None,
        stats: None,
        magic_key: None,
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
        return Ok((StatusCode::UNAUTHORIZED, Json(error)).into_response());
    }

    // Create a survey session with the Magic Key stored in the session ID format
    // Format: "survey_{magic_key}_{timestamp}_{uuid}"
    // This allows us to validate the session later without a database
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let uuid = Uuid::new_v4();
    let survey_session_id = format!(
        "survey_{}_{}_{}",
        payload.magic_key.replace("-", "_"),
        timestamp,
        uuid
    );

    tracing::info!("Magic Key validated: {}", payload.magic_key);

    Ok((
        StatusCode::OK,
        [(
            header::SET_COOKIE,
            format!(
                "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
                auth_state.config.magic_keys.survey_cookie_name,
                survey_session_id,
                auth_state.config.magic_keys.survey_timeout_seconds
            ),
        )],
        Json(serde_json::json!({
            "message": "Magic Key validated successfully"
        })),
    )
        .into_response())
}

/// Validate Magic Key and create survey session, recording to auth cache
pub async fn magic_key_auth_with_cache(
    State(handler_state): State<AuthHandlerState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<MagicKeyRequest>,
) -> Result<Response, StatusCode> {
    let auth_state = &handler_state.auth_state;

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
        return Ok((StatusCode::UNAUTHORIZED, Json(error)).into_response());
    }

    // Record the authenticated address to the cache
    if let Some(cache) = &handler_state.auth_cache {
        cache.record_auth(
            addr.ip(),
            format!("magic_key:{}", payload.magic_key),
            None,
            "magic_key".to_string(),
        );
    }

    // Create a survey session with the Magic Key stored in the session ID format
    // Format: "survey_{magic_key}_{timestamp}_{uuid}"
    // This allows us to validate the session later without a database
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let uuid = Uuid::new_v4();
    let survey_session_id = format!(
        "survey_{}_{}_{}",
        payload.magic_key.replace("-", "_"),
        timestamp,
        uuid
    );

    tracing::info!(
        "Magic Key validated: {} from {}",
        payload.magic_key,
        addr.ip()
    );

    Ok((
        StatusCode::OK,
        [(
            header::SET_COOKIE,
            format!(
                "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
                auth_state.config.magic_keys.survey_cookie_name,
                survey_session_id,
                auth_state.config.magic_keys.survey_timeout_seconds
            ),
        )],
        Json(serde_json::json!({
            "message": "Magic Key validated successfully"
        })),
    )
        .into_response())
}
