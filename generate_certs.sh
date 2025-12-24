#!/bin/bash

# Script to generate self-signed SSL certificates for development/testing
# This creates certificates that will work for HTTPS but will show security warnings in browsers

echo "Generating self-signed SSL certificates for development..."

# Create certs directory if it doesn't exist
mkdir -p certs

# Create OpenSSL configuration file for v3 extensions
cat > certs/openssl.cnf << 'EOF'
[req]
default_bits = 2048
distinguished_name = req_distinguished_name
req_extensions = v3_req
x509_extensions = v3_ca

[req_distinguished_name]

[v3_req]
subjectAltName = @alt_names

[v3_ca]
subjectAltName = @alt_names
basicConstraints = CA:FALSE
keyUsage = digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth

[alt_names]
DNS.1 = localhost
DNS.2 = *.localhost
IP.1 = 127.0.0.1
IP.2 = ::1
EOF

# Generate private key and v3 certificate in one step (valid for 365 days)
openssl req -x509 -newkey rsa:2048 -keyout certs/server.key -out certs/server.crt -days 365 -nodes -subj "/C=US/ST=State/L=City/O=Organization/CN=localhost" -config certs/openssl.cnf

# Clean up the config file
rm certs/openssl.cnf

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