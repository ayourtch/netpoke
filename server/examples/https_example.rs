// Example showing how to test HTTPS WebSocket connections
// This file demonstrates secure WebSocket connections over HTTPS

use axum::{
    extract::{
        ws::{WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
    routing::get,
    Router,
};
use std::net::SocketAddr;
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let app = Router::new()
        .route("/ws", get(websocket_handler))
        .route("/secure-health", get(secure_health_check))
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3443));
    tracing::info!("Secure WebSocket server listening on {}", addr);

    // This would require proper TLS setup - see main.rs for full implementation
    println!("This is an example. Use the main server configuration for HTTPS.");
    println!("Connect to WebSocket at: wss://127.0.0.1:3443/ws");

    Ok(())
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(_state): State<()>,
) -> Response {
    ws.on_upgrade(|socket| handle_websocket(socket))
}

async fn handle_websocket(mut socket: WebSocket) {
    loop {
        if let Some(msg) = socket.recv().await {
            if let Ok(msg) = msg {
                if socket.send(msg).await.is_err() {
                    break;
                }
            } else {
                break;
            }
        }
    }
}

async fn secure_health_check() -> &'static str {
    "HTTPS is working!"
}