# Bluesky Auth Demo

A Rust web application demonstrating proper Bluesky OAuth authentication with dynamic service discovery using Axum framework.

## Features

- 🔐 **Multi-provider OAuth 2.0 authentication**: Bluesky, GitHub, and Google
- 🌐 Complete Bluesky OAuth with dynamic service discovery
- 🐙 GitHub OAuth integration with standard flow
- 🔵 Google OAuth integration with OpenID Connect
- 🌍 Support for custom domain handles (e.g., @user.example.com)
- 📡 DNS TXT record resolution for handle-to-DID conversion
- 🔗 Automatic OAuth endpoint discovery per BlueSky specification
- 🍪 Session-based authentication with PKCE
- 🛡️ Protected routes that require authentication
- 🎨 Clean, responsive HTML interface
- 🚀 Built with Rust and Axum

## Setup

### Prerequisites

- Rust 1.70+ installed
- A publicly accessible URL for hosting your client metadata (or use localhost for development)

### OAuth Client Configuration

**Important**: Bluesky uses **decentralized OAuth** - there is no central registration dashboard!

Instead of registering your app in a web portal, you need to:

1. **Host a client metadata JSON file** at a publicly accessible HTTPS URL (or localhost for development)
2. **Your client metadata URL becomes your `client_id`** (e.g., `https://yourdomain.com/client-metadata.json`)
3. **No client secret is required** - Bluesky OAuth uses PKCE (Proof Key for Code Exchange) for security

For local development:
- The application automatically serves `client-metadata.json` at `http://localhost:3000/client-metadata.json`
- Edit `client-metadata.json` to customize your app name and URIs

For production deployment:
- Host `client-metadata.json` on your domain at a public HTTPS URL
- Update the `client_id` and `redirect_uris` in the file to match your production domain
- Set the `BLUESKY_CLIENT_ID` environment variable to your metadata file URL

### GitHub OAuth Setup

1. **Go to GitHub Settings**: https://github.com/settings/developers
2. **Click "New OAuth App"**
3. **Configure your application**:
   - Application name: Your app name
   - Homepage URL: `https://yourdomain.com`
   - Authorization callback URL: `https://yourdomain.com/auth/github/callback`
4. **Copy your credentials**:
   - Client ID
   - Client Secret (click "Generate a new client secret")
5. **Add to `.env` file**:
   ```
   GITHUB_CLIENT_ID=your_github_client_id
   GITHUB_CLIENT_SECRET=your_github_client_secret
   GITHUB_REDIRECT_URL=https://yourdomain.com/auth/github/callback
   ```

### Google OAuth Setup

1. **Go to Google Cloud Console**: https://console.cloud.google.com/apis/credentials
2. **Create a new project** (if needed)
3. **Enable Google+ API**:
   - Go to "APIs & Services" > "Library"
   - Search for "Google+ API" and enable it
4. **Create OAuth 2.0 Client ID**:
   - Go to "APIs & Services" > "Credentials"
   - Click "Create Credentials" > "OAuth client ID"
   - Application type: "Web application"
   - Authorized JavaScript origins: `https://yourdomain.com`
   - Authorized redirect URIs: `https://yourdomain.com/auth/google/callback`
5. **Copy your credentials**:
   - Client ID
   - Client Secret
6. **Add to `.env` file**:
   ```
   GOOGLE_CLIENT_ID=your_google_client_id.apps.googleusercontent.com
   GOOGLE_CLIENT_SECRET=your_google_client_secret
   GOOGLE_REDIRECT_URL=https://yourdomain.com/auth/google/callback
   ```

### Installation

1. Clone this repository:
```bash
git clone <repository-url>
cd blinkout
```

2. Copy the example environment file:
```bash
cp .env.example .env
```

3. Edit `.env` with your configuration:
```env
BLUESKY_CLIENT_ID=http://localhost:3000/client-metadata.json
BLUESKY_REDIRECT_URL=http://localhost:3000/auth/callback
```

4. (Optional) Customize `client-metadata.json` with your app details:
```json
{
  "client_id": "http://localhost:3000/client-metadata.json",
  "client_name": "Your App Name",
  "redirect_uris": ["http://localhost:3000/auth/callback"]
}
```

5. Install dependencies and run:
```bash
cargo run
```

The server will start on `http://localhost:3000`.

## Service Discovery Flow

This application implements the complete BlueSky OAuth service discovery flow as described in the ATProtocol specification:

1. **DNS TXT Record Lookup**: Resolves `@handle` to DID via `_atproto.<handle>` TXT record
2. **DID Resolution**: Resolves DID to service endpoint via PLC directory
3. **Resource Metadata**: Fetches OAuth protected resource metadata
4. **Authorization Metadata**: Discovers OAuth endpoints dynamically

This enables support for both `@*.bsky.social` handles and custom domain handles like `@user.example.com`.

## Usage

1. Open your browser and navigate to `http://localhost:3000`
2. Enter your Bluesky handle (e.g., `@alice.bsky.social` or `@user.example.com`)
3. Click "Login with Bluesky" to authenticate
4. The application will discover the appropriate OAuth endpoints automatically
5. You'll be redirected to Bluesky for authorization
6. After successful authentication, you can access the protected page at `/protected`
7. Use the logout button to end your session

## Routes

- `/` - Home page with login options
- `/client-metadata.json` - OAuth client metadata (served automatically)
- `/auth/login` - Initiates Bluesky OAuth flow
- `/auth/callback` - Bluesky OAuth callback handler
- `/auth/github/login` - Initiates GitHub OAuth flow
- `/auth/github/callback` - GitHub OAuth callback handler
- `/auth/google/login` - Initiates Google OAuth flow
- `/auth/google/callback` - Google OAuth callback handler
- `/auth/logout` - Logout endpoint
- `/protected` - Protected page (requires authentication)

## Security Features

- PKCE (Proof Key for Code Exchange) for enhanced security
- CSRF protection via state tokens
- HttpOnly, SameSite cookies for session management
- Secure token storage in server-side sessions

## Development

This application uses:
- **Axum** - Web framework
- **OAuth2** - Authentication flow
- **Tokio** - Async runtime
- **Serde** - Serialization
- **UUID** - Session ID generation

## License

MIT License