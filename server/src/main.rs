mod state;
mod signaling;
mod webrtc_manager;
mod data_channels;
mod measurements;
mod dashboard;
mod cleanup;
mod config;
mod auth_handlers;
mod survey_middleware;
mod packet_tracker;
mod icmp_listener;
mod packet_tracking_api;
mod tracking_channel;
mod packet_capture;
mod capture_api;
mod tracing_buffer;
mod tracing_api;

use axum::{Router, routing::{delete, get, post}, extract::State, Json, middleware};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::{trace::TraceLayer, services::ServeDir};
use tower_http::services::ServeFile;
use tracing_subscriber;
use state::AppState;
use common::{DashboardMessage, ClientInfo};
use webrtc::stats::StatsReportType;
use webrtc::ice::candidate::CandidatePairState;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::ice_transport::ice_connection_state::RTCIceConnectionState;
use webrtc::ice_transport::ice_gathering_state::RTCIceGatheringState;
use rustls;
use wifi_verify_auth::{AuthService, auth_routes, require_auth};

use axum::{http::uri::Uri, response::Redirect};
use axum_server::tls_rustls::RustlsConfig;

use axum::routing::IntoMakeService;

use packet_capture::PacketCaptureService;
use tracing_buffer::TracingService;


fn get_make_service(
    app_state: state::AppState, 
    auth_service: Option<Arc<AuthService>>,
    capture_service: Arc<PacketCaptureService>,
    tracing_service: Arc<TracingService>,
) -> IntoMakeService<axum::Router> {
    // Signaling API routes - these need to be accessible by survey users (Magic Key)
    let signaling_routes = Router::new()
        .route("/api/signaling/start", post(signaling::signaling_start))
        .route("/api/signaling/ice", post(signaling::ice_candidate))
        .route("/api/signaling/ice/remote", post(signaling::get_ice_candidates))
        .with_state(app_state.clone());
    
    // Dashboard and admin routes - these should only be accessible by authenticated users
    let dashboard_routes = Router::new()
        .route("/api/dashboard/ws", get(dashboard::dashboard_ws_handler))
        .route("/api/dashboard/debug", get(dashboard_debug))
        .route("/api/diagnostics", get(server_diagnostics))
        .route("/api/clients/{id}", delete(cleanup::cleanup_client_handler))
        .route("/api/tracking/events", get(packet_tracking_api::get_tracked_events))
        .route("/api/tracking/stats", get(packet_tracking_api::get_tracked_stats))
        .with_state(app_state);
    
    // Capture API routes - accessible with hybrid auth (both user and magic key)
    let capture_routes = Router::new()
        .route("/api/capture/download", get(capture_api::download_pcap))
        .route("/api/capture/stats", get(capture_api::capture_stats))
        .route("/api/capture/clear", post(capture_api::clear_capture))
        .with_state(capture_service);
    
    // Tracing API routes - accessible with hybrid auth (both user and magic key)
    let tracing_routes = Router::new()
        .route("/api/tracing/download", get(tracing_api::download_tracing_buffer))
        .route("/api/tracing/stats", get(tracing_api::tracing_stats))
        .route("/api/tracing/clear", post(tracing_api::clear_tracing))
        .with_state(tracing_service);
    
    // Add authentication if enabled
    let app = if let Some(auth_svc) = auth_service {
        if auth_svc.is_enabled() && auth_svc.has_enabled_providers() {
            tracing::info!("Authentication is enabled, adding auth routes and middleware");
            
            // Create auth routes with their own state
            let auth_router = auth_routes().with_state(auth_svc.clone());
            
            // Public API routes (auth status and magic key)
            let public_api = Router::new()
                .route("/api/auth/status", get(auth_handlers::auth_status))
                .route("/api/auth/magic-key", post(auth_handlers::magic_key_auth))
                .with_state(auth_svc.clone());
            
            // Signaling routes with hybrid auth - allow EITHER regular auth OR survey session (Magic Key)
            let hybrid_signaling = signaling_routes
                .route_layer(middleware::from_fn_with_state(
                    auth_svc.clone(),
                    survey_middleware::require_auth_or_survey_session
                ));
            
            // Capture routes with hybrid auth - allow EITHER regular auth OR survey session (Magic Key)
            let hybrid_capture = capture_routes
                .route_layer(middleware::from_fn_with_state(
                    auth_svc.clone(),
                    survey_middleware::require_auth_or_survey_session
                ));
            
            // Tracing routes with hybrid auth - allow EITHER regular auth OR survey session (Magic Key)
            let hybrid_tracing = tracing_routes
                .route_layer(middleware::from_fn_with_state(
                    auth_svc.clone(),
                    survey_middleware::require_auth_or_survey_session
                ));
            
            // Protected dashboard routes - require full authentication
            let protected_dashboard = dashboard_routes
                .route_layer(middleware::from_fn_with_state(
                    auth_svc.clone(),
                    require_auth
                ));
            
            // Protected static files - require authentication
            let protected_static = Router::new()
                .nest_service("/static", ServeDir::new("server/static"))
                .route_layer(middleware::from_fn_with_state(
                    auth_svc.clone(),
                    require_auth
                ));
            
            // Network test page and its dependencies - allow EITHER regular auth OR survey session (Magic Key)
            let nettest_route = Router::new()
                .route_service("/static/nettest.html", ServeFile::new("server/static/nettest.html"))
                .nest_service("/static/lib", ServeDir::new("server/static/lib"))
                .route_layer(middleware::from_fn_with_state(
                    auth_svc.clone(),
                    survey_middleware::require_auth_or_survey_session
                ));
            
            // Combine: auth routes (public) + public API + public static files + nettest (hybrid auth) + signaling (hybrid auth) + capture (hybrid auth) + tracing (hybrid auth) + dashboard (protected) + static (protected)
            Router::new()
                .nest("/auth", auth_router)
                .merge(public_api)
                .route_service("/", ServeFile::new("server/static/public/index.html"))
                .nest_service("/public", ServeDir::new("server/static/public"))
                .route("/health", get(health_check))
                .merge(nettest_route)
                .merge(hybrid_signaling)
                .merge(hybrid_capture)
                .merge(hybrid_tracing)
                .merge(protected_dashboard)
                .merge(protected_static)
                .layer(TraceLayer::new_for_http())
        } else {
            tracing::info!("Authentication is disabled or no providers are enabled");
            Router::new()
                .route("/health", get(health_check))
                .merge(signaling_routes)
                .merge(dashboard_routes)
                .merge(capture_routes)
                .merge(tracing_routes)
                .route_service("/", ServeFile::new("server/static/public/index.html"))
                .nest_service("/public", ServeDir::new("server/static/public"))
                .nest_service("/static", ServeDir::new("server/static"))
                .layer(TraceLayer::new_for_http())
        }
    } else {
        tracing::info!("No authentication service configured");
        Router::new()
            .route("/health", get(health_check))
            .merge(signaling_routes)
            .merge(dashboard_routes)
            .merge(capture_routes)
            .merge(tracing_routes)
            .route_service("/", ServeFile::new("server/static/public/index.html"))
            .nest_service("/public", ServeDir::new("server/static/public"))
            .nest_service("/static", ServeDir::new("server/static"))
            .layer(TraceLayer::new_for_http())
    };

    app.into_make_service()
}

