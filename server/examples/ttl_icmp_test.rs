use common::SendOptions;
/// Example: TTL Testing with ICMP Time Exceeded
///
/// This example demonstrates how to use the packet tracking API to:
/// 1. Send UDP packets with decreasing TTL values
/// 2. Track these packets for ICMP correlation
/// 3. Capture ICMP "Time Exceeded" errors
/// 4. Match ICMP errors back to original packets
///
/// This is useful for network path discovery (similar to traceroute).
///
/// Run with:
/// ```
/// sudo cargo run --example ttl_icmp_test
/// ```
///
/// Note: Requires CAP_NET_RAW or root for ICMP listener
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== TTL Testing with ICMP Time Exceeded ===\n");

    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("This example demonstrates packet tracking with TTL testing.");
    println!("It shows how ICMP Time Exceeded errors are matched to original packets.\n");

    // In a real application, you would:
    // 1. Set send_options on ProbePacket or BulkPacket
    // 2. Send via WebRTC data channel
    // 3. Retrieve tracked events via API endpoint

    demonstrate_api_usage();
    demonstrate_ttl_sequence();
    demonstrate_mtu_discovery();

    println!("\n=== Example Complete ===\n");
    println!("In production:");
    println!("1. Set send_options on ProbePacket/BulkPacket before sending");
    println!("2. Packets are automatically tracked if track_for_ms > 0");
    println!("3. ICMP listener captures errors and matches them");
    println!("4. Retrieve events via GET /api/tracking/events");
    println!("5. Analyze RTT, packet loss, and network topology");

    Ok(())
}

fn demonstrate_api_usage() {
    println!("## API Usage Example\n");
    println!("```rust");
    println!("// 1. Create SendOptions for packet");
    println!("let options = SendOptions {{");
    println!("    ttl: Some(5),           // Will expire after 5 hops");
    println!("    df_bit: Some(true),     // Don't fragment");
    println!("    tos: Some(0x10),        // Low delay");
    println!("    flow_label: None,       // IPv6 only");
    println!("    track_for_ms: 5000,     // Track for 5 seconds");
    println!("}};");
    println!();
    println!("// 2. Attach to probe packet");
    println!("let mut probe = ProbePacket {{");
    println!("    seq: 42,");
    println!("    timestamp_ms: current_time_ms(),");
    println!("    direction: Direction::ServerToClient,");
    println!("    send_options: Some(options),");
    println!("}};");
    println!();
    println!("// 3. Send via data channel (WebRTC handles the rest)");
    println!("data_channel.send(&serde_json::to_vec(&probe)?).await?;");
    println!();
    println!("// 4. Retrieve tracked events");
    println!("let response = reqwest::get(\"http://localhost:3000/api/tracking/events\")");
    println!("    .await?");
    println!("    .json::<TrackedEventsResponse>()");
    println!("    .await?;");
    println!();
    println!("// 5. Analyze events");
    println!("for event in response.events {{");
    println!("    println!(\"ICMP error: RTT={{}}ms, TTL={{}}\",");
    println!("             event.rtt_ms, event.send_options.ttl.unwrap());");
    println!("}}");
    println!("```\n");
}

fn demonstrate_ttl_sequence() {
    println!("## Path Discovery (Traceroute-like)\n");
    println!("Send packets with incrementing TTL to discover network path:\n");

    println!("```rust");
    println!("// Send probes with TTL 1, 2, 3, ... until destination reached");
    println!("for ttl in 1..=30 {{");
    println!("    let options = SendOptions {{");
    println!("        ttl: Some(ttl),");
    println!("        track_for_ms: 5000,  // Track for 5 seconds");
    println!("        ..Default::default()");
    println!("    }};");
    println!("    ");
    println!("    // Send probe with these options");
    println!("    send_probe_with_options(options).await?;");
    println!("    ");
    println!("    // Wait a bit between probes");
    println!("    tokio::time::sleep(Duration::from_millis(100)).await;");
    println!("}}");
    println!();
    println!("// Check tracked events after a few seconds");
    println!("tokio::time::sleep(Duration::from_secs(6)).await;");
    println!("let events = get_tracked_events().await?;");
    println!();
    println!("// Build network path from ICMP Time Exceeded errors");
    println!("for event in events {{");
    println!("    if is_icmp_time_exceeded(&event.icmp_packet) {{");
    println!("        let hop = event.send_options.ttl.unwrap();");
    println!("        let router_ip = extract_icmp_source(&event.icmp_packet);");
    println!("        println!(\"Hop {{}}: {{}} ({{}}ms)\", hop, router_ip, event.rtt_ms);");
    println!("    }}");
    println!("}}");
    println!("```\n");

    println!("Expected output:");
    println!("  Hop 1: 192.168.1.1 (2ms)      <- Your router");
    println!("  Hop 2: 10.0.0.1 (5ms)         <- ISP gateway");
    println!("  Hop 3: 203.0.113.1 (15ms)     <- ISP backbone");
    println!("  Hop 4: 198.51.100.1 (25ms)    <- Internet backbone");
    println!("  ...\n");
}

