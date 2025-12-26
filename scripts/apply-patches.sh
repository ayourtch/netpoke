#!/bin/bash
# Apply patches to vendored webrtc-util
# This script applies all patches from patches/webrtc-util/ to the vendored crate

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"
VENDORED_DIR="$PROJECT_ROOT/vendored/webrtc-util"
PATCHES_DIR="$PROJECT_ROOT/patches/webrtc-util"

echo "=== Applying patches to webrtc-util ==="
echo

if [ ! -d "$VENDORED_DIR" ]; then
    echo "ERROR: vendored/webrtc-util directory not found"
    echo "Run ./scripts/refresh-vendored.sh first"
    exit 1
fi

if [ ! -d "$PATCHES_DIR" ]; then
    echo "ERROR: patches/webrtc-util directory not found"
    exit 1
fi

# Check if already patched
if grep -q "UDP Socket Options Support" "$VENDORED_DIR/src/conn/conn_udp.rs" 2>/dev/null; then
    echo "✓ Patches already applied"
    exit 0
fi

# Apply patches in order
cd "$VENDORED_DIR"
echo "Applying patches..."

for patch_file in "$PATCHES_DIR"/*.patch; do
    if [ -f "$patch_file" ]; then
        patch_name=$(basename "$patch_file")
        echo "  - Applying $patch_name"
        
        # Try to apply patch
        if ! patch -p1 --dry-run < "$patch_file" > /dev/null 2>&1; then
            echo "ERROR: Failed to apply $patch_name"
            echo "The patch may not match the current file state."
            echo "You may need to manually apply the modifications."
            exit 1
        fi
        
        # Actually apply the patch
        patch -p1 < "$patch_file"
    fi
done

echo
echo "✓ All patches applied successfully"
echo
echo "Modified files:"
echo "  - vendored/webrtc-util/Cargo.toml"
echo "  - vendored/webrtc-util/src/lib.rs"
echo "  - vendored/webrtc-util/src/conn/mod.rs"
echo "  - vendored/webrtc-util/src/conn/conn_udp.rs"
