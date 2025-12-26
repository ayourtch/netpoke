#!/bin/bash
# Refresh vendored webrtc-util crate
# This script downloads a fresh copy of webrtc-util from crates.io
# and re-applies our modifications using the patch files

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"
VENDORED_DIR="$PROJECT_ROOT/vendored"

echo "=== Refreshing vendored webrtc-util ==="
echo

# Get the version we're using - it's hardcoded in our version info
VERSION="0.12.0"
echo "Target version: $VERSION"
echo "Commit SHA: a1f8f1919235d8452835852e018efd654f2f8366"
echo

# Create temp project to fetch the crate
TEMP_DIR=$(mktemp -d)
trap "rm -rf $TEMP_DIR" EXIT

echo "Creating temporary project to download crate..."
cd "$TEMP_DIR"
cargo new --lib temp_fetch > /dev/null 2>&1
cd temp_fetch
cargo add webrtc-util@$VERSION > /dev/null 2>&1
cargo fetch > /dev/null 2>&1

# Find the downloaded source
SOURCE_DIR=$(find ~/.cargo/registry/src -name "webrtc-util-$VERSION" -type d | head -1)

if [ -z "$SOURCE_DIR" ]; then
    echo "ERROR: Could not find webrtc-util-$VERSION in cargo registry"
    echo "Try running: cargo search webrtc-util"
    exit 1
fi

echo "Found source: $SOURCE_DIR"

# Backup old version if it exists
if [ -d "$VENDORED_DIR/webrtc-util" ]; then
    BACKUP_NAME="webrtc-util.backup.$(date +%Y%m%d_%H%M%S)"
    echo "Backing up old version to $BACKUP_NAME..."
    mv "$VENDORED_DIR/webrtc-util" "$VENDORED_DIR/$BACKUP_NAME"
fi

# Copy fresh version
echo "Copying fresh version..."
mkdir -p "$VENDORED_DIR"
cp -r "$SOURCE_DIR" "$VENDORED_DIR/webrtc-util"

# Copy version info file
if [ -f "$PROJECT_ROOT/vendored/webrtc-util/VENDORED_VERSION_INFO.md" ]; then
    # If it exists in backup, don't overwrite
    :
else
    echo "Note: Remember to verify VENDORED_VERSION_INFO.md is present"
fi

# Apply modifications
echo
echo "Applying modifications..."
cd "$PROJECT_ROOT"
./scripts/apply-patches.sh

echo
echo "✓ webrtc-util refreshed successfully"
echo "  Version: $VERSION"
echo "  Location: vendored/webrtc-util"
echo
echo "Next steps:"
echo "  1. Verify: diff -ur $VENDORED_DIR/$BACKUP_NAME vendored/webrtc-util | head -50"
echo "  2. Test: cargo check --all"
echo "  3. Test: cargo test --all"
echo "  4. If successful, remove backup: rm -rf $VENDORED_DIR/$BACKUP_NAME"
echo "  5. Commit: git add vendored/ patches/ && git commit -m 'Update vendored webrtc-util to $VERSION'"