async fn http_server(
    config: config::ServerConfig, 
    app_state: state::AppState, 
    auth_service: Option<Arc<AuthService>>,
    capture_service: Arc<PacketCaptureService>,
    tracing_service: Arc<TracingService>,
) {
    let srv = get_make_service(app_state, auth_service, capture_service, tracing_service);

    let ip_addr = config.host.parse::<std::net::IpAddr>().unwrap_or_else(|e| {
        tracing::warn!("Failed to parse host '{}': {}. Using 0.0.0.0", config.host, e);
        [0, 0, 0, 0].into()
    });
    let addr = SocketAddr::from((ip_addr, config.http_port));
    
    tracing::info!("HTTP server listening on {}", addr);
    axum_server::bind(addr)
        .serve(srv)
        .await
        .unwrap();
}

async fn http_handler(uri: Uri) -> Redirect {
    let uri = format!("https://127.0.0.1:3443{}", uri.path());

    Redirect::temporary(&uri)
}

async fn https_server(
    config: config::ServerConfig, 
    app_state: state::AppState, 
    auth_service: Option<Arc<AuthService>>,
    capture_service: Arc<PacketCaptureService>,
    tracing_service: Arc<TracingService>,
) {
    let srv = get_make_service(app_state, auth_service, capture_service, tracing_service);

    let cert_path = config.ssl_cert_path.as_deref().unwrap_or("server.crt");
    let key_path = config.ssl_key_path.as_deref().unwrap_or("server.key");

    let rustls_config = RustlsConfig::from_pem_file(cert_path, key_path)
        .await
        .unwrap_or_else(|e| {
            tracing::error!(
                "Failed to load SSL certificates (cert: '{}', key: '{}'): {}",
                cert_path,
                key_path,
                e
            );
            panic!("SSL certificate loading failed");
        });

    let ip_addr = config.host.parse::<std::net::IpAddr>().unwrap_or_else(|e| {
        tracing::warn!("Failed to parse host '{}': {}. Using 0.0.0.0", config.host, e);
        [0, 0, 0, 0].into()
    });
    let addr = SocketAddr::from((ip_addr, config.https_port));
    
    tracing::info!("HTTPS server listening on {}", addr);
    axum_server::bind_rustls(addr, rustls_config)
        .serve(srv)
        .await
        .unwrap();
}



