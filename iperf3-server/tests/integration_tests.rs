//! Integration tests for the iperf3 server.

use iperf3_server::{Iperf3Config, Iperf3Server};
use std::net::IpAddr;

#[test]
fn test_config_defaults() {
    let config = Iperf3Config::default();
    assert!(!config.enabled);
    assert_eq!(config.host, "0.0.0.0");
    assert_eq!(config.port, 5201);
    assert_eq!(config.max_sessions, 10);
    assert_eq!(config.max_duration_secs, 3600);
    assert!(!config.require_auth);
    assert_eq!(config.max_bandwidth, 0);
}

#[test]
fn test_config_serialization() {
    let config = Iperf3Config {
        enabled: true,
        host: "127.0.0.1".to_string(),
        port: 5202,
        max_sessions: 5,
        max_duration_secs: 600,
        require_auth: true,
        auth_timeout_secs: 300,
        max_bandwidth: 1_000_000_000,
    };

    let json = serde_json::to_string(&config).unwrap();
    let parsed: Iperf3Config = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.enabled, config.enabled);
    assert_eq!(parsed.host, config.host);
    assert_eq!(parsed.port, config.port);
    assert_eq!(parsed.max_sessions, config.max_sessions);
    assert_eq!(parsed.max_duration_secs, config.max_duration_secs);
    assert_eq!(parsed.require_auth, config.require_auth);
    assert_eq!(parsed.auth_timeout_secs, config.auth_timeout_secs);
    assert_eq!(parsed.max_bandwidth, config.max_bandwidth);
}

#[tokio::test]
async fn test_server_creation() {
    let config = Iperf3Config::default();
    let server = Iperf3Server::new(config);

    // Server should start with no sessions
    assert_eq!(server.session_count().await, 0);
}

#[tokio::test]
async fn test_allowed_ips() {
    let mut config = Iperf3Config::default();
    config.require_auth = true;
    let server = Iperf3Server::new(config);

    let test_ip: IpAddr = "192.168.1.100".parse().unwrap();

    // Initially, IP should not be allowed (require_auth is true, no IPs added)
    assert!(!server.is_ip_allowed(test_ip).await);

    // Add the IP
    server.add_allowed_ip(test_ip).await;

    // Now it should be allowed
    assert!(server.is_ip_allowed(test_ip).await);

    // Remove it
    server.remove_allowed_ip(&test_ip).await;

    // Should no longer be allowed
    assert!(!server.is_ip_allowed(test_ip).await);
}

#[tokio::test]
async fn test_auth_disabled_allows_all() {
    let mut config = Iperf3Config::default();
    config.require_auth = false;
    let server = Iperf3Server::new(config);

    let test_ip: IpAddr = "10.0.0.1".parse().unwrap();

    // With auth disabled, any IP should be allowed
    assert!(server.is_ip_allowed(test_ip).await);
}

#[test]
fn test_protocol_state_conversion() {
    use iperf3_server::protocol::State;

    // Test all state conversions
    let states = [
        State::ParamExchange,
        State::CreateStreams,
        State::TestStart,
        State::TestRunning,
        State::TestEnd,
        State::ExchangeResults,
        State::DisplayResults,
        State::IperfDone,
        State::ServerTerminate,
        State::AccessDenied,
        State::ServerError,
    ];

    for state in states {
        let byte = state.to_byte();
        let parsed = State::from_byte(byte);
        assert_eq!(parsed, Some(state));
    }
}

#[test]
fn test_test_parameters_defaults() {
    use iperf3_server::protocol::TestParameters;

    let params = TestParameters::default();
    assert_eq!(params.protocol, "TCP");
    assert_eq!(params.time, 10);
    assert_eq!(params.parallel, 1);
    assert!(!params.reverse);
    assert!(!params.bidirectional);
    assert_eq!(params.bandwidth, 0);
    assert_eq!(params.blksize, 128 * 1024);
}
