#!/bin/bash
# Refresh vendored webrtc-util crate
# This script downloads the latest version of webrtc-util from crates.io
# and re-applies our modifications

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"
VENDORED_DIR="$PROJECT_ROOT/vendored"

echo "=== Refreshing vendored webrtc-util ==="
echo

# Get the version we're using
VERSION=$(grep 'webrtc-util' "$PROJECT_ROOT/server/Cargo.toml" | grep -oP '\d+\.\d+\.\d+' | head -1)
echo "Target version: $VERSION"

# Create temp project to fetch the crate
TEMP_DIR=$(mktemp -d)
trap "rm -rf $TEMP_DIR" EXIT

echo "Creating temporary project..."
cd "$TEMP_DIR"
cargo new --lib temp_fetch
cd temp_fetch
cargo add webrtc-util@$VERSION
cargo fetch

# Find the downloaded source
SOURCE_DIR=$(find ~/.cargo/registry/src -name "webrtc-util-$VERSION" -type d | head -1)

if [ -z "$SOURCE_DIR" ]; then
    echo "ERROR: Could not find webrtc-util-$VERSION in cargo registry"
    exit 1
fi

echo "Found source: $SOURCE_DIR"

# Backup old version if it exists
if [ -d "$VENDORED_DIR/webrtc-util" ]; then
    echo "Backing up old version..."
    mv "$VENDORED_DIR/webrtc-util" "$VENDORED_DIR/webrtc-util.backup.$(date +%Y%m%d_%H%M%S)"
fi

# Copy fresh version
echo "Copying fresh version..."
mkdir -p "$VENDORED_DIR"
cp -r "$SOURCE_DIR" "$VENDORED_DIR/webrtc-util"

# Apply modifications
echo "Applying modifications..."
cd "$PROJECT_ROOT"
./scripts/apply-patches.sh

echo
echo "✓ webrtc-util refreshed successfully"
echo "  Version: $VERSION"
echo "  Location: vendored/webrtc-util"
echo
echo "Next steps:"
echo "  1. Test: cargo check --all"
echo "  2. Commit: git add vendored/ && git commit -m 'Update vendored webrtc-util'"
