//! Test session management for iperf3.

use crate::error::{Iperf3Error, Result};
use crate::protocol::{
    ConnectedInfo, EndInfo, ServerResults, StartInfo, State, StreamEndResult,
    StreamResult, TestParameters, TestStartInfo,
};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

/// A test session with a client
pub struct TestSession {
    /// Session ID (cookie)
    pub cookie: String,

    /// Client address
    pub client_addr: SocketAddr,

    /// Test parameters
    pub params: TestParameters,

    /// Control connection
    control_stream: Arc<Mutex<TcpStream>>,

    /// Current state
    state: Arc<Mutex<State>>,

    /// Data streams (for TCP tests)
    data_streams: Arc<Mutex<Vec<Arc<Mutex<TcpStream>>>>>,

    /// Session start time
    pub started_at: Instant,

    /// Test start time (when actual test begins)
    test_started_at: Arc<Mutex<Option<Instant>>>,

    /// Bytes received from client
    bytes_received: Arc<AtomicU64>,

    /// Bytes sent to client
    bytes_sent: Arc<AtomicU64>,

    /// Whether the session is cancelled
    cancelled: Arc<AtomicBool>,
}

impl TestSession {
    /// Create a new test session
    pub fn new(cookie: String, client_addr: SocketAddr, control_stream: TcpStream) -> Self {
        Self {
            cookie,
            client_addr,
            params: TestParameters::default(),
            control_stream: Arc::new(Mutex::new(control_stream)),
            state: Arc::new(Mutex::new(State::ParamExchange)),
            data_streams: Arc::new(Mutex::new(Vec::new())),
            started_at: Instant::now(),
            test_started_at: Arc::new(Mutex::new(None)),
            bytes_received: Arc::new(AtomicU64::new(0)),
            bytes_sent: Arc::new(AtomicU64::new(0)),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Cancel the session
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Check if session is cancelled
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Get current state
    pub async fn get_state(&self) -> State {
        *self.state.lock().await
    }

    /// Set state
    pub async fn set_state(&self, state: State) {
        *self.state.lock().await = state;
    }

    /// Get bytes received
    pub fn get_bytes_received(&self) -> u64 {
        self.bytes_received.load(Ordering::Relaxed)
    }

    /// Get bytes sent
    pub fn get_bytes_sent(&self) -> u64 {
        self.bytes_sent.load(Ordering::Relaxed)
    }

    /// Add bytes received
    pub fn add_bytes_received(&self, bytes: u64) {
        self.bytes_received.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Add bytes sent
    pub fn add_bytes_sent(&self, bytes: u64) {
        self.bytes_sent.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Add a data stream
    pub async fn add_data_stream(&self, stream: TcpStream) {
        let mut streams = self.data_streams.lock().await;
        streams.push(Arc::new(Mutex::new(stream)));
    }

    /// Get number of data streams
    pub async fn stream_count(&self) -> usize {
        self.data_streams.lock().await.len()
    }

    /// Start the test timer
    pub async fn start_test(&self) {
        *self.test_started_at.lock().await = Some(Instant::now());
    }

    /// Get test elapsed time
    pub async fn test_elapsed(&self) -> Option<Duration> {
        self.test_started_at.lock().await.map(|t| t.elapsed())
    }

    /// Read a JSON message from the control connection
    pub async fn read_json_message(&self) -> Result<serde_json::Value> {
        let mut stream = self.control_stream.lock().await;

        // Read 4-byte length prefix (big-endian)
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;

        if len > 1024 * 1024 {
            return Err(Iperf3Error::Protocol(format!(
                "Message too large: {} bytes",
                len
            )));
        }

        // Read JSON data
        let mut data = vec![0u8; len];
        stream.read_exact(&mut data).await?;

        let json: serde_json::Value = serde_json::from_slice(&data)?;
        Ok(json)
    }

    /// Write a JSON message to the control connection
    pub async fn write_json_message(&self, json: &serde_json::Value) -> Result<()> {
        let mut stream = self.control_stream.lock().await;

        let data = serde_json::to_vec(json)?;
        let len = data.len() as u32;

        // Write 4-byte length prefix (big-endian)
        stream.write_all(&len.to_be_bytes()).await?;

        // Write JSON data
        stream.write_all(&data).await?;
        stream.flush().await?;

        Ok(())
    }

    /// Send a state byte on the control connection
    pub async fn send_state(&self, state: State) -> Result<()> {
        let mut stream = self.control_stream.lock().await;
        stream.write_all(&[state.to_byte()]).await?;
        stream.flush().await?;
        self.set_state(state).await;
        Ok(())
    }

    /// Read a state byte from the control connection
    pub async fn read_state(&self) -> Result<State> {
        let mut stream = self.control_stream.lock().await;
        let mut buf = [0u8; 1];
        stream.read_exact(&mut buf).await?;

        State::from_byte(buf[0])
            .ok_or_else(|| Iperf3Error::Protocol(format!("Unknown state: {}", buf[0])))
    }

    /// Run the data stream receiving loop (for normal mode - client sends to server)
    pub async fn run_receiver(&self, max_duration: Duration) -> Result<()> {
        let streams = self.data_streams.lock().await;
        if streams.is_empty() {
            return Ok(());
        }

        let cancelled = self.cancelled.clone();
        let bytes_received = self.bytes_received.clone();
        let deadline = Instant::now() + max_duration;

        // Spawn receiver tasks for each stream
        let mut handles = Vec::new();
        for stream in streams.iter() {
            let stream = stream.clone();
            let cancelled = cancelled.clone();
            let bytes_received = bytes_received.clone();

            let handle = tokio::spawn(async move {
                let mut buf = vec![0u8; 128 * 1024]; // 128KB buffer
                loop {
                    if cancelled.load(Ordering::SeqCst) || Instant::now() > deadline {
                        break;
                    }

                    let mut stream_guard = stream.lock().await;
                    match tokio::time::timeout(
                        Duration::from_millis(100),
                        stream_guard.read(&mut buf),
                    )
                    .await
                    {
                        Ok(Ok(0)) => break, // Connection closed
                        Ok(Ok(n)) => {
                            bytes_received.fetch_add(n as u64, Ordering::Relaxed);
                        }
                        Ok(Err(_)) => break, // Error
                        Err(_) => continue,  // Timeout, check again
                    }
                }
            });
            handles.push(handle);
        }

        // Wait for all receiver tasks
        for handle in handles {
            let _ = handle.await;
        }

        Ok(())
    }

    /// Run the data stream sending loop (for reverse mode - server sends to client)
    pub async fn run_sender(&self, max_duration: Duration, bandwidth: u64) -> Result<()> {
        let streams = self.data_streams.lock().await;
        if streams.is_empty() {
            return Ok(());
        }

        let cancelled = self.cancelled.clone();
        let bytes_sent = self.bytes_sent.clone();
        let deadline = Instant::now() + max_duration;
        let blksize = self.params.blksize as usize;

        // Calculate bytes per interval for bandwidth limiting
        let bytes_per_second = if bandwidth > 0 {
            bandwidth / 8 // Convert bits to bytes
        } else {
            u64::MAX // No limit
        };

        // Spawn sender tasks for each stream
        let mut handles = Vec::new();
        for stream in streams.iter() {
            let stream = stream.clone();
            let cancelled = cancelled.clone();
            let bytes_sent = bytes_sent.clone();

            let handle = tokio::spawn(async move {
                let buf = vec![0u8; blksize];
                let mut last_send = Instant::now();
                let mut bytes_this_second: u64 = 0;

                loop {
                    if cancelled.load(Ordering::SeqCst) || Instant::now() > deadline {
                        break;
                    }

                    // Bandwidth limiting
                    if bytes_per_second != u64::MAX {
                        let elapsed = last_send.elapsed();
                        if elapsed >= Duration::from_secs(1) {
                            last_send = Instant::now();
                            bytes_this_second = 0;
                        } else if bytes_this_second >= bytes_per_second {
                            tokio::time::sleep(Duration::from_millis(1)).await;
                            continue;
                        }
                    }

                    let mut stream_guard = stream.lock().await;
                    match stream_guard.write_all(&buf).await {
                        Ok(_) => {
                            bytes_sent.fetch_add(blksize as u64, Ordering::Relaxed);
                            bytes_this_second += blksize as u64;
                        }
                        Err(_) => break,
                    }
                }
            });
            handles.push(handle);
        }

        // Wait for all sender tasks
        for handle in handles {
            let _ = handle.await;
        }

        Ok(())
    }

    /// Generate server results
    pub fn generate_results(&self, test_duration: f64) -> ServerResults {
        let bytes_sent = self.bytes_sent.load(Ordering::Relaxed);
        let bytes_received = self.bytes_received.load(Ordering::Relaxed);

        let sent_bps = if test_duration > 0.0 {
            (bytes_sent as f64 * 8.0) / test_duration
        } else {
            0.0
        };

        let received_bps = if test_duration > 0.0 {
            (bytes_received as f64 * 8.0) / test_duration
        } else {
            0.0
        };

        ServerResults {
            start: StartInfo {
                connected: vec![ConnectedInfo {
                    socket: 0,
                    local_host: "0.0.0.0".to_string(),
                    local_port: 5201,
                    remote_host: self.client_addr.ip().to_string(),
                    remote_port: self.client_addr.port(),
                }],
                version: "iperf 3.16 (Rust)".to_string(),
                system_info: "Rust iperf3 server".to_string(),
                test_start: TestStartInfo {
                    protocol: self.params.protocol.clone(),
                    num_streams: self.params.parallel,
                    blksize: self.params.blksize,
                    omit: self.params.omit,
                    duration: self.params.time,
                    bytes: self.params.bytes,
                    blocks: self.params.blockcount,
                    reverse: self.params.reverse,
                },
            },
            intervals: vec![],
            end: EndInfo {
                streams: vec![StreamEndResult {
                    sender: StreamResult {
                        id: 1,
                        bytes: bytes_sent,
                        seconds: test_duration,
                        bits_per_second: sent_bps,
                        retransmits: Some(0),
                        jitter_ms: None,
                        lost_packets: None,
                        packets: None,
                        lost_percent: None,
                    },
                    receiver: StreamResult {
                        id: 1,
                        bytes: bytes_received,
                        seconds: test_duration,
                        bits_per_second: received_bps,
                        retransmits: None,
                        jitter_ms: None,
                        lost_packets: None,
                        packets: None,
                        lost_percent: None,
                    },
                }],
                sum_sent: Some(StreamResult {
                    id: 0,
                    bytes: bytes_sent,
                    seconds: test_duration,
                    bits_per_second: sent_bps,
                    retransmits: Some(0),
                    jitter_ms: None,
                    lost_packets: None,
                    packets: None,
                    lost_percent: None,
                }),
                sum_received: Some(StreamResult {
                    id: 0,
                    bytes: bytes_received,
                    seconds: test_duration,
                    bits_per_second: received_bps,
                    retransmits: None,
                    jitter_ms: None,
                    lost_packets: None,
                    packets: None,
                    lost_percent: None,
                }),
                cpu_utilization_percent: None,
            },
        }
    }
}