#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // https://github.com/snapview/tokio-tungstenite/issues/353
    rustls::crypto::ring::default_provider().install_default().expect("Failed to install default rustls crypto provider");

    // Load configuration
    let config = config::Config::load_or_default();
    
    // Initialize tracing service first
    let tracing_service = Arc::new(TracingService::new(
        config.tracing.max_log_entries,
        config.tracing.enabled,
    ));
    
    // Initialize logging with configured level and optional buffer
    let log_level = config.logging.level.to_lowercase();
    let env_filter = match log_level.as_str() {
        "trace" => tracing::Level::TRACE,
        "debug" => tracing::Level::DEBUG,
        "info" => tracing::Level::INFO,
        "warn" => tracing::Level::WARN,
        "error" => tracing::Level::ERROR,
        _ => tracing::Level::INFO,
    };
    
    if config.tracing.enabled {
        // Initialize with both console output and buffer
        tracing_buffer::init_tracing_with_buffer(env_filter, &tracing_service);
    } else {
        // Standard console-only tracing
        tracing_subscriber::fmt()
            .with_max_level(env_filter)
            .init();
    }

    tracing::info!("Starting WiFi Verify Server");
    tracing::info!("Configuration loaded:");
    tracing::info!("  HTTP enabled: {}, port: {}", config.server.enable_http, config.server.http_port);
    tracing::info!("  HTTPS enabled: {}, port: {}", config.server.enable_https, config.server.https_port);
    tracing::info!("  Host: {}", config.server.host);
    tracing::info!("  Log level: {}", config.logging.level);
    tracing::info!("  CORS enabled: {}", config.security.enable_cors);
    
    // Log tracing buffer status
    if config.tracing.enabled {
        tracing::info!("Tracing buffer enabled:");
        tracing::info!("  Max log entries: {}", config.tracing.max_log_entries);
    } else {
        tracing::info!("Tracing buffer disabled");
    }
    
    // Initialize packet tracker and ICMP listener
    let (app_state, peer_cleanup_rx) = state::AppState::new();
    
    // Spawn the peer connection cleanup task
    // This receives peer connections that failed during signaling and closes them
    // to prevent resource leaks (spawned tasks, UDP sockets, ICE agent loops).
    // Note: This task runs for the lifetime of the server. It will automatically
    // shut down when all senders (cloned into AppState) are dropped during server shutdown.
    tokio::spawn(async move {
        let mut rx = peer_cleanup_rx;
        while let Some(peer) = rx.recv().await {
            tracing::debug!("Cleaning up failed peer connection");
            if let Err(e) = peer.close().await {
                tracing::warn!("Error closing peer connection during cleanup: {}", e);
            } else {
                tracing::debug!("Successfully closed peer connection during cleanup");
            }
        }
        tracing::info!("Peer connection cleanup task shutting down");
    });
    
    // Register ICMP error callback for session-based error tracking and cleanup
    {
        let clients = app_state.clients.clone();
        let icmp_error_callback = Arc::new(move |embedded_info: packet_tracker::EmbeddedUdpInfo| {
            let clients = clients.clone();
            let dest_addr = embedded_info.dest_addr;
            let udp_checksum = embedded_info.udp_checksum;
            let udp_length = embedded_info.udp_length;
            let src_port = embedded_info.src_port;
            tokio::spawn(async move {
                let clients_guard = clients.read().await;
                
                tracing::debug!("ICMP error callback invoked for dest_addr: {}, total sessions: {}", 
                    dest_addr, clients_guard.len());
                
                // Find the specific session with this peer socket address (IP + port)
                let mut target_session = None;
                for (id, session) in clients_guard.iter() {
                    let peer_addr = session.peer_address.lock().await;
                    tracing::debug!("Checking session {}: peer_address = {:?}", id, *peer_addr);
                    if let Some((addr_str, port)) = &*peer_addr {
                        match addr_str.parse::<std::net::IpAddr>() {
                            Ok(peer_ip) => {
                                let peer_socket_addr = std::net::SocketAddr::new(peer_ip, *port);
                                if peer_socket_addr == dest_addr {
                                    target_session = Some((id.clone(), session.clone()));
                                    break;
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to parse peer IP address '{}' for session {}: {}",
                                    addr_str, id, e
                                );
                            }
                        }
                    }
                }
                drop(clients_guard);
                
                // Handle the ICMP error for the found session
                if let Some((session_id, session)) = target_session {
                    let now = std::time::Instant::now();
                    let mut error_count = session.icmp_error_count.lock().await;
                    let mut last_error = session.last_icmp_error.lock().await;
                    
                    // Time-based logic: increment if < 1 second since last error, reset to 1 if > 1 second
                    let count = if let Some(last_time) = *last_error {
                        if now.duration_since(last_time) < std::time::Duration::from_secs(1) {
                            *error_count += 1;
                            *error_count
                        } else {
                            *error_count = 1;
                            1
                        }
                    } else {
                        *error_count = 1;
                        1
                    };
                    
                    *last_error = Some(now);
                    drop(error_count);
                    drop(last_error);
                    
                    tracing::warn!(
                        "Unmatched ICMP error for session {} at address {} (count: {}/5) - ICMP embedded UDP: src_port={}, dest={}, udp_length={}, udp_checksum={:#06x}",
                        session_id, dest_addr, count, src_port, dest_addr, udp_length, udp_checksum
                    );
                    
                    // Cleanup if threshold reached
                    if count >= 5 {
                        tracing::warn!(
                            "ICMP error threshold reached for session {} at address {}, triggering cleanup",
                            session_id, dest_addr
                        );
                        
                        let mut clients_write = clients.write().await;
                        if let Some(session) = clients_write.get(&session_id) {
                            // Close the WebRTC peer connection
                            if let Err(e) = session.peer_connection.close().await {
                                tracing::warn!("Error closing peer connection for {}: {}", session_id, e);
                            } else {
                                tracing::info!("Closed peer connection for {} due to ICMP errors", session_id);
                            }
                        }
                        clients_write.remove(&session_id);
                    }
                } else {
                    tracing::debug!("No session found for peer address {} (ICMP error dropped)", dest_addr);
                }
            });
        });
        
        app_state.packet_tracker.set_icmp_error_callback(icmp_error_callback).await;
        tracing::info!("ICMP error callback registered for session-based cleanup");
    }
    
    // Initialize the global tracking callback for UDP-to-ICMP communication
    let tracking_sender = app_state.tracking_sender.clone();
    tracking_channel::init_tracking_callback(move |dest_addr, udp_length, ttl, cleartext, sent_at, conn_id, udp_checksum| {
        use crate::packet_tracker::UdpPacketInfo;
        use common::SendOptions;
        
        // Only track if TTL is set (indicating traceroute probe)
        if let Some(ttl_value) = ttl {
            let info = UdpPacketInfo {
                dest_addr,
                udp_length,
                cleartext,
                send_options: SendOptions {
                    ttl: Some(ttl_value),
                    df_bit: Some(true),
                    tos: None,
                    flow_label: None,
                    track_for_ms: 5000, // Track for 5 seconds
                },
                sent_at,
                conn_id,
                udp_checksum,
            };
            
            if let Err(e) = tracking_sender.send(info) {
                tracing::error!("Failed to send tracking info: {}", e);
            }
        }
    });
    tracing::info!("Global tracking callback initialized");
    
    icmp_listener::start_icmp_listener(app_state.packet_tracker.clone());
    tracing::info!("Packet tracking and ICMP listener initialized");
    
    // Initialize packet capture service
    let capture_config = packet_capture::CaptureConfig {
        enabled: config.capture.enabled,
        max_packets: config.capture.max_packets,
        snaplen: config.capture.snaplen,
        interface: config.capture.interface.clone(),
        promiscuous: config.capture.promiscuous,
    };
    let capture_service = packet_capture::PacketCaptureService::new(capture_config);
    
    // Start packet capture if enabled
    if config.capture.enabled {
        tracing::info!("Packet capture enabled:");
        tracing::info!("  Max packets: {}", config.capture.max_packets);
        tracing::info!("  Snaplen: {}", config.capture.snaplen);
        tracing::info!("  Interface: {}", if config.capture.interface.is_empty() { "default" } else { &config.capture.interface });
        tracing::info!("  Promiscuous: {}", config.capture.promiscuous);
        packet_capture::start_packet_capture(capture_service.clone());
    } else {
        tracing::info!("Packet capture disabled");
    }
    
    // Initialize authentication service if enabled
    let auth_service = if config.auth.enable_auth {
        tracing::info!("Authentication enabled:");
        tracing::info!("  Bluesky: {}", config.auth.oauth.enable_bluesky);
        tracing::info!("  GitHub: {}", config.auth.oauth.enable_github);
        tracing::info!("  Google: {}", config.auth.oauth.enable_google);
        tracing::info!("  LinkedIn: {}", config.auth.oauth.enable_linkedin);
        
        match AuthService::new(config.auth.clone()).await {
            Ok(svc) => {
                tracing::info!("Authentication service initialized successfully");
                Some(Arc::new(svc))
            }
            Err(e) => {
                tracing::error!("Failed to initialize authentication service: {}", e);
                tracing::warn!("Continuing without authentication");
                None
            }
        }
    } else {
        tracing::info!("Authentication disabled");
        None
    };

    let mut tasks = Vec::new();

    if config.server.enable_http {
        let http_config = config.server.clone();
        let auth_svc = auth_service.clone();
        let app_state_clone = app_state.clone();
        let capture_svc = capture_service.clone();
        let tracing_svc = tracing_service.clone();
        tasks.push(tokio::spawn(async move {
            http_server(http_config, app_state_clone, auth_svc, capture_svc, tracing_svc).await
        }));
    } else {
        tracing::info!("HTTP server disabled in configuration");
    }

    if config.server.enable_https {
        let https_config = config.server.clone();
        let auth_svc = auth_service.clone();
        let app_state_clone = app_state.clone();
        let capture_svc = capture_service.clone();
        let tracing_svc = tracing_service.clone();
        tasks.push(tokio::spawn(async move {
            https_server(https_config, app_state_clone, auth_svc, capture_svc, tracing_svc).await
        }));
    } else {
        tracing::info!("HTTPS server disabled in configuration");
    }

    if tasks.is_empty() {
        tracing::error!("No servers enabled! Please enable at least one server (HTTP or HTTPS) in the configuration.");
        return Err("No servers enabled".into());
    }

    // Wait for all tasks to complete (which they won't unless there's an error)
    for task in tasks {
        let _ = task.await;
    }

    Ok(())
}

