//! # iperf3-server
//!
//! A modular iperf3 server implementation in Rust.
//!
//! This crate provides a complete iperf3-compatible server that can be easily
//! integrated into other applications or used standalone.
//!
//! ## Features
//!
//! - Full iperf3 protocol support (control connection + data streams)
//! - TCP and UDP test modes
//! - Configurable test parameters
//! - IP-based access control for authenticated users
//! - Async/await based on Tokio
//!
//! ## Example
//!
//! ```no_run
//! use iperf3_server::{Iperf3Server, Iperf3Config};
//! use std::net::IpAddr;
//!
//! #[tokio::main]
//! async fn main() {
//!     let config = Iperf3Config::default();
//!     let server = Iperf3Server::new(config);
//!     
//!     // Optionally add allowed IPs (if empty, all IPs are allowed)
//!     // server.add_allowed_ip("192.168.1.100".parse().unwrap()).await;
//!     
//!     server.run().await.unwrap();
//! }
//! ```

pub mod config;
pub mod error;
pub mod protocol;
pub mod server;
pub mod session;

pub use config::Iperf3Config;
pub use error::Iperf3Error;
pub use server::Iperf3Server;
pub use session::TestSession;
