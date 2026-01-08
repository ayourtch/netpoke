# Allowed Users List - Access Control Guide

## Overview

The allowed users list provides fine-grained access control by restricting which authenticated users can access your NetPoke server. This feature works with all authentication methods (Plain Login, Bluesky, GitHub, Google, LinkedIn).

## How It Works

```
User Authentication Flow with Access Control:
┌─────────────────┐
│ User attempts   │
│ to login        │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Authentication  │ ◄─── Plain Login, OAuth (Bluesky, GitHub, etc.)
│ succeeds        │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Check if user   │
│ is in allowed   │
│ users list      │
└────────┬────────┘
         │
    ┌────┴────┐
    │         │
    ▼         ▼
┌─────┐   ┌─────┐
│ YES │   │ NO  │
└──┬──┘   └──┬──┘
   │         │
   ▼         ▼
┌─────┐   ┌─────────────┐
│Allow│   │Access Denied│
│Access   │    Page     │
└─────┘   └─────────────┘
```

## Configuration

### Basic Setup

In `server_config.toml`:

```toml
[auth]
enable_auth = true

# Access Control List
allowed_users = [
    "admin",
    "@alice.bsky.social",
    "developer@company.com",
]
```

### Empty List (Default Behavior)

```toml
[auth]
allowed_users = []  # All authenticated users are allowed
```

When the `allowed_users` list is empty, **any user who can successfully authenticate** will be granted access.

### Restricted Access

```toml
[auth]
allowed_users = [
    "user1",
    "user2",
]
```

When the list contains entries, **only these users** will be granted access after authentication.

## Examples by Authentication Method

### Plain Login Users

For plain login, use the **username** exactly as configured:

```toml
[auth]
allowed_users = ["admin", "developer", "operator"]

[auth.plain_login]
enabled = true

[[auth.plain_login.users]]
username = "admin"
password = "$2b$12$..."
display_name = "Administrator"

[[auth.plain_login.users]]
username = "developer"
password = "$2b$12$..."
display_name = "Developer"

[[auth.plain_login.users]]
username = "tester"  # This user CAN authenticate but will be DENIED access
password = "$2b$12$..."
display_name = "Tester"
```

In this example:
- `admin` and `developer` can authenticate AND access the application
- `tester` can authenticate but will see the "Access Denied" page

### Bluesky OAuth

For Bluesky, use the **full handle including @**:

```toml
[auth]
allowed_users = [
    "@alice.bsky.social",
    "@bob.custom-domain.com",
]

[auth.oauth]
enable_bluesky = true
bluesky_client_id = "http://localhost:3000/client-metadata.json"
bluesky_redirect_url = "http://localhost:3000/auth/bluesky/callback"
```

### GitHub OAuth

For GitHub, use the **username or email**:

```toml
[auth]
allowed_users = [
    "octocat",              # GitHub username
    "developer@github.com", # GitHub email
]

[auth.oauth]
enable_github = true
github_client_id = "your_client_id"
github_client_secret = "your_client_secret"
github_redirect_url = "http://localhost:3000/auth/github/callback"
```

### Google OAuth

For Google, use the **email address**:

```toml
[auth]
allowed_users = [
    "user@company.com",
    "admin@company.com",
]

[auth.oauth]
enable_google = true
google_client_id = "your_client_id"
google_client_secret = "your_client_secret"
google_redirect_url = "http://localhost:3000/auth/google/callback"
```

### LinkedIn OAuth

For LinkedIn, use the **email address**:

```toml
[auth]
allowed_users = [
    "professional@company.com",
]

[auth.oauth]
enable_linkedin = true
linkedin_client_id = "your_client_id"
linkedin_client_secret = "your_client_secret"
linkedin_redirect_url = "http://localhost:3000/auth/linkedin/callback"
```

## Mixed Authentication Methods

You can allow users from different authentication methods:

```toml
[auth]
enable_auth = true

# Mix of plain login users, Bluesky handles, and OAuth emails
allowed_users = [
    # Plain login users
    "admin",
    "local_operator",
    
    # Bluesky handles
    "@security.team.bsky.social",
    "@cto.company.com",
    
    # OAuth emails (GitHub, Google, LinkedIn)
    "developer@company.com",
    "ops@company.com",
]

# Enable multiple authentication methods
[auth.plain_login]
enabled = true

[[auth.plain_login.users]]
username = "admin"
password = "$2b$12$..."

[auth.oauth]
enable_bluesky = true
enable_github = true
enable_google = true
```

## User Experience

### Allowed User

1. User logs in via any provider
2. User is redirected to the main application
3. User can access all protected resources

### Denied User

1. User logs in via any provider (authentication succeeds)
2. System checks the allowed users list
3. User is shown a professional "Access Denied" page with:
   - 🚫 Icon
   - "Access Denied" heading
   - Message explaining they don't have access
   - Their authenticated handle/email displayed
   - Suggestion to contact administrator
   - Logout button

The user **remains authenticated** but **cannot access protected resources**.

## Security Best Practices

1. **Start Restrictive**: Begin with a limited allowed users list and expand as needed
2. **Regular Audits**: Review the allowed users list periodically
3. **Remove Departing Users**: Remove users who no longer need access
4. **Use Specific Identifiers**: 
   - For Bluesky: Use full handle with @
   - For OAuth: Use verified email addresses
   - For Plain Login: Use unique usernames
5. **Log Access Denials**: The system logs when users are denied access (check server logs)

## Troubleshooting

### User is authenticated but sees "Access Denied"

**Cause**: User's handle/email is not in the `allowed_users` list

**Solution**: 
1. Check the user's exact handle/email by looking at server logs
2. Add the handle/email to `allowed_users` in `server_config.toml`
3. Restart the server
4. User can log out and log back in

### All users are being denied

**Cause**: `allowed_users` list exists but is incomplete

**Solution**: 
- Option 1: Set `allowed_users = []` to allow all authenticated users
- Option 2: Add all intended users to the list

### Case sensitivity issues

**Note**: User handles and emails are case-sensitive. Ensure exact matches:
- ❌ Wrong: `"admin"` vs `"Admin"`
- ✅ Correct: Both must match exactly

## Example Configurations

### Development Setup (All authenticated users allowed)

```toml
[auth]
enable_auth = true
allowed_users = []  # Empty = allow all

[auth.plain_login]
enabled = true

[[auth.plain_login.users]]
username = "dev"
password = "dev123"
```

### Production Setup (Restricted access)

```toml
[auth]
enable_auth = true
allowed_users = [
    "admin",
    "operator",
    "support@company.com",
    "@security.team.bsky.social",
]

[auth.plain_login]
enabled = true

[[auth.plain_login.users]]
username = "admin"
password = "$2b$12$..."  # bcrypt hash

[[auth.plain_login.users]]
username = "operator"
password = "$2b$12$..."

[auth.oauth]
enable_github = true
enable_bluesky = true

[auth.session]
timeout_seconds = 3600  # 1 hour
secure = true  # HTTPS only
```

### Team Setup (Multiple auth methods)

```toml
[auth]
enable_auth = true
allowed_users = [
    # Internal team (plain login)
    "admin",
    "ops",
    
    # External contractors (OAuth)
    "contractor@external.com",
    
    # Community moderators (Bluesky)
    "@moderator.community.bsky.social",
]
```

## Summary

The allowed users list provides:
- ✅ Fine-grained access control
- ✅ Works with all authentication methods
- ✅ Easy to configure and maintain
- ✅ Professional user experience for denied access
- ✅ No code changes required
- ✅ Simple TOML configuration
