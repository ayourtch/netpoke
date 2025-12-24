#!/bin/bash
set -e

echo "Building WASM client..."
wasm-pack build --target web --out-dir ../server/static/public/pkg

echo "WASM client built successfully!"
echo "Output: server/static/public/pkg/"
