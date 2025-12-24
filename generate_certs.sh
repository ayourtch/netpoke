#!/bin/bash

# Script to generate self-signed SSL certificates for development/testing
# This creates certificates that will work for HTTPS but will show security warnings in browsers

echo "Generating self-signed SSL certificates for development..."

# Create certs directory if it doesn't exist
mkdir -p certs

# Generate private key
openssl genrsa -out certs/server.key 2048

# Generate certificate signing request
openssl req -new -key certs/server.key -out certs/server.csr -subj "/C=US/ST=State/L=City/O=Organization/CN=localhost"

# Generate self-signed certificate (valid for 365 days)
openssl x509 -req -days 365 -in certs/server.csr -signkey certs/server.key -out certs/server.crt

# Clean up the CSR file
rm certs/server.csr

echo "Certificates generated successfully!"
echo ""
echo "Files created:"
echo "  certs/server.key - Private key"
echo "  certs/server.crt - Self-signed certificate"
echo ""
echo "To use these certificates:"
echo "1. Copy certs/server.crt and certs/server.key to your server configuration"
echo "2. Update your server_config.toml to point to these files"
echo "3. Set enable_https = true in the configuration"
echo ""
echo "Note: These are self-signed certificates and will show security warnings in browsers."
echo "For production use, obtain certificates from a trusted Certificate Authority (CA)."
echo ""
echo "Example server_config.toml settings:"
echo '  enable_https = true'
echo '  ssl_cert_path = "certs/server.crt"'
echo '  ssl_key_path = "certs/server.key"'