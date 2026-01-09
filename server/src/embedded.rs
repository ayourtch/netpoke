//! Embedded static files module
//!
//! This module provides embedded static file serving for the NetPoke server,
//! allowing deployment as a single executable without external static files.

use axum::{
    body::Body,
    extract::Path,
    http::{header, Response, StatusCode},
    response::IntoResponse,
};
use rust_embed::Embed;

/// Embedded static files from the `server/static` directory
#[derive(Embed)]
#[folder = "static"]
pub struct StaticAssets;

/// Serve an embedded static file
pub async fn serve_static(Path(path): Path<String>) -> impl IntoResponse {
    serve_embedded_file(&path)
}

/// Serve an embedded static file from the public directory
pub async fn serve_public(Path(path): Path<String>) -> impl IntoResponse {
    let full_path = format!("public/{}", path);
    serve_embedded_file(&full_path)
}

/// Serve the root index.html
pub async fn serve_index() -> impl IntoResponse {
    serve_embedded_file("public/index.html")
}

/// Helper function to serve an embedded file
fn serve_embedded_file(path: &str) -> Response<Body> {
    match StaticAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(Body::from(content.data.into_owned()))
                .unwrap()
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Not Found"))
            .unwrap(),
    }
}

/// Serve a specific embedded file by exact path
pub fn get_embedded_file(path: &str) -> Option<(Vec<u8>, String)> {
    StaticAssets::get(path).map(|content| {
        let mime = mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string();
        (content.data.into_owned(), mime)
    })
}

/// Create a response for a specific embedded file
pub fn embedded_file_response(path: &str) -> Response<Body> {
    serve_embedded_file(path)
}
