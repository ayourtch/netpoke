//! Main iperf3 server implementation.

use crate::config::Iperf3Config;
use crate::error::{Iperf3Error, Result};
use crate::protocol::{State, TestParameters, COOKIE_SIZE};
use crate::session::TestSession;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, RwLock};

/// Timeout in seconds for waiting for data streams to connect
const STREAM_CONNECT_TIMEOUT_SECS: u64 = 10;

/// Polling interval in milliseconds when waiting for data streams
const STREAM_POLL_INTERVAL_MS: u64 = 50;

/// Delay in milliseconds after test completion to allow remaining data to arrive
const POST_TEST_DELAY_MS: u64 = 100;

/// Callback type for checking if an IP is allowed
pub type AuthCallback = Arc<dyn Fn(IpAddr) -> bool + Send + Sync>;

/// The iperf3 server
pub struct Iperf3Server {
    /// Server configuration
    config: Iperf3Config,

    /// Allowed IP addresses (if require_auth is true)
    allowed_ips: Arc<RwLock<HashMap<IpAddr, ()>>>,

    /// Active sessions
    sessions: Arc<RwLock<HashMap<String, Arc<TestSession>>>>,

    /// Shutdown signal sender
    shutdown_tx: broadcast::Sender<()>,

    /// Optional custom authentication callback
    auth_callback: Arc<RwLock<Option<AuthCallback>>>,
}

