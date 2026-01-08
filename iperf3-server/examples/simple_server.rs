//! Simple iperf3 server example.
//!
//! This example demonstrates how to run a basic iperf3 server that accepts
//! connections from iperf3 clients.
//!
//! Run with:
//! ```bash
//! cargo run --example simple_server
//! ```
//!
//! Then test with:
//! ```bash
//! iperf3 -c 127.0.0.1 -t 5
//! ```

use iperf3_server::{Iperf3Config, Iperf3Server};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure the server
    let config = Iperf3Config {
        enabled: true,
        host: "0.0.0.0".to_string(), // Listen on all interfaces
        port: 5201,                  // Default iperf3 port
        max_sessions: 10,            // Maximum concurrent sessions
        max_duration_secs: 3600,     // Maximum test duration (1 hour)
        require_auth: false,         // No authentication required
        auth_timeout_secs: 60,
        max_bandwidth: 0, // No bandwidth limit (0 = unlimited)
    };

    let server = Arc::new(Iperf3Server::new(config));

    println!("iperf3 server starting on 0.0.0.0:5201");
    println!("Press Ctrl+C to stop");
    println!();
    println!("Test with: iperf3 -c <server-ip> -t 5");
    println!();

    // Handle Ctrl+C gracefully
    let server_for_signal = server.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for Ctrl+C");
        println!("\nShutting down...");
        server_for_signal.shutdown();
    });

    // Run the server
    if let Err(e) = server.run().await {
        eprintln!("Server error: {}", e);
        return Err(e.into());
    }

    Ok(())
}
