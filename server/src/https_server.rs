use std::{net::SocketAddr, sync::Arc};
use tokio_rustls::rustls::ServerConfig as RustlsServerConfig;
use tracing;
use axum::Router;
use rustls_pemfile::{certs, pkcs8_private_keys};

use crate::state::AppState;

pub async fn create_tls_config(
    cert_path: &str,
    key_path: &str,
) -> Result<Arc<RustlsServerConfig>, Box<dyn std::error::Error + Send + Sync>> {
    let cert_file = std::fs::File::open(cert_path)
        .map_err(|e| format!("Failed to open certificate file: {}", e))?;
    let key_file = std::fs::File::open(key_path)
        .map_err(|e| format!("Failed to open private key file: {}", e))?;

    let cert_chain = certs(&mut std::io::BufReader::new(cert_file))
        .collect::<Result<Vec<_>, std::io::Error>>()
        .map_err(|e| format!("Failed to parse certificate: {}", e))?;
    
    let mut keys = pkcs8_private_keys(&mut std::io::BufReader::new(key_file))
        .collect::<Result<Vec<_>, std::io::Error>>()
        .map_err(|e| format!("Failed to parse private key: {}", e))?;

    if keys.is_empty() {
        return Err("No private keys found".into());
    }

    let key = keys.remove(0);

    let config = RustlsServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, tokio_rustls::rustls::pki_types::PrivateKeyDer::Pkcs8(key))
        .map_err(|e| format!("Failed to create TLS config: {}", e))?;

    Ok(Arc::new(config))
}

// For now, this will serve as a placeholder until we implement proper HTTPS support
// The current Axum setup doesn't directly support HTTPS without additional dependencies
pub async fn start_https_server(
    _app: Router<AppState>,
    addr: SocketAddr,
    _tls_config: Arc<RustlsServerConfig>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing::info!("HTTPS server functionality is under development");
    tracing::info!("Intended to listen on {}", addr);
    
    // This would need a proper HTTPS implementation
    // For now, we'll just keep the server running
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}