# HTTPS Setup for WiFi Verify Server

This guide explains how to enable HTTPS support for your WiFi Verify Server.

## Quick Start

### 1. Generate Development Certificates

For development and testing, you can generate self-signed certificates:

```bash
./generate_certs.sh
```

This creates:
- `certs/server.key` - Private key
- `certs/server.crt` - Self-signed certificate

### 2. Update Configuration

Edit `server_config.toml` and enable HTTPS:

```toml
[server]
enable_https = true
ssl_cert_path = "certs/server.crt"
ssl_key_path = "certs/server.key"
```

### 3. Run the Server

```bash
cd server
cargo run
```

Your server will now be available at:
- HTTP: http://localhost:3000
- HTTPS: https://localhost:3443

## Configuration Options

### Basic HTTPS Configuration

```toml
[server]
host = "0.0.0.0"
http_port = 3000          # HTTP port
https_port = 3443         # HTTPS port
enable_http = true        # Enable HTTP server
enable_https = true       # Enable HTTPS server
ssl_cert_path = "certs/server.crt"  # Path to certificate file
ssl_key_path = "certs/server.key"   # Path to private key file
```

### Advanced Configuration

You can also configure the server using environment variables:

```bash
export WIFI_VERIFY_SERVER_ENABLE_HTTPS=true
export WIFI_VERIFY_SERVER_SSL_CERT_PATH=certs/server.crt
export WIFI_VERIFY_SERVER_SSL_KEY_PATH=certs/server.key
export WIFI_VERIFY_SERVER_HTTPS_PORT=8443
cargo run
```

## Certificate Options

### 1. Self-Signed Certificates (Development)

Use for local development and testing. Browsers will show security warnings.

**Generate:**
```bash
./generate_certs.sh
```

**Pros:**
- Quick and easy
- Free
- Good for development

**Cons:**
- Security warnings in browsers
- Not trusted by browsers
- Not suitable for production

### 2. Let's Encrypt Certificates (Production)

For production use, get certificates from Let's Encrypt using tools like `certbot`:

```bash
# Install certbot
sudo apt install certbot  # Ubuntu/Debian
# or
sudo yum install certbot  # CentOS/RHEL

# Generate certificates
sudo certbot certonly --standalone -d your-domain.com

# Copy certificates
sudo cp /etc/letsencrypt/live/your-domain.com/fullchain.pem certs/server.crt
sudo cp /etc/letsencrypt/live/your-domain.com/privkey.pem certs/server.key
```

**Configuration:**
```toml
[server]
domain = "your-domain.com"
enable_https = true
ssl_cert_path = "certs/server.crt"
ssl_key_path = "certs/server.key"
```

### 3. Commercial Certificates

Purchase certificates from Certificate Authorities like:
- DigiCert
- Sectigo
- GeoTrust
- GlobalSign

## WebSocket over HTTPS

Your WebSocket connections will automatically work over HTTPS when you connect to `wss://` URLs:

```javascript
// HTTP WebSocket
const ws = new WebSocket('ws://localhost:3000/api/dashboard/ws');

// HTTPS WebSocket (secure)
const ws = new WebSocket('wss://localhost:3443/api/dashboard/ws');
```

## Security Considerations

### 1. HTTP Strict Transport Security (HSTS)

Consider adding HSTS headers for production:

```rust
// In your route handlers, add this header:
// Strict-Transport-Security: max-age=31536000; includeSubDomains
```

### 2. Certificate Validation

Ensure your certificates are valid and trusted:

- Check expiration dates
- Verify certificate chain
- Use strong cipher suites

### 3. Firewall Configuration

Make sure your HTTPS port (default 3443) is open:

```bash
# Ubuntu/Debian
sudo ufw allow 3443

# CentOS/RHEL
sudo firewall-cmd --add-port=3443/tcp --permanent
sudo firewall-cmd --reload
```

## Troubleshooting

### Certificate Issues

**Problem:** "Certificate verify failed"
**Solution:** Check if your certificate is valid and trusted

**Problem:** "Private key doesn't match certificate"
**Solution:** Ensure the key and certificate are a matching pair

### Port Issues

**Problem:** "Address already in use"
**Solution:** Check if another service is using the port or if HTTP/HTTPS are conflicting

**Problem:** Can't connect to HTTPS
**Solution:** Verify firewall settings and certificate configuration

### WebSocket Issues

**Problem:** WebSocket connection fails over HTTPS
**Solution:** Use `wss://` URL instead of `ws://` and ensure your certificates are valid

## Production Deployment

For production deployment:

1. **Use trusted certificates** (Let's Encrypt or commercial CA)
2. **Disable HTTP** or redirect HTTP to HTTPS
3. **Use strong cipher suites**
4. **Enable HSTS headers**
5. **Monitor certificate expiration**
6. **Set up automated certificate renewal**

### Example Production Configuration

```toml
[server]
host = "0.0.0.0"
http_port = 3000
https_port = 443
enable_http = false           # Disable HTTP in production
enable_https = true
ssl_cert_path = "/etc/letsencrypt/live/your-domain.com/fullchain.pem"
ssl_key_path = "/etc/letsencrypt/live/your-domain.com/privkey.pem"

[security]
enable_cors = true
allowed_origins = ["https://your-domain.com"]
```

## API Endpoints

All your existing API endpoints will work over HTTPS:

- `https://your-domain.com/health`
- `https://your-domain.com/api/signaling/start`
- `https://your-domain.com/api/signaling/ice`
- `wss://your-domain.com/api/dashboard/ws` (WebSocket)

## Additional Resources

- [Mozilla SSL Configuration Generator](https://ssl-config.mozilla.org/)
- [Let's Encrypt Documentation](https://letsencrypt.org/docs/)
- [Rustls Documentation](https://docs.rs/rustls/)