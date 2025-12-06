#!/bin/bash
set -e

echo "Building WASM client..."
wasm-pack build --target web --out-dir ../server/static/pkg

echo "WASM client built successfully!"
echo "Output: server/static/pkg/"
