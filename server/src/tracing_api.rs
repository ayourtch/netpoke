/// API handlers for tracing buffer functionality
///
/// These endpoints allow downloading captured log messages and viewing tracing statistics.
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use std::sync::Arc;

use crate::tracing_buffer::TracingService;

/// Download tracing buffer as a text file
pub async fn download_tracing_buffer(
    State(tracing_service): State<Arc<TracingService>>,
) -> Response {
    if !tracing_service.is_enabled() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "Tracing buffer is not enabled"
            })),
        )
            .into_response();
    }

    let text_data = tracing_service.export_as_text();
    let stats = tracing_service.stats();

    // Generate filename with timestamp
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("tracing_{}.log", timestamp);

    tracing::info!(
        "Tracing buffer download requested: {} entries, {} bytes",
        stats.entries_in_buffer,
        text_data.len()
    );

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                &format!("attachment; filename=\"{}\"", filename),
            ),
        ],
        text_data,
    )
        .into_response()
}

/// Get tracing buffer statistics
pub async fn tracing_stats(
    State(tracing_service): State<Arc<TracingService>>,
) -> Json<crate::tracing_buffer::TracingStats> {
    Json(tracing_service.stats())
}

/// Clear tracing buffer
pub async fn clear_tracing(State(tracing_service): State<Arc<TracingService>>) -> Response {
    if !tracing_service.is_enabled() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "Tracing buffer is not enabled"
            })),
        )
            .into_response();
    }

    tracing_service.clear();
    tracing::info!("Tracing buffer cleared");

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "message": "Tracing buffer cleared"
        })),
    )
        .into_response()
}