fn demonstrate_mtu_discovery() {
    println!("## MTU Discovery\n");
    println!("Send packets with DF bit set and increasing sizes to find path MTU:\n");

    println!("```rust");
    println!("// Binary search for MTU");
    println!("let mut min_mtu = 576;   // IPv4 minimum");
    println!("let mut max_mtu = 9000;  // Jumbo frames");
    println!();
    println!("while min_mtu < max_mtu {{");
    println!("    let test_mtu = (min_mtu + max_mtu) / 2;");
    println!("    ");
    println!("    let options = SendOptions {{");
    println!("        ttl: Some(64),");
    println!("        df_bit: Some(true),      // Don't fragment - critical!");
    println!("        track_for_ms: 2000,");
    println!("        ..Default::default()");
    println!("    }};");
    println!("    ");
    println!("    // Send packet of size test_mtu");
    println!("    send_bulk_with_options(test_mtu, options).await?;");
    println!("    tokio::time::sleep(Duration::from_millis(200)).await;");
    println!("    ");
    println!("    // Check if we got ICMP Fragmentation Needed");
    println!("    let events = get_tracked_events().await?;");
    println!("    if events.iter().any(|e| is_icmp_frag_needed(&e.icmp_packet)) {{");
    println!("        max_mtu = test_mtu - 1;  // Too large");
    println!("    }} else {{");
    println!("        min_mtu = test_mtu + 1;  // Fits, try larger");
    println!("    }}");
    println!("}}");
    println!();
    println!("println!(\"Path MTU discovered: {{}} bytes\", min_mtu);");
    println!("```\n");

    println!("Expected output:");
    println!("  Testing MTU 4788...");
    println!("  Testing MTU 2394...");
    println!("  Testing MTU 1697...");
    println!("  Testing MTU 1500...");
    println!("  Path MTU discovered: 1500 bytes\n");
}

/// Example helper functions (not implemented - just signatures)

#[allow(dead_code)]
fn is_icmp_time_exceeded(_icmp_packet: &[u8]) -> bool {
    // Check if ICMP type is 11 (Time Exceeded)
    // icmp_packet[0] == 11
    true
}

#[allow(dead_code)]
fn is_icmp_frag_needed(_icmp_packet: &[u8]) -> bool {
    // Check if ICMP type 3, code 4 (Fragmentation Needed)
    // icmp_packet[0] == 3 && icmp_packet[1] == 4
    true
}

#[allow(dead_code)]
fn extract_icmp_source(_icmp_packet: &[u8]) -> String {
    // Extract source IP from ICMP packet
    // Would parse IP header to get source address
    "192.168.1.1".to_string()
}

/// HTTP API examples

#[allow(dead_code)]
async fn get_tracked_events() -> Result<TrackedEventsResponse, Box<dyn std::error::Error>> {
    // In production, call: GET http://localhost:3000/api/tracking/events
    Ok(TrackedEventsResponse {
        events: vec![],
        count: 0,
    })
}

#[derive(Debug)]
#[allow(dead_code)]
struct TrackedEventsResponse {
    events: Vec<TrackedEventInfo>,
    count: usize,
}

#[derive(Debug)]
#[allow(dead_code)]
struct TrackedEventInfo {
    rtt_ms: u64,
    send_options: SendOptions,
    icmp_packet: Vec<u8>,
}