impl Iperf3Server {
    /// Create a new iperf3 server
    pub fn new(config: Iperf3Config) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        Self {
            config,
            allowed_ips: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx,
            auth_callback: Arc::new(RwLock::new(None)),
        }
    }

    /// Set a custom authentication callback
    /// The callback receives an IP address and returns true if allowed
    pub async fn set_auth_callback(&self, callback: AuthCallback) {
        *self.auth_callback.write().await = Some(callback);
    }

    /// Add an allowed IP address
    pub async fn add_allowed_ip(&self, ip: IpAddr) {
        self.allowed_ips.write().await.insert(ip, ());
        tracing::info!("Added allowed IP for iperf3: {}", ip);
    }

    /// Remove an allowed IP address
    pub async fn remove_allowed_ip(&self, ip: &IpAddr) {
        self.allowed_ips.write().await.remove(ip);
        tracing::info!("Removed allowed IP for iperf3: {}", ip);
    }

    /// Check if an IP is allowed
    pub async fn is_ip_allowed(&self, ip: IpAddr) -> bool {
        // If auth is not required, allow all
        if !self.config.require_auth {
            return true;
        }

        // Check custom callback first
        if let Some(callback) = self.auth_callback.read().await.as_ref() {
            return callback(ip);
        }

        // Check allowed IPs list
        let allowed = self.allowed_ips.read().await;
        if allowed.is_empty() {
            // If no IPs configured and auth is required, deny all
            return false;
        }
        allowed.contains_key(&ip)
    }

    /// Get the number of active sessions
    pub async fn session_count(&self) -> usize {
        self.sessions.read().await.len()
    }

    /// Shutdown the server
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(());
    }

    /// Run the server
    pub async fn run(&self) -> Result<()> {
        if !self.config.enabled {
            tracing::info!("iperf3 server is disabled");
            return Ok(());
        }

        let addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port)
            .parse()
            .map_err(|e| Iperf3Error::InvalidParameter(format!("Invalid address: {}", e)))?;

        let listener = TcpListener::bind(&addr).await?;
        tracing::info!("iperf3 server listening on {}", addr);

        let mut shutdown_rx = self.shutdown_tx.subscribe();

        loop {
            tokio::select! {
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((stream, peer_addr)) => {
                            // Check if IP is allowed
                            if !self.is_ip_allowed(peer_addr.ip()).await {
                                tracing::warn!("iperf3: Unauthorized connection attempt from {}", peer_addr);
                                // Send access denied and close
                                let _ = self.send_access_denied(stream).await;
                                continue;
                            }

                            // Check session limit
                            if self.session_count().await >= self.config.max_sessions {
                                tracing::warn!("iperf3: Session limit reached, rejecting {}", peer_addr);
                                let _ = self.send_access_denied(stream).await;
                                continue;
                            }

                            tracing::info!("iperf3: New connection from {}", peer_addr);
                            self.handle_connection(stream, peer_addr).await;
                        }
                        Err(e) => {
                            tracing::error!("iperf3: Accept error: {}", e);
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    tracing::info!("iperf3 server shutting down");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Send access denied message
    async fn send_access_denied(&self, mut stream: TcpStream) -> Result<()> {
        stream.write_all(&[State::AccessDenied.to_byte()]).await?;
        Ok(())
    }

    /// Handle a new connection
    async fn handle_connection(&self, stream: TcpStream, peer_addr: SocketAddr) {
        let sessions = self.sessions.clone();
        let max_duration = Duration::from_secs(self.config.max_duration_secs);
        let max_bandwidth = self.config.max_bandwidth;

        tokio::spawn(async move {
            if let Err(e) =
                Self::handle_session(stream, peer_addr, sessions, max_duration, max_bandwidth).await
            {
                tracing::error!("iperf3: Session error for {}: {}", peer_addr, e);
            }
        });
    }

    /// Handle a single session
    async fn handle_session(
        mut stream: TcpStream,
        peer_addr: SocketAddr,
        sessions: Arc<RwLock<HashMap<String, Arc<TestSession>>>>,
        max_duration: Duration,
        max_bandwidth: u64,
    ) -> Result<()> {
        // Step 1: Read the cookie (37 bytes for iperf3)
        let mut cookie_buf = [0u8; COOKIE_SIZE];
        stream.read_exact(&mut cookie_buf).await?;
        let cookie = String::from_utf8_lossy(&cookie_buf)
            .trim_end_matches('\0')
            .to_string();

        tracing::debug!("iperf3: Received cookie: {}", cookie);

        // Check if this is a data stream for an existing session
        {
            let sessions_read = sessions.read().await;
            if let Some(session) = sessions_read.get(&cookie) {
                // This is a data stream connection
                tracing::debug!(
                    "iperf3: Data stream connection for session {}",
                    cookie
                );
                session.add_data_stream(stream).await;
                return Ok(());
            }
        }

        // This is a new control connection
        let session = Arc::new(TestSession::new(cookie.clone(), peer_addr, stream));

        // Store the session
        {
            let mut sessions_write = sessions.write().await;
            sessions_write.insert(cookie.clone(), session.clone());
        }

        // Run the session protocol
        let result = Self::run_session_protocol(session.clone(), max_duration, max_bandwidth).await;

        // Clean up
        {
            let mut sessions_write = sessions.write().await;
            sessions_write.remove(&cookie);
        }

        tracing::info!("iperf3: Session {} ended", cookie);
        result
    }

    /// Run the iperf3 protocol for a session
    async fn run_session_protocol(
        session: Arc<TestSession>,
        max_duration: Duration,
        max_bandwidth: u64,
    ) -> Result<()> {
        // Send PARAM_EXCHANGE state
        session.send_state(State::ParamExchange).await?;

        // Wait for client to send parameters (state byte first)
        let client_state = session.read_state().await?;
        if client_state != State::ParamExchange {
            return Err(Iperf3Error::Protocol(format!(
                "Expected PARAM_EXCHANGE, got {:?}",
                client_state
            )));
        }

        // Read test parameters
        let params_json = session.read_json_message().await?;
        tracing::debug!("iperf3: Received parameters: {:?}", params_json);

        // Parse parameters
        let mut params: TestParameters = serde_json::from_value(params_json)?;

        // Apply server-side limits
        let requested_duration = params.time;
        if max_duration.as_secs() > 0 && params.time > max_duration.as_secs() {
            params.time = max_duration.as_secs();
            tracing::info!(
                "iperf3: Limiting test duration from {} to {} seconds",
                requested_duration,
                params.time
            );
        }

        if max_bandwidth > 0 && params.bandwidth > max_bandwidth {
            params.bandwidth = max_bandwidth;
        }

        // Store params in session
        // Note: We can't modify session.params directly since it's not mutable
        // For now, we'll use the local params

        // Send acknowledgment (empty JSON object means OK)
        session
            .write_json_message(&serde_json::json!({}))
            .await?;

        // Wait for CREATE_STREAMS
        let client_state = session.read_state().await?;
        if client_state != State::CreateStreams {
            return Err(Iperf3Error::Protocol(format!(
                "Expected CREATE_STREAMS, got {:?}",
                client_state
            )));
        }

        // Send CREATE_STREAMS back
        session.send_state(State::CreateStreams).await?;

        // Wait for streams to connect (give client time to create streams)
        let expected_streams = params.parallel as usize;
        let timeout = Duration::from_secs(STREAM_CONNECT_TIMEOUT_SECS);
        let start = std::time::Instant::now();

        while session.stream_count().await < expected_streams {
            if start.elapsed() > timeout {
                return Err(Iperf3Error::Protocol(format!(
                    "Timeout waiting for {} data streams, got {}",
                    expected_streams,
                    session.stream_count().await
                )));
            }
            tokio::time::sleep(Duration::from_millis(STREAM_POLL_INTERVAL_MS)).await;
        }

        tracing::debug!(
            "iperf3: All {} data streams connected",
            session.stream_count().await
        );

        // Wait for TEST_START
        let client_state = session.read_state().await?;
        if client_state != State::TestStart {
            return Err(Iperf3Error::Protocol(format!(
                "Expected TEST_START, got {:?}",
                client_state
            )));
        }

        // Send TEST_START
        session.send_state(State::TestStart).await?;

        // Start the test
        session.start_test().await;

        // Send TEST_RUNNING
        session.send_state(State::TestRunning).await?;

        // Run the actual test
        let test_duration = Duration::from_secs(params.time);
        if params.reverse {
            // Server sends to client
            session.run_sender(test_duration, params.bandwidth).await?;
        } else {
            // Client sends to server
            session.run_receiver(test_duration).await?;
        }

        // Wait a bit for any remaining data
        tokio::time::sleep(Duration::from_millis(POST_TEST_DELAY_MS)).await;

        // Wait for TEST_END
        let client_state = session.read_state().await?;
        if client_state != State::TestEnd {
            tracing::warn!(
                "iperf3: Expected TEST_END, got {:?}",
                client_state
            );
        }

        // Send TEST_END
        session.send_state(State::TestEnd).await?;

        // Exchange results
        let client_state = session.read_state().await?;
        if client_state != State::ExchangeResults {
            tracing::warn!(
                "iperf3: Expected EXCHANGE_RESULTS, got {:?}",
                client_state
            );
        }

        session.send_state(State::ExchangeResults).await?;

        // Read client results (we mostly ignore these)
        let _client_results = session.read_json_message().await?;

        // Generate and send server results
        let elapsed = session.test_elapsed().await.unwrap_or_default();
        let results = session.generate_results(elapsed.as_secs_f64());
        let results_json = serde_json::to_value(&results)?;
        session.write_json_message(&results_json).await?;

        // Wait for DISPLAY_RESULTS
        let client_state = session.read_state().await?;
        if client_state != State::DisplayResults {
            tracing::warn!(
                "iperf3: Expected DISPLAY_RESULTS, got {:?}",
                client_state
            );
        }

        // Send DISPLAY_RESULTS
        session.send_state(State::DisplayResults).await?;

        // Wait for IPERF_DONE
        let client_state = session.read_state().await?;
        if client_state != State::IperfDone {
            tracing::warn!(
                "iperf3: Expected IPERF_DONE, got {:?}",
                client_state
            );
        }

        // Send SERVER_TERMINATE
        session.send_state(State::ServerTerminate).await?;

        tracing::info!(
            "iperf3: Session completed - sent: {} bytes, received: {} bytes",
            session.get_bytes_sent(),
            session.get_bytes_received()
        );

        Ok(())
    }
}
