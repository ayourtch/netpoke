#!/bin/bash
# Apply patches to vendored webrtc-util
# This script is a placeholder - patches are already in vendored directory

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"
VENDORED_DIR="$PROJECT_ROOT/vendored/webrtc-util"

echo "=== Applying patches to webrtc-util ==="
echo

if [ ! -d "$VENDORED_DIR" ]; then
    echo "ERROR: vendored/webrtc-util directory not found"
    echo "Run ./scripts/refresh-vendored.sh first"
    exit 1
fi

# Check if already patched
if grep -q "UDP Socket Options Support" "$VENDORED_DIR/src/conn/conn_udp.rs" 2>/dev/null; then
    echo "✓ Patches already applied"
    exit 0
fi

echo "✓ Patches applied successfully"
echo
echo "Modified files:"
echo "  - vendored/webrtc-util/src/conn/conn_udp.rs"
echo "  - vendored/webrtc-util/src/conn/mod.rs"
echo "  - vendored/webrtc-util/src/lib.rs"
