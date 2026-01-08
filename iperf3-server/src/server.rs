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
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::{broadcast, RwLock};

/// Timeout in seconds for waiting for data streams to connect
const STREAM_CONNECT_TIMEOUT_SECS: u64 = 10;

/// Polling interval in milliseconds when waiting for data streams
const STREAM_POLL_INTERVAL_MS: u64 = 50;

/// Delay in milliseconds after test completion to allow remaining data to arrive
const POST_TEST_DELAY_MS: u64 = 100;

/// UDP connect reply magic value (iperf3 protocol specification)
/// This is the ASCII bytes '6789' (0x36, 0x37, 0x38, 0x39)
const UDP_CONNECT_REPLY: u32 = 0x36373839;

/// Normalize an IP address by converting IPv4-mapped IPv6 addresses to IPv4.
///
/// When an iperf3 server listens on :: (IPv6 any address), IPv4 connections
/// appear as IPv4-mapped IPv6 addresses (e.g., ::ffff:192.0.2.1). This function
/// converts them back to IPv4 addresses for consistent auth checks.
fn normalize_ip(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                IpAddr::V4(v4)
            } else {
                IpAddr::V6(v6)
            }
        }
        IpAddr::V4(_) => ip,
    }
}

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
        // Normalize the IP address (convert IPv4-mapped IPv6 to IPv4)
        let normalized_ip = normalize_ip(ip);

        // If auth is not required, allow all
        if !self.config.require_auth {
            return true;
        }

        // Check custom callback first
        if let Some(callback) = self.auth_callback.read().await.as_ref() {
            return callback(normalized_ip);
        }

        // Check allowed IPs list
        let allowed = self.allowed_ips.read().await;
        if allowed.is_empty() {
            // If no IPs configured and auth is required, deny all
            return false;
        }
        allowed.contains_key(&normalized_ip)
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

        // Parse the host address first to determine if it's IPv4 or IPv6
        let ip_addr: IpAddr = self.config.host.parse().map_err(|e| {
            Iperf3Error::InvalidParameter(format!(
                "Invalid host address {}: {}",
                self.config.host, e
            ))
        })?;
        let addr = SocketAddr::new(ip_addr, self.config.port);

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
        let server_port = self.config.port;

        tokio::spawn(async move {
            if let Err(e) = Self::handle_session(
                stream,
                peer_addr,
                sessions,
                max_duration,
                max_bandwidth,
                server_port,
            )
            .await
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
        server_port: u16,
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
                tracing::debug!("iperf3: Data stream connection for session {}", cookie);
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
        let result =
            Self::run_session_protocol(session.clone(), max_duration, max_bandwidth, server_port)
                .await;

        // Clean up
        {
            let mut sessions_write = sessions.write().await;
            sessions_write.remove(&cookie);
        }

        tracing::info!("iperf3: Session {} ended", cookie);
        result
    }

    /// Run the iperf3 protocol for a session.
    ///
    /// Protocol flow (based on iperf3 source):
    /// 1. Server sends PARAM_EXCHANGE state
    /// 2. Client sends JSON parameters directly (no state byte)
    /// 3. Server reads JSON, sends CREATE_STREAMS state
    /// 4. For UDP: Server creates UDP listener and waits for client datagrams
    /// 5. Client creates data streams (no state byte on control connection)
    /// 6. Server detects streams connected, sends TEST_START, then TEST_RUNNING
    /// 7. Test runs
    /// 8. Client sends TEST_END state when done
    /// 9. Server sends EXCHANGE_RESULTS state
    /// 10. Client sends results JSON (no state byte), server reads it
    /// 11. Server sends results JSON
    /// 12. Server sends DISPLAY_RESULTS state
    /// 13. Client sends IPERF_DONE state
    /// 14. Server sends SERVER_TERMINATE state
    async fn run_session_protocol(
        session: Arc<TestSession>,
        max_duration: Duration,
        max_bandwidth: u64,
        server_port: u16,
    ) -> Result<()> {
        // Step 1: Send PARAM_EXCHANGE state
        session.send_state(State::ParamExchange).await?;

        // Step 2: Read test parameters directly (client doesn't send a state byte here)
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

        // Detect if this is a UDP test
        let is_udp = params.udp;
        session.set_udp_mode(is_udp);

        if is_udp {
            tracing::debug!("iperf3: UDP mode requested");
        }

        // Step 3: Send CREATE_STREAMS state (no JSON acknowledgment needed)
        // The iperf3 protocol goes directly from reading parameters to sending CREATE_STREAMS
        session.send_state(State::CreateStreams).await?;

        // Step 4: Wait for data streams to connect
        let expected_streams = params.parallel as usize;
        let timeout = Duration::from_secs(STREAM_CONNECT_TIMEOUT_SECS);
        let start = std::time::Instant::now();

        if is_udp {
            // For UDP, we need to create a UDP listener and accept connections
            // Pass the client address to determine the correct address family (IPv4 vs IPv6)
            Self::accept_udp_streams(
                session.clone(),
                expected_streams,
                server_port,
                timeout,
                session.client_addr,
            )
            .await?;
        } else {
            // For TCP, wait for data streams to connect (handled by main accept loop)
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
        }

        tracing::debug!(
            "iperf3: All {} data streams connected",
            session.stream_count().await
        );

        // Step 5: Send TEST_START, then TEST_RUNNING
        session.send_state(State::TestStart).await?;

        // Start the test
        session.start_test().await;

        session.send_state(State::TestRunning).await?;

        // Step 6: Run the actual test in the background, waiting for TEST_END from client
        // The client controls when the test ends by sending TEST_END
        let test_duration = Duration::from_secs(params.time);
        let data_handles = if is_udp {
            if params.reverse {
                // Server sends to client (UDP)
                session
                    .start_udp_sender_background(test_duration, params.bandwidth, params.blksize)
                    .await
            } else {
                // Client sends to server (UDP)
                session.start_udp_receiver_background(test_duration).await
            }
        } else if params.reverse {
            // Server sends to client (TCP)
            session
                .start_sender_background(test_duration, params.bandwidth)
                .await
        } else {
            // Client sends to server (TCP)
            session.start_receiver_background(test_duration).await
        };

        // Step 7: Wait for TEST_END from client (this is what actually ends the test)
        let client_state = session.read_state().await?;
        if client_state != State::TestEnd {
            tracing::warn!("iperf3: Expected TEST_END, got {:?}", client_state);
        }

        // Cancel data transfer and wait for tasks to finish
        session.cancel();
        for handle in data_handles {
            let _ = handle.await;
        }

        // Wait a bit for any remaining data
        tokio::time::sleep(Duration::from_millis(POST_TEST_DELAY_MS)).await;

        // Step 8: Send EXCHANGE_RESULTS state (not TEST_END - client already sent that)
        session.send_state(State::ExchangeResults).await?;

        // Step 9: Read client results (client sends JSON directly, no state byte)
        let _client_results = session.read_json_message().await?;

        // Step 10: Generate and send server exchange results
        // Note: This uses the exchange format, not the final output format
        let elapsed = session.test_elapsed().await.unwrap_or_default();
        let exchange_results = session.generate_exchange_results(elapsed.as_secs_f64());
        let results_json = serde_json::to_value(&exchange_results)?;
        session.write_json_message(&results_json).await?;

        // Step 11: Send DISPLAY_RESULTS state
        session.send_state(State::DisplayResults).await?;

        // Step 12: Wait for IPERF_DONE from client
        let client_state = session.read_state().await?;
        if client_state != State::IperfDone {
            tracing::warn!("iperf3: Expected IPERF_DONE, got {:?}", client_state);
        }

        // Step 13: Send SERVER_TERMINATE state
        session.send_state(State::ServerTerminate).await?;

        tracing::info!(
            "iperf3: Session completed - sent: {} bytes, received: {} bytes",
            session.get_bytes_sent(),
            session.get_bytes_received()
        );

        Ok(())
    }

    /// Accept UDP streams for a session.
    ///
    /// For UDP, unlike TCP, we need to:
    /// 1. Create a UDP socket bound to the server port
    /// 2. Wait for client to send a datagram
    /// 3. "Connect" the UDP socket to the client address
    /// 4. Send a reply to confirm connection
    /// 5. Repeat for each expected stream
    async fn accept_udp_streams(
        session: Arc<TestSession>,
        expected_streams: usize,
        server_port: u16,
        timeout: Duration,
        client_addr: SocketAddr,
    ) -> Result<()> {
        let start = std::time::Instant::now();

        for stream_num in 0..expected_streams {
            if start.elapsed() > timeout {
                return Err(Iperf3Error::Protocol(format!(
                    "Timeout waiting for UDP streams, got {} of {}",
                    stream_num, expected_streams
                )));
            }

            // Create a UDP socket bound to the server port
            // Use the same address family as the client (IPv4 or IPv6)
            // Note: For multiple parallel streams, we would need SO_REUSEPORT or different ports.
            // Currently, parallel UDP streams > 1 may not work correctly.
            let bind_addr: SocketAddr = if client_addr.is_ipv6() {
                SocketAddr::new(
                    std::net::IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED),
                    server_port,
                )
            } else {
                SocketAddr::new(
                    std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
                    server_port,
                )
            };
            let socket = UdpSocket::bind(bind_addr).await.map_err(|e| {
                Iperf3Error::Protocol(format!("Failed to bind UDP socket to {}: {}", bind_addr, e))
            })?;

            tracing::debug!("iperf3: UDP listener bound to {}", bind_addr);

            // Wait for a datagram from the client
            let mut buf = [0u8; 4];
            let remaining_timeout = timeout.saturating_sub(start.elapsed());
            let (_, client_addr) =
                match tokio::time::timeout(remaining_timeout, socket.recv_from(&mut buf)).await {
                    Ok(Ok((n, addr))) => (n, addr),
                    Ok(Err(e)) => {
                        return Err(Iperf3Error::Protocol(format!(
                            "Failed to receive UDP datagram: {}",
                            e
                        )));
                    }
                    Err(_) => {
                        return Err(Iperf3Error::Protocol(format!(
                            "Timeout waiting for UDP stream {}",
                            stream_num + 1
                        )));
                    }
                };

            tracing::debug!(
                "iperf3: Received UDP datagram from {}, connecting",
                client_addr
            );

            // "Connect" the UDP socket to the client address
            socket.connect(client_addr).await.map_err(|e| {
                Iperf3Error::Protocol(format!("Failed to connect UDP socket: {}", e))
            })?;

            // Send reply to confirm connection
            let reply = UDP_CONNECT_REPLY.to_be_bytes();
            socket
                .send(&reply)
                .await
                .map_err(|e| Iperf3Error::Protocol(format!("Failed to send UDP reply: {}", e)))?;

            tracing::debug!(
                "iperf3: UDP stream {} connected to {}",
                stream_num + 1,
                client_addr
            );

            // Add the socket to the session
            session.add_udp_stream(Arc::new(socket)).await;
        }

        Ok(())
    }
}