async fn health_check() -> &'static str {
    "OK"
}

async fn dashboard_debug(State(state): State<AppState>) -> Json<DashboardMessage> {
    let clients_lock = state.clients.read().await;
    let mut clients_info = Vec::new();

    for (_, session) in clients_lock.iter() {
        let metrics = session.metrics.read().await.clone();
        let connected_at = session.connected_at.elapsed().as_secs();
        let measurement_state = session.measurement_state.read().await;
        let current_seq = measurement_state.probe_seq;

        // Try to get the actual peer (client) address from the selected candidate pair
        let mut peer_address = None;
        let mut peer_port = None;

        // Check connection state before trying to get stats
        use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
        let conn_state = session.peer_connection.connection_state();

        tracing::debug!("Client {} connection state: {:?}", session.id, conn_state);

        // Only try to get stats if connection is established
        if conn_state == RTCPeerConnectionState::Connected {
            let stats_report = session.peer_connection.get_stats().await;
            tracing::debug!("Client {} stats report has {} entries", session.id, stats_report.reports.len());

            // Find the selected candidate pair
            for stat in stats_report.reports.values() {
                if let StatsReportType::CandidatePair(pair) = stat {
                    tracing::debug!("Candidate pair state: {:?}, nominated: {}", pair.state, pair.nominated);

                    // Look for succeeded or in-progress pairs
                    if pair.state == CandidatePairState::Succeeded ||
                       pair.state == CandidatePairState::InProgress && pair.nominated {
                        // Find the remote candidate by ID to get the IP address
                        for candidate_stat in stats_report.reports.values() {
                            if let StatsReportType::RemoteCandidate(candidate) = candidate_stat {
                                if candidate.id == pair.remote_candidate_id {
                                    peer_address = Some(candidate.ip.clone());
                                    peer_port = Some(candidate.port);
                                    tracing::info!("Got client {} address from selected pair: {}:{}",
                                        session.id, candidate.ip, candidate.port);
                                    break;
                                }
                            }
                        }
                        if peer_address.is_some() {
                            break;
                        }
                    }
                }
            }
        }

        // Update stored address if we got a new one from stats
        if let (Some(addr), Some(port)) = (&peer_address, peer_port) {
            let mut stored_peer = session.peer_address.lock().await;
            if stored_peer.as_ref() != Some(&(addr.clone(), port)) {
                tracing::info!("Peer address changed for client {}: {}:{}", session.id, addr, port);
                *stored_peer = Some((addr.clone(), port));
            }
        }

        // Fallback to stored address if stats didn't work this time
        if peer_address.is_none() {
            let stored_peer = session.peer_address.lock().await;
            if let Some((addr, port)) = stored_peer.as_ref() {
                peer_address = Some(addr.clone());
                peer_port = Some(*port);
                tracing::debug!("Using stored peer address for client {}: {}:{}", session.id, addr, port);
            }
        }

        let peer_address_final = peer_address.unwrap_or_else(|| {
            tracing::warn!("No peer address available for client {}", session.id);
            "N/A".to_string()
        });

        clients_info.push(ClientInfo {
            id: session.id.clone(),
            parent_id: session.parent_id.clone(),
            ip_version: session.ip_version.clone(),
            connected_at,
            metrics,
            peer_address: Some(peer_address_final),
            peer_port,
            current_seq,
        });
    }
    drop(clients_lock);

    Json(DashboardMessage {
        clients: clients_info,
    })
}

