# Authentication Setup Guide

This guide explains how to set up and use the authentication system in WiFi Verify.

## Overview

The authentication system supports multiple authentication methods:
- **Plain Login** (username/password) - File-based authentication
- **Bluesky** (decentralized OAuth with dynamic discovery)
- **GitHub**
- **Google**
- **LinkedIn**

Authentication can be enabled or disabled globally, and individual providers can be enabled/disabled independently.

## Quick Start

### 1. Enable Authentication

Edit `server_config.toml`:

```toml
[auth]
enable_auth = true

[auth.plain_login]
enabled = true

[auth.oauth]
enable_bluesky = false
enable_github = false
enable_google = false
enable_linkedin = false
```

### 2. Configure Authentication Methods

You can configure authentication either in `server_config.toml` or using environment variables.

#### Plain Login (Username/Password)

Add users to `server_config.toml`:

```toml
[auth.plain_login]
enabled = true

[[auth.plain_login.users]]
username = "admin"
password = "admin123"  # Plain text (not recommended for production)
display_name = "Administrator"

[[auth.plain_login.users]]
username = "user1"
password = "$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/LewY5xyJNPtYPmvwe"  # bcrypt hash
display_name = "User One"
```

**Security Note**: For production, always use bcrypt hashed passwords (starting with `$2`). You can generate bcrypt hashes using online tools or the bcrypt CLI.

#### OAuth Providers

Add to `server_config.toml`:

```toml
[auth.oauth]
bluesky_client_id = "http://localhost:3000/client-metadata.json"
bluesky_redirect_url = "http://localhost:3000/auth/bluesky/callback"
```

Or use environment variables (see `.env.example`):

```bash
export BLUESKY_CLIENT_ID="http://localhost:3000/client-metadata.json"
export BLUESKY_REDIRECT_URL="http://localhost:3000/auth/bluesky/callback"
```

### 3. Run the Server

```bash
cd /path/to/wifi-verify
cargo run --bin wifi-verify-server
```

Visit `http://localhost:3000` - you'll be redirected to the login page if authentication is enabled.

## Access Control (Allowed Users List)

The authentication system includes a powerful access control feature that allows you to restrict access to specific users, even after they successfully authenticate.

### How It Works

1. **User authenticates** via any provider (Plain Login, Bluesky, GitHub, Google, LinkedIn)
2. **System checks** if the user's handle/email is in the `allowed_users` list
3. **Access granted** if the list is empty (all authenticated users allowed) OR user is in the list
4. **Access denied** if the list is not empty AND user is not in the list

### Configuration

Edit `server_config.toml`:

```toml
[auth]
# List of allowed user handles/emails
allowed_users = [
    "admin",                    # Plain login username
    "@alice.bsky.social",       # Bluesky handle
    "user@example.com",         # OAuth email (GitHub, Google, LinkedIn)
    "developer",                # Another plain login user
]
```

**Important Notes:**
- If `allowed_users` is **empty** (`[]`), all authenticated users can access the application
- If `allowed_users` has entries, only those users will be granted access
- For **Plain Login**: use the username (e.g., `"admin"`)
- For **Bluesky**: use the handle including @ (e.g., `"@alice.bsky.social"`)
- For **GitHub/Google/LinkedIn**: use the email or username returned by the provider

### Example Scenarios

**Scenario 1: Allow everyone who can authenticate**
```toml
[auth]
allowed_users = []  # Empty list = all authenticated users allowed
```

**Scenario 2: Restrict to specific users**
```toml
[auth]
allowed_users = [
    "admin",
    "developer@company.com",
    "@security.team.bsky.social",
]
```

**Scenario 3: Mix of authentication methods**
```toml
[auth]
allowed_users = [
    "admin",                      # Plain login
    "ops@company.com",           # GitHub OAuth
    "@team.member.bsky.social",  # Bluesky OAuth
]

[auth.plain_login]
enabled = true

[[auth.plain_login.users]]
username = "admin"
password = "$2b$12$..."

[auth.oauth]
enable_github = true
enable_bluesky = true
```

### Access Denied Page

If a user successfully authenticates but is not in the allowed list, they will see a professional "Access Denied" page that:
- Displays their authenticated handle/email
- Explains they don't have access
- Provides a logout button
- Suggests contacting the system administrator

## Provider-Specific Setup

### Plain Login (Username/Password)

Plain login provides file-based authentication without requiring external OAuth providers. This is ideal for:
- Small deployments with a fixed set of users
- Internal tools and services
- Development and testing

**Setup:**

1. Enable plain login in `server_config.toml`:

```toml
[auth.plain_login]
enabled = true
```

2. Add users to the configuration:

```toml
[[auth.plain_login.users]]
username = "admin"
password = "securepassword123"
display_name = "System Administrator"

[[auth.plain_login.users]]
username = "user1"
password = "$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/LewY5xyJNPtYPmvwe"
display_name = "Regular User"
```

**Password Security:**

- **Plain text passwords** (e.g., `"admin123"`) are supported but **NOT recommended** for production
- **Bcrypt hashed passwords** (starting with `$2b$` or `$2a$`) are strongly recommended for production
- The system automatically detects if a password is bcrypt hashed and verifies accordingly
- To generate a bcrypt hash:
  - Use online tools like bcrypt-generator.com
  - Use bcrypt CLI tools
  - Use Python: `python -c "import bcrypt; print(bcrypt.hashpw(b'password', bcrypt.gensalt()).decode())"`

