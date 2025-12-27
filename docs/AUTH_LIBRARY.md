# Project Raindrops Authentication

A reusable Rust authentication library that provides multiple authentication methods including plain login and OAuth2 providers with session management. Built to be easily portable to other projects.

## Features

- **Plain Login (Username/Password)**
  - File-based authentication
  - Bcrypt password hashing support
  - No external dependencies required

- **Multiple OAuth2 Providers**
  - Bluesky (with dynamic OAuth discovery and DPoP support)
  - GitHub
  - Google
  - LinkedIn

- **Session Management**
  - Secure session storage
  - Configurable session timeouts
  - Automatic session cleanup

- **Professional UI**
  - Beautiful "Project Raindrops" branded login page
  - Responsive design
  - Modern, professional appearance

- **Flexible Architecture**
  - Middleware for protecting routes
  - Optional authentication support
  - Easy to extend with new providers

- **Easy Configuration**
  - TOML-based configuration
  - Enable/disable providers individually
  - Configurable session settings

## Usage

### Configuration

Add authentication configuration to your `server_config.toml`:

```toml
[auth]
enable_auth = true

# Plain Login (file-based)
[auth.plain_login]
enabled = true

[[auth.plain_login.users]]
username = "admin"
password = "$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/LewY5xyJNPtYPmvwe"  # bcrypt hash
display_name = "Administrator"

# OAuth Providers
[auth.oauth]
enable_bluesky = false
enable_github = false
enable_google = false
enable_linkedin = false

# Bluesky OAuth (no central registration required!)
# bluesky_client_id = "http://localhost:3000/client-metadata.json"
# bluesky_redirect_url = "http://localhost:3000/auth/bluesky/callback"

# GitHub OAuth
# github_client_id = "your_github_client_id"
# github_client_secret = "your_github_client_secret"
# github_redirect_url = "http://localhost:3000/auth/github/callback"

[auth.session]
cookie_name = "session_id"
timeout_seconds = 86400  # 24 hours
secure = false  # Set to true for HTTPS
```

### Integration

```rust
use wifi_verify_auth::{AuthConfig, AuthService, auth_routes, require_auth};
use std::sync::Arc;
use axum::{Router, middleware};

#[tokio::main]
async fn main() {
    // Load your configuration
    let config: AuthConfig = // ... load from file

    // Create auth service
    let auth_service = Arc::new(AuthService::new(config).await.unwrap());
    
    // Build your application
    let app = Router::new()
        // Add auth routes
        .nest("/auth", auth_routes().with_state(auth_service.clone()))
        // Protected routes
        .route("/protected", get(protected_handler))
        .layer(middleware::from_fn_with_state(
            auth_service.clone(),
            require_auth
        ))
        // Public routes
        .route("/public", get(public_handler));
    
    // Run your server
    // ...
}
```

### For Bluesky OAuth

Bluesky uses decentralized OAuth with dynamic discovery. You need to serve a `client-metadata.json` file:

```json
{
  "client_id": "http://localhost:3000/client-metadata.json",
  "application_type": "web",
  "client_name": "Your App Name",
  "client_uri": "http://localhost:3000",
  "dpop_bound_access_tokens": true,
  "grant_types": ["authorization_code", "refresh_token"],
  "redirect_uris": ["http://localhost:3000/auth/bluesky/callback"],
  "response_types": ["code"],
  "scope": "atproto transition:generic",
  "token_endpoint_auth_method": "none"
}
```

### OAuth Provider Setup

- **GitHub**: Register an OAuth App at https://github.com/settings/developers
- **Google**: Create OAuth 2.0 credentials at https://console.cloud.google.com/apis/credentials
- **LinkedIn**: Create an app at https://www.linkedin.com/developers/apps

## Architecture

The crate is organized into modules:

- `config`: Configuration structures
- `error`: Error types
- `middleware`: Authentication middleware for Axum
- `providers`: OAuth provider implementations
- `routes`: HTTP route handlers
- `service`: Main authentication service
- `session`: Session data structures
- `views`: HTML templates (login page)

## Future Enhancements

- Plain username/password authentication
- JWT token support
- Additional OAuth providers
- Rate limiting
- Two-factor authentication

## License

See the parent project's license.
