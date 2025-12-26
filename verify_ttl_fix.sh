#!/bin/bash
# Verification script for TTL propagation fix
# This script helps verify that UDP options (TTL) are being set correctly

set -e

echo "=========================================="
echo "TTL Propagation Fix Verification Script"
echo "=========================================="
echo ""

# Check if running on Linux
if [[ "$(uname)" != "Linux" ]]; then
    echo "⚠️  WARNING: This fix is Linux-specific (#[cfg(target_os = \"linux\")])"
    echo "             The send_with_options functionality will not be compiled on this OS."
    exit 1
fi

echo "✅ Running on Linux"
echo ""

# Check if we can build
echo "Step 1: Building the server..."
echo "----------------------------"
if cargo build --package wifi-verify-server 2>&1 | tail -20; then
    echo "✅ Server built successfully"
else
    echo "❌ Build failed"
    exit 1
fi
echo ""

# Provide instructions for testing
echo "Step 2: Testing Instructions"
echo "----------------------------"
echo ""
echo "To verify the fix is working, you need to:"
echo ""
echo "1. Run the server with INFO logging enabled:"
echo "   RUST_LOG=info cargo run --package wifi-verify-server"
echo ""
echo "2. In another terminal, monitor UDP packets (requires root/sudo):"
echo "   sudo tcpdump -vvv -i any 'udp and port 5004' -n"
echo ""
echo "3. Connect a client and trigger traceroute probes"
echo ""
echo "4. Look for these log markers in the server output:"
echo "   🔵 - Operation in progress (options being propagated)"
echo "   ✅ - Success (sendmsg succeeded)"
echo "   ❌ - Failure (if any errors occur)"
echo ""
echo "5. Expected log sequence for each probe:"
echo "   🔵 Sending traceroute probe via data channel: TTL=X"
echo "   🔵 Created UdpSendOptions: TTL=Some(X)"
echo "   🔵 Stream::packetize: Set UDP options on chunk: TTL=Some(X)"
echo "   🔵 Association::bundle: Extracted UDP options from chunk: TTL=Some(X)"
echo "   🔵 SCTP Association: Sending packet with UDP options: TTL=Some(X)"
echo "   🔵 DTLSConn::send_with_options: Forwarding with TTL=Some(X)"
echo "   🔵 Endpoint::send_with_options: Forwarding with TTL=Some(X)"
echo "   🔵 UdpSocket::send_with_options called with TTL=Some(X)"
echo "   🔵 sendmsg: Adding IPv4 TTL control message: TTL=X"
echo "   ✅ sendmsg SUCCEEDED: sent XXX bytes"
echo ""
echo "6. In tcpdump, verify the TTL field in IP headers:"
echo "   Look for: 'ttl 1', 'ttl 2', 'ttl 3', etc."
echo "   Each traceroute probe should have an incrementing TTL"
echo ""
echo "7. Verify ICMP Time Exceeded messages are generated:"
echo "   sudo tcpdump -vvv -i any 'icmp and icmp[0] == 11'"
echo "   (icmp[0] == 11 means ICMP Time Exceeded)"
echo ""
echo "=========================================="
echo "Quick Test Commands"
echo "=========================================="
echo ""
echo "# Build and run server with logging"
echo "RUST_LOG=info cargo run --package wifi-verify-server"
echo ""
echo "# In another terminal - Monitor all UDP traffic on port 5004"
echo "sudo tcpdump -vvv -i any 'udp port 5004' -n"
echo ""
echo "# Or filter to see just TTL values"
echo "sudo tcpdump -i any 'udp port 5004' -n | grep -o 'ttl [0-9]*'"
echo ""
echo "# Monitor ICMP Time Exceeded messages"
echo "sudo tcpdump -i any 'icmp and icmp[0] == 11' -n -vvv"
echo ""
echo "# Filter server logs for emoji markers"
echo "cargo run --package wifi-verify-server 2>&1 | grep -E '🔵|✅|❌'"
echo ""
echo "=========================================="
echo ""
echo "If you see the complete log sequence with all 🔵 markers"
echo "and tcpdump shows varying TTL values (1, 2, 3, ...), then"
echo "the fix is working correctly! ✅"
echo ""
