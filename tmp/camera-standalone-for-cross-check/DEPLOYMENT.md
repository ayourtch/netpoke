# Deployment Guide

This project uses GitHub Actions to automatically build the Rust WebAssembly application and deploy it to a remote server.

## GitHub Actions Workflow

The deployment workflow (`.github/workflows/deploy.yml`) automatically:
1. Builds the WASM module using `wasm-pack`
2. Prepares deployment files (index.html, pkg/, js/)
3. Deploys to the configured server via SSH/rsync

## Triggers

The workflow runs on:
- Every push to `main` or `master` branch
- Manual trigger via GitHub Actions UI (workflow_dispatch)

## Required GitHub Secrets

You need to configure the following secrets in your GitHub repository:

### Setting up secrets:
Go to: **Repository Settings → Secrets and variables → Actions → New repository secret**

### Required secrets:

#### 1. `DEPLOY_SSH_KEY`
Your private SSH key for authentication.

**To generate a new SSH key:**
```bash
ssh-keygen -t ed25519 -C "github-deploy@camera-wasm" -f ~/.ssh/camera_deploy_key
```

**Copy the private key:**
```bash
cat ~/.ssh/camera_deploy_key
```
Paste the entire content (including `-----BEGIN OPENSSH PRIVATE KEY-----` and `-----END OPENSSH PRIVATE KEY-----`) into the GitHub secret.

**Add the public key to your server:**
```bash
# Copy the public key
cat ~/.ssh/camera_deploy_key.pub

# On the server, add it to authorized_keys
ssh user@foobar.de
mkdir -p ~/.ssh
chmod 700 ~/.ssh
echo "paste-public-key-here" >> ~/.ssh/authorized_keys
chmod 600 ~/.ssh/authorized_keys
```

#### 2. `DEPLOY_HOST`
The hostname or IP address of your server.

**Example value:**
```
foobar.de
```

#### 3. `DEPLOY_USER`
The SSH username for connecting to the server.

**Example value:**
```
www-data
```
or
```
deploy
```

#### 4. `DEPLOY_PATH`
The full path on the server where files should be deployed.

**For your setup:**
```
/var/www/foobar.de/cast/
```

**Important:** The path should:
- Exist on the server (create it first)
- Be writable by the DEPLOY_USER
- End with a trailing slash

## Server Setup

### 1. Create deployment directory
```bash
ssh user@foobar.de
sudo mkdir -p /var/www/foobar.de/cast
sudo chown www-data:www-data /var/www/foobar.de/cast
# Or adjust ownership to your DEPLOY_USER
```

### 2. Configure web server

#### For Nginx:
```nginx
server {
    listen 80;
    server_name foobar.de;

    location /cast/ {
        alias /var/www/foobar.de/cast/;
        index index.html;

        # CORS headers for WASM
        add_header Cross-Origin-Embedder-Policy require-corp;
        add_header Cross-Origin-Opener-Policy same-origin;

        # Cache control for WASM files
        location ~* \.(wasm|js)$ {
            add_header Cache-Control "public, max-age=31536000, immutable";
        }

        # MIME types
        types {
            application/wasm wasm;
            application/javascript js;
            text/html html;
        }
    }
}
```

#### For Apache:
```apache
<VirtualHost *:80>
    ServerName foobar.de

    Alias /cast /var/www/foobar.de/cast
    <Directory /var/www/foobar.de/cast>
        Options -Indexes +FollowSymLinks
        AllowOverride None
        Require all granted

        # CORS headers for WASM
        Header set Cross-Origin-Embedder-Policy "require-corp"
        Header set Cross-Origin-Opener-Policy "same-origin"

        # MIME types
        AddType application/wasm .wasm
        AddType application/javascript .js
    </Directory>
</VirtualHost>
```

Reload your web server:
```bash
# Nginx
sudo systemctl reload nginx

# Apache
sudo systemctl reload apache2
```

### 3. Verify permissions
```bash
# Ensure the deploy user can write to the directory
ls -la /var/www/foobar.de/
# Should show: drwxr-xr-x ... www-data www-data ... cast/
```

## Testing Deployment

### Manual trigger
1. Go to GitHub repository
2. Click "Actions" tab
3. Select "Build and Deploy WASM" workflow
4. Click "Run workflow" → "Run workflow"

### Check deployment
Once the workflow completes:
1. Visit: `http://foobar.de/cast/`
2. The camera recording app should load
3. Check browser console for errors

## Troubleshooting

### SSH connection fails
- Verify `DEPLOY_HOST` is correct and server is reachable
- Check SSH key is correctly formatted in secret
- Ensure public key is in server's `~/.ssh/authorized_keys`

### Permission denied
- Check `DEPLOY_USER` has write access to `DEPLOY_PATH`
- Verify directory ownership: `sudo chown -R $USER:$USER /var/www/foobar.de/cast`

### WASM files won't load
- Check browser console for CORS errors
- Ensure web server CORS headers are set correctly
- Verify MIME types are configured

### Deployment succeeds but site shows old version
- Hard refresh browser: Cmd+Shift+R (Mac) or Ctrl+Shift+R (Windows/Linux)
- Check if web server is caching files
- Verify files actually updated on server: `ls -la /var/www/foobar.de/cast/pkg/`

## Manual Deployment

If you need to deploy manually without GitHub Actions:

```bash
# Build locally
wasm-pack build --target web --out-dir pkg --release

# Deploy
rsync -avz --delete \
  -e "ssh -i ~/.ssh/your_key" \
  index.html pkg/ js/ \
  user@foobar.de:/var/www/foobar.de/cast/
```

## Security Notes

1. **Never commit SSH private keys** - Always use GitHub Secrets
2. **Use dedicated deploy keys** - Don't reuse personal SSH keys
3. **Limit key permissions** - Create a dedicated user with minimal permissions
4. **Enable HTTPS** - Use Let's Encrypt for SSL certificates
5. **Review rsync flags** - The `--delete` flag removes files not in source

## CI/CD Best Practices

- The workflow only deploys from `main`/`master` branches
- Failed builds don't deploy
- SSH keys are cleaned up after deployment (even on failure)
- The workflow can be manually triggered for testing

## Next Steps

After setup:
1. Test by pushing to main branch
2. Verify deployment at http://foobar.de/cast/
3. Set up HTTPS with Let's Encrypt
4. Configure web server caching headers
5. Set up monitoring/alerts for deployment failures