async fn server_diagnostics(State(state): State<AppState>) -> Json<common::ServerDiagnostics> {
    let server_uptime = state.server_start_time.elapsed().as_secs();
    let clients_lock = state.clients.read().await;
    
    let mut sessions = Vec::new();
    let mut connected_count = 0;
    let mut disconnected_count = 0;
    let mut failed_count = 0;
    
    for (_, session) in clients_lock.iter() {
        let conn_state = session.peer_connection.connection_state();
        let ice_conn_state = session.peer_connection.ice_connection_state();
        let ice_gathering_state = session.peer_connection.ice_gathering_state();
        
        // Count connection states
        match conn_state {
            RTCPeerConnectionState::Connected => connected_count += 1,
            RTCPeerConnectionState::Disconnected => disconnected_count += 1,
            RTCPeerConnectionState::Failed => failed_count += 1,
            _ => {}
        }
        
        // Get peer address
        let peer_addr = session.peer_address.lock().await;
        let (peer_address, peer_port) = peer_addr.as_ref()
            .map(|(addr, port)| (Some(addr.clone()), Some(*port)))
            .unwrap_or((None, None));
        drop(peer_addr);
        
        // Get candidate pairs from stats for all sessions (not just Connected)
        // This is important for diagnosing connection issues
        let mut candidate_pairs = Vec::new();
        let stats_report = session.peer_connection.get_stats().await;
        
        // Build lookup maps for candidates to avoid O(n²) complexity
        let mut local_candidates = std::collections::HashMap::new();
        let mut remote_candidates = std::collections::HashMap::new();
        
        for stat in stats_report.reports.values() {
            match stat {
                StatsReportType::LocalCandidate(candidate) => {
                    local_candidates.insert(
                        candidate.id.clone(),
                        (format!("{:?}", candidate.candidate_type), format!("{}:{}", candidate.ip, candidate.port))
                    );
                }
                StatsReportType::RemoteCandidate(candidate) => {
                    remote_candidates.insert(
                        candidate.id.clone(),
                        (format!("{:?}", candidate.candidate_type), format!("{}:{}", candidate.ip, candidate.port))
                    );
                }
                _ => {}
            }
        }
        
        // Now process candidate pairs with O(1) lookups
        for stat in stats_report.reports.values() {
            if let StatsReportType::CandidatePair(pair) = stat {
                let (local_type, local_address) = local_candidates.get(&pair.local_candidate_id)
                    .map(|(t, a)| (t.clone(), a.clone()))
                    .unwrap_or_else(|| ("unknown".to_string(), "unknown".to_string()));
                
                let (remote_type, remote_address) = remote_candidates.get(&pair.remote_candidate_id)
                    .map(|(t, a)| (t.clone(), a.clone()))
                    .unwrap_or_else(|| ("unknown".to_string(), "unknown".to_string()));
                
                candidate_pairs.push(common::CandidatePairInfo {
                    local_candidate_type: local_type,
                    local_address,
                    remote_candidate_type: remote_type,
                    remote_address,
                    state: format!("{:?}", pair.state),
                    nominated: pair.nominated,
                    bytes_sent: pair.bytes_sent,
                    bytes_received: pair.bytes_received,
                });
            }
        }
        
        // Get data channel status
        let data_channels = session.data_channels.read().await;
        let data_channel_status = common::DataChannelStatus {
            probe: data_channels.probe.as_ref().map(|dc| format!("{:?}", dc.ready_state())),
            bulk: data_channels.bulk.as_ref().map(|dc| format!("{:?}", dc.ready_state())),
            control: data_channels.control.as_ref().map(|dc| format!("{:?}", dc.ready_state())),
            testprobe: data_channels.testprobe.as_ref().map(|dc| format!("{:?}", dc.ready_state())),
        };
        drop(data_channels);
        
        // Get ICMP error info
        let icmp_error_count = *session.icmp_error_count.lock().await;
        let last_icmp_error = session.last_icmp_error.lock().await;
        let last_icmp_error_secs_ago = last_icmp_error.as_ref()
            .map(|t| t.elapsed().as_secs());
        drop(last_icmp_error);
        
        sessions.push(common::SessionDiagnostics {
            session_id: session.id.clone(),
            parent_id: session.parent_id.clone(),
            ip_version: session.ip_version.clone(),
            mode: session.mode.clone(),
            conn_id: session.conn_id.clone(),
            connected_at_secs: session.connected_at.elapsed().as_secs(),
            connection_state: format!("{:?}", conn_state),
            ice_connection_state: format!("{:?}", ice_conn_state),
            ice_gathering_state: format!("{:?}", ice_gathering_state),
            peer_address,
            peer_port,
            candidate_pairs,
            data_channels: data_channel_status,
            icmp_error_count,
            last_icmp_error_secs_ago,
        });
    }
    
    let total_sessions = clients_lock.len();
    drop(clients_lock);
    
    Json(common::ServerDiagnostics {
        server_uptime_secs: server_uptime,
        total_sessions,
        connected_sessions: connected_count,
        disconnected_sessions: disconnected_count,
        failed_sessions: failed_count,
        sessions,
    })
}
