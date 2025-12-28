# GitHub Actions

## Update Server Workflow

This repository includes a GitHub Action workflow for updating the deployment server.

### Workflow: Update Server

**File:** `.github/workflows/update-server.yml`

**Trigger:** Manual (workflow_dispatch)

**Purpose:** Connects to the remote server via SSH and runs the server update script.

### Setup Requirements

Before using this workflow, you need to configure a repository secret:

1. Go to your repository's Settings > Secrets and variables > Actions
2. Click "New repository secret"
3. Name: `BEEHOST_PRIVATE_KEY`
4. Value: The private SSH key content that has access to `netpoke@beehost.stdio.be`

### How to Run

1. Navigate to the "Actions" tab in your GitHub repository
2. Select "Update Server" from the workflow list
3. Click "Run workflow" button
4. Select the branch (typically main or master)
5. Click "Run workflow" to start the deployment

### What it Does

The workflow performs the following steps:

1. **Setup SSH**: 
   - Creates SSH directory
   - Writes the private key from the secret
   - Sets proper permissions (600)
   - Adds the server to known hosts

2. **Run Update Command**:
   - Connects to `beehost.stdio.be` as user `netpoke`
   - Executes `/home/netpoke/bin/update-server`

3. **Cleanup**:
   - Removes the private key file (runs even if previous steps fail)

### Security Notes

- The SSH private key is stored as a GitHub secret and is not exposed in logs
- The key is automatically removed after the workflow completes
- SSH host key verification is performed via ssh-keyscan
- Consider using a dedicated SSH key with limited permissions for this workflow

### Troubleshooting

If the workflow fails:

- Check that the `BEEHOST_PRIVATE_KEY` secret is properly configured
- Verify that the SSH key has access to the server
- Ensure `/home/netpoke/bin/update-server` exists and is executable on the server
- Check the workflow run logs for specific error messages