**Example with bcrypt:**

```bash
# Generate a bcrypt hash (cost factor 12)
$ python3 -c "import bcrypt; print(bcrypt.hashpw(b'mypassword', bcrypt.gensalt(12)).decode())"
$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/LewY5xyJNPtYPmvwe
```

Then use it in config:

```toml
[[auth.plain_login.users]]
username = "secure_user"
password = "$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/LewY5xyJNPtYPmvwe"
display_name = "Secure User"
```

### Bluesky

Bluesky uses decentralized OAuth, so no central registration is required!

1. Ensure `server/static/client-metadata.json` exists with your configuration
2. The file is served at `/client-metadata.json` by the server
3. Users authenticate by entering their Bluesky handle (e.g., `@alice.bsky.social`)

**Configuration:**
```toml
[auth.oauth]
enable_bluesky = true
bluesky_client_id = "http://localhost:3000/client-metadata.json"
bluesky_redirect_url = "http://localhost:3000/auth/bluesky/callback"
```

### GitHub

1. Go to https://github.com/settings/developers
2. Click "New OAuth App"
3. Fill in:
   - Application name: WiFi Verify
   - Homepage URL: http://localhost:3000
   - Authorization callback URL: http://localhost:3000/auth/github/callback
4. Copy the Client ID and Client Secret

**Configuration:**
```toml
[auth.oauth]
enable_github = true
github_client_id = "your_github_client_id"
github_client_secret = "your_github_client_secret"
github_redirect_url = "http://localhost:3000/auth/github/callback"
```

### Google

1. Go to https://console.cloud.google.com/apis/credentials
2. Create a new "OAuth 2.0 Client ID"
3. Application type: Web application
4. Add authorized redirect URI: http://localhost:3000/auth/google/callback
5. Copy the Client ID and Client Secret

**Configuration:**
```toml
[auth.oauth]
enable_google = true
google_client_id = "your_google_client_id"
google_client_secret = "your_google_client_secret"
google_redirect_url = "http://localhost:3000/auth/google/callback"
```

### LinkedIn

1. Go to https://www.linkedin.com/developers/apps
2. Create a new app
3. Under "Auth" tab, add redirect URL: http://localhost:3000/auth/linkedin/callback
4. Request access to "Sign In with LinkedIn using OpenID Connect"
5. Copy the Client ID and Client Secret

**Configuration:**
```toml
[auth.oauth]
enable_linkedin = true
linkedin_client_id = "your_linkedin_client_id"
linkedin_client_secret = "your_linkedin_client_secret"
linkedin_redirect_url = "http://localhost:3000/auth/linkedin/callback"
```

## Configuration Reference

### Authentication Settings

```toml
[auth]
# Enable/disable authentication globally
enable_auth = true

[auth.oauth]
# Enable individual providers
enable_bluesky = false
enable_github = false
enable_google = false
enable_linkedin = false

# Provider credentials (see above)
# ...

[auth.session]
# Session cookie name
cookie_name = "session_id"

# Session timeout in seconds (default: 86400 = 24 hours)
timeout_seconds = 86400

# Require HTTPS for cookies (set to true in production)
secure = false
```

## Routes

When authentication is enabled, the following routes are available:

- `GET /auth/login` - Login page (Project Raindrops)
- `POST /auth/bluesky/login` - Start Bluesky auth
- `GET /auth/bluesky/callback` - Bluesky callback
- `GET /auth/github/login` - Start GitHub auth
- `GET /auth/github/callback` - GitHub callback
- `GET /auth/google/login` - Start Google auth
- `GET /auth/google/callback` - Google callback
- `GET /auth/linkedin/login` - Start LinkedIn auth
- `GET /auth/linkedin/callback` - LinkedIn callback
- `POST /auth/logout` - Logout

## Security Considerations

1. **HTTPS in Production**: Always use HTTPS in production and set `auth.session.secure = true`
2. **Redirect URLs**: Ensure redirect URLs match exactly in both OAuth provider settings and configuration
3. **Session Timeout**: Adjust `timeout_seconds` based on your security requirements
4. **Client Secrets**: Never commit OAuth client secrets to version control
5. **Environment Variables**: Use environment variables or secure secret management for production

## Troubleshooting

### "Authentication service failed to initialize"

Check that:
- At least one provider is enabled
- Provider credentials are correctly configured
- For Bluesky, `client-metadata.json` is accessible

### "OAuth callback error"

Check that:
- Redirect URLs match in both provider settings and configuration
- Credentials are correct
- Provider is enabled in configuration

### "Session validation failed"

This is normal when:
- Session has expired
- User hasn't logged in yet
- Cookie was cleared

## Development Tips

1. **Disable Auth for Development**: Set `enable_auth = false` to bypass authentication
2. **Test Multiple Providers**: Enable multiple providers to test the login page
3. **Session Cleanup**: Sessions are automatically cleaned up on validation
4. **Logs**: Check server logs for authentication debug information

## Architecture

The authentication system is built as a separate reusable crate (`wifi-verify-auth`) that can be easily ported to other projects. Key components:

- **Providers**: Individual OAuth provider implementations
- **Service**: Central authentication service managing sessions
- **Middleware**: Route protection middleware
- **Views**: Professional login page UI (Project Raindrops)
- **Routes**: HTTP handlers for auth flows

For more details, see `auth/README.md`.
