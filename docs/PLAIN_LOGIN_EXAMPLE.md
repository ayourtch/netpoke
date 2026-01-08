# Example: Enable Plain Login Authentication

To enable file-based username/password authentication, add this to your `server_config.toml`:

```toml
[auth]
enable_auth = true

[auth.plain_login]
enabled = true

# Define allowed users
[[auth.plain_login.users]]
username = "admin"
password = "admin123"  # Plain text (dev only - NOT for production!)
display_name = "System Administrator"

[[auth.plain_login.users]]
username = "developer"
password = "$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/LewY5xyJNPtYPmvwe"  # bcrypt hash
display_name = "Developer Account"

[[auth.plain_login.users]]
username = "user1"
password = "$2b$12$aBcDeFgHiJkLmNoPqRsTuVwXyZ1234567890AbCdEfGhIjKlMnOp"  # bcrypt hash
display_name = "Regular User"
```

## Password Security

**For Development:**
- Plain text passwords are supported for quick testing
- Example: `password = "mysecret123"`

**For Production (RECOMMENDED):**
- Always use bcrypt hashed passwords
- Bcrypt hashes start with `$2b$` or `$2a$`
- Use cost factor 12 or higher

## Generate Bcrypt Hash

### Using Python:
```bash
python3 -c "import bcrypt; print(bcrypt.hashpw(b'your_password_here', bcrypt.gensalt(12)).decode())"
```

### Using Node.js:
```bash
npm install -g bcrypt-cli
bcrypt-cli hash your_password_here
```

### Using Online Tools:
- Visit bcrypt-generator.com
- Enter your password
- Copy the generated hash

## Login Page Display

When plain login is enabled, users will see a username/password form on the login page before any OAuth provider buttons.

The login form includes:
- Username field
- Password field (masked)
- Login button styled to match the Project Raindrops theme

## Combining with OAuth

You can enable both plain login AND OAuth providers:

```toml
[auth.plain_login]
enabled = true

[[auth.plain_login.users]]
username = "admin"
password = "$2b$12$..."

[auth.oauth]
enable_github = true
github_client_id = "..."
github_client_secret = "..."
github_redirect_url = "..."
```

This allows users to choose between:
1. Logging in with username/password (plain login)
2. Logging in with GitHub (OAuth)

## Security Notes

⚠️ **Important Security Considerations:**

1. **Never commit plain text passwords to version control**
2. **Always use bcrypt hashing in production**
3. **Keep server_config.toml secure** - it contains authentication credentials
4. **Consider using environment variables** for sensitive configurations
5. **Enable HTTPS** in production and set `secure = true` for cookies
6. **Rotate passwords regularly** for production systems
7. **Use strong passwords** with minimum 12 characters

## Testing the Configuration

1. Update your `server_config.toml` with the example above
2. Start the server: `cargo run --bin netpoke-server`
3. Navigate to `http://localhost:3000`
4. You'll be redirected to the login page
5. Enter username "admin" and password "admin123"
6. You should be authenticated and redirected to the main application
