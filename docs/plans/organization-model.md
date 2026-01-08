# WiFi-Verify: Organization & Access Model

## Overview

WiFi-Verify uses a hierarchical model for organizing users, projects, and survey data. This enables enterprise deployments where multiple teams or clients need isolated access to their own data.

---

## Hierarchy

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              Platform                                    │
│                                                                          │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                        Organization                              │   │
│  │  (e.g., "Acme IT Services")                                     │   │
│  │                                                                  │   │
│  │  ┌────────────────────┐  ┌────────────────────┐                 │   │
│  │  │     Project        │  │     Project        │                 │   │
│  │  │  "Client A Office" │  │  "Client B Campus" │                 │   │
│  │  │                    │  │                    │                 │   │
│  │  │  ┌──────────────┐  │  │  ┌──────────────┐  │                 │   │
│  │  │  │ Magic Key 1  │  │  │  │ Magic Key 1  │  │                 │   │
│  │  │  │ (field tech) │  │  │  │ (surveyor A) │  │                 │   │
│  │  │  └──────────────┘  │  │  └──────────────┘  │                 │   │
│  │  │  ┌──────────────┐  │  │  ┌──────────────┐  │                 │   │
│  │  │  │ Magic Key 2  │  │  │  │ Magic Key 2  │  │                 │   │
│  │  │  │ (customer)   │  │  │  │ (surveyor B) │  │                 │   │
│  │  │  └──────────────┘  │  │  └──────────────┘  │                 │   │
│  │  │                    │  │                    │                 │   │
│  │  │  ┌──────────────┐  │  │  ┌──────────────┐  │                 │   │
│  │  │  │  Survey 1    │  │  │  │  Survey 1    │  │                 │   │
│  │  │  │  Survey 2    │  │  │  │  Survey 2    │  │                 │   │
│  │  │  │  Survey 3    │  │  │  │  Survey 3    │  │                 │   │
│  │  │  └──────────────┘  │  │  └──────────────┘  │                 │   │
│  │  └────────────────────┘  └────────────────────┘                 │   │
│  │                                                                  │   │
│  │  Members:                                                        │   │
│  │  - admin@acme.com (Owner)                                       │   │
│  │  - tech1@acme.com (Member)                                      │   │
│  │  - tech2@acme.com (Member)                                      │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                          │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                        Organization                              │   │
│  │  (e.g., "BigCorp IT")                                           │   │
│  │                                                                  │   │
│  │  ... (similar structure)                                        │   │
│  └─────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Entity Definitions

### User

An authenticated individual with a platform account.

```typescript
interface User {
    id: string;                    // Unique user ID
    email: string;                 // Primary email
    handle: string;                // Display handle (from OAuth or set)
    authProvider: AuthProvider;    // How they authenticate
    createdAt: DateTime;
    lastLoginAt: DateTime;
    
    // Derived
    organizations: OrganizationMembership[];
}

enum AuthProvider {
    Plain,      // Username/password
    GitHub,
    Google,
    LinkedIn,
    Bluesky
}
```

### Organization

A group of users who share access to projects and surveys.

```typescript
interface Organization {
    id: string;                    // org_xxxxx
    name: string;                  // Display name
    slug: string;                  // URL-friendly identifier
    createdAt: DateTime;
    
    // Billing
    plan: PlanType;
    billingEmail: string;
    
    // Limits (from plan)
    maxProjects: number;
    maxMembersPerProject: number;
    maxMagicKeysPerProject: number;
    surveyRetentionDays: number;
    storageQuotaBytes: number;
}

enum PlanType {
    Free,
    Pro,
    Team,
    Enterprise
}
```

### Organization Membership

Links users to organizations with roles.

```typescript
interface OrganizationMembership {
    userId: string;
    organizationId: string;
    role: OrganizationRole;
    joinedAt: DateTime;
    invitedBy: string;             // User ID of inviter
}

enum OrganizationRole {
    Owner,      // Full control, billing, can delete org
    Admin,      // Manage members, projects, settings
    Member      // Access projects, create surveys
}
```

### Project

A container for related surveys, typically representing a physical site or engagement.

```typescript
interface Project {
    id: string;                    // proj_xxxxx
    organizationId: string;
    name: string;                  // "Building A - 2024 Assessment"
    description: string;
    createdAt: DateTime;
    createdBy: string;             // User ID
    
    // Optional metadata
    location: {
        address?: string;
        coordinates?: {lat: number, lng: number};
    };
    
    // Access control
    magicKeys: MagicKey[];
    
    // Statistics (computed)
    surveyCount: number;
    totalStorageBytes: number;
    lastSurveyAt: DateTime;
}
```

### Magic Key

A shareable, time-limited access token for survey collection.

```typescript
interface MagicKey {
    id: string;                    // mk_xxxxx
    projectId: string;
    
    // The actual key value (included in URLs)
    key: string;                   // e.g., "field-survey-2024-abc123"
    
    // Metadata
    name: string;                  // "Field Tech - January"
    createdAt: DateTime;
    createdBy: string;             // User ID
    
    // Expiration
    expiresAt: DateTime;
    
    // Usage limits (optional)
    maxSurveys?: number;           // Max surveys this key can create
    surveyCount: number;           // Current count
    
    // Status
    isActive: boolean;             // Can be manually deactivated
    lastUsedAt?: DateTime;
}
```

### Survey

A single walk-through assessment captured using WiFi-Verify.

```typescript
interface Survey {
    id: string;                    // survey_xxxxx
    projectId: string;
    
    // Who created it
    createdBy: {
        type: "user" | "magic_key";
        userId?: string;           // If authenticated user
        magicKeyId?: string;       // If via Magic Key
    };
    
    // Timing
    startedAt: DateTime;
    endedAt: DateTime;
    duration: number;              // seconds
    
    // Content
    hasVideo: boolean;
    hasScreenCapture: boolean;     // Premium feature
    hasSensorData: boolean;
    
    // Storage
    videoStorageBytes: number;
    screenStorageBytes: number;
    dataStorageBytes: number;
    
    // Summary metrics (computed)
    summary: {
        avgLatency: number;
        maxLatency: number;
        minLatency: number;
        packetLoss: number;
        jitter: number;
        pathHops: number;
        gpsPoints: number;
    };
    
    // Status
    status: SurveyStatus;
    uploadProgress?: number;       // 0-100 during upload
}

enum SurveyStatus {
    Uploading,
    Processing,
    Ready,
    Failed,
    Deleted
}
```

---

## Access Control Matrix

### Organization Level

| Action | Owner | Admin | Member |
|--------|-------|-------|--------|
| View organization details | ✅ | ✅ | ✅ |
| Edit organization settings | ✅ | ✅ | ❌ |
| Manage billing | ✅ | ❌ | ❌ |
| Invite members | ✅ | ✅ | ❌ |
| Remove members | ✅ | ✅ | ❌ |
| Change member roles | ✅ | ✅* | ❌ |
| Create projects | ✅ | ✅ | ✅ |
| Delete organization | ✅ | ❌ | ❌ |

*Admins cannot change Owner role

### Project Level

| Action | Owner | Admin | Member | Magic Key |
|--------|-------|-------|--------|-----------|
| View project | ✅ | ✅ | ✅ | ❌ |
| Edit project details | ✅ | ✅ | ❌ | ❌ |
| Delete project | ✅ | ✅ | ❌ | ❌ |
| Create Magic Key | ✅ | ✅ | ✅ | ❌ |
| Revoke Magic Key | ✅ | ✅ | ✅* | ❌ |
| View all surveys | ✅ | ✅ | ✅ | ❌ |
| Delete surveys | ✅ | ✅ | ✅* | ❌ |
| Create survey | ✅ | ✅ | ✅ | ✅ |
| Run network test | ✅ | ✅ | ✅ | ✅ |
| Use screen capture | ✅ | ✅ | ✅ | ❌ |

*Own surveys/keys only

### Survey Level

| Action | Survey Creator | Other Org Members | Magic Key User |
|--------|----------------|-------------------|----------------|
| View survey | ✅ | ✅ | ❌ |
| Download data | ✅ | ✅ | ❌ |
| Delete survey | ✅ | ✅ (Admin/Owner) | ❌ |
| Export report | ✅ | ✅ | ❌ |

---

## Magic Key Workflow

### Creating a Magic Key

1. User logs in to WiFi-Verify
2. Navigates to project
3. Clicks "Create Magic Key"
4. Configures:
   - Name (for identification)
   - Expiration (hours/days/custom date)
   - Usage limit (optional)
5. Receives shareable URL

### Magic Key URL Format

```
https://app.wifi-verify.com/survey?key=field-survey-2024-abc123
```

Or with custom domain:
```
https://survey.clientname.com/?key=field-survey-2024-abc123
```

### Using a Magic Key

1. Recipient opens Magic Key URL
2. Browser validates key with server
3. If valid:
   - Session cookie set (`survey_session=survey_{key}_{timestamp}_{uuid}`)
   - Redirected to survey capture page
   - Limited UI (no project management, no org settings)
4. User performs survey
5. Survey uploaded with `magic_key_id` association
6. Survey appears in project for org members to review

### Magic Key Session Format

```
survey_{magic_key}_{unix_timestamp}_{uuid}
```

Example:
```
survey_field-survey-2024-abc123_1705312200_550e8400-e29b-41d4-a716-446655440000
```

Validation checks:
1. Prefix is `survey_`
2. Magic key exists in project
3. Magic key is active (not revoked)
4. Session timestamp + timeout > current time
5. Usage count < max surveys (if limit set)

---

## API Endpoints

### Organizations

```
GET    /api/organizations                    # List user's orgs
POST   /api/organizations                    # Create org
GET    /api/organizations/:orgId             # Get org details
PATCH  /api/organizations/:orgId             # Update org
DELETE /api/organizations/:orgId             # Delete org

GET    /api/organizations/:orgId/members     # List members
POST   /api/organizations/:orgId/members     # Invite member
PATCH  /api/organizations/:orgId/members/:userId  # Update role
DELETE /api/organizations/:orgId/members/:userId  # Remove member
```

### Projects

```
GET    /api/organizations/:orgId/projects    # List projects
POST   /api/organizations/:orgId/projects    # Create project
GET    /api/projects/:projectId              # Get project
PATCH  /api/projects/:projectId              # Update project
DELETE /api/projects/:projectId              # Delete project
```

### Magic Keys

```
GET    /api/projects/:projectId/keys         # List Magic Keys
POST   /api/projects/:projectId/keys         # Create Magic Key
GET    /api/keys/:keyId                      # Get key details
PATCH  /api/keys/:keyId                      # Update (e.g., deactivate)
DELETE /api/keys/:keyId                      # Revoke key

GET    /api/keys/validate?key=xxx            # Validate key (public)
```

### Surveys

```
GET    /api/projects/:projectId/surveys      # List surveys
POST   /api/surveys/upload                   # Upload survey
GET    /api/surveys/:surveyId                # Get survey metadata
GET    /api/surveys/:surveyId/video          # Stream video
GET    /api/surveys/:surveyId/screen         # Stream screen capture
GET    /api/surveys/:surveyId/data           # Get sensor + network data
DELETE /api/surveys/:surveyId                # Delete survey
POST   /api/surveys/:surveyId/export         # Generate PDF report
```

---

## Database Schema (Conceptual)

```sql
-- Users
CREATE TABLE users (
    id UUID PRIMARY KEY,
    email VARCHAR(255) UNIQUE NOT NULL,
    handle VARCHAR(100),
    auth_provider VARCHAR(50) NOT NULL,
    password_hash VARCHAR(255),  -- For plain auth
    created_at TIMESTAMP NOT NULL,
    last_login_at TIMESTAMP
);

-- Organizations
CREATE TABLE organizations (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(100) UNIQUE NOT NULL,
    plan VARCHAR(50) NOT NULL DEFAULT 'free',
    billing_email VARCHAR(255),
    created_at TIMESTAMP NOT NULL,
    
    -- Limits
    max_projects INTEGER NOT NULL DEFAULT 3,
    max_members INTEGER NOT NULL DEFAULT 5,
    storage_quota_bytes BIGINT NOT NULL DEFAULT 1073741824,  -- 1GB
    survey_retention_days INTEGER NOT NULL DEFAULT 30
);

-- Organization Memberships
CREATE TABLE organization_memberships (
    user_id UUID REFERENCES users(id),
    organization_id UUID REFERENCES organizations(id),
    role VARCHAR(50) NOT NULL DEFAULT 'member',
    joined_at TIMESTAMP NOT NULL,
    invited_by UUID REFERENCES users(id),
    PRIMARY KEY (user_id, organization_id)
);

-- Projects
CREATE TABLE projects (
    id UUID PRIMARY KEY,
    organization_id UUID REFERENCES organizations(id) NOT NULL,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    location_address VARCHAR(500),
    location_lat DECIMAL(10, 8),
    location_lng DECIMAL(11, 8),
    created_at TIMESTAMP NOT NULL,
    created_by UUID REFERENCES users(id)
);

-- Magic Keys
CREATE TABLE magic_keys (
    id UUID PRIMARY KEY,
    project_id UUID REFERENCES projects(id) NOT NULL,
    key VARCHAR(100) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    created_at TIMESTAMP NOT NULL,
    created_by UUID REFERENCES users(id),
    expires_at TIMESTAMP NOT NULL,
    max_surveys INTEGER,
    survey_count INTEGER NOT NULL DEFAULT 0,
    is_active BOOLEAN NOT NULL DEFAULT true,
    last_used_at TIMESTAMP
);

-- Surveys
CREATE TABLE surveys (
    id UUID PRIMARY KEY,
    project_id UUID REFERENCES projects(id) NOT NULL,
    
    -- Creator (either user or magic key)
    created_by_user_id UUID REFERENCES users(id),
    created_by_magic_key_id UUID REFERENCES magic_keys(id),
    
    -- Timing
    started_at TIMESTAMP NOT NULL,
    ended_at TIMESTAMP NOT NULL,
    duration_seconds INTEGER NOT NULL,
    
    -- Content flags
    has_video BOOLEAN NOT NULL DEFAULT false,
    has_screen_capture BOOLEAN NOT NULL DEFAULT false,
    has_sensor_data BOOLEAN NOT NULL DEFAULT false,
    
    -- Storage
    video_storage_bytes BIGINT NOT NULL DEFAULT 0,
    screen_storage_bytes BIGINT NOT NULL DEFAULT 0,
    data_storage_bytes BIGINT NOT NULL DEFAULT 0,
    
    -- Summary metrics (JSON for flexibility)
    summary JSONB,
    
    -- Status
    status VARCHAR(50) NOT NULL DEFAULT 'processing',
    
    -- Storage references
    video_path VARCHAR(500),
    screen_path VARCHAR(500),
    data_path VARCHAR(500),
    
    CHECK (created_by_user_id IS NOT NULL OR created_by_magic_key_id IS NOT NULL)
);

-- Indexes
CREATE INDEX idx_org_memberships_org ON organization_memberships(organization_id);
CREATE INDEX idx_projects_org ON projects(organization_id);
CREATE INDEX idx_magic_keys_project ON magic_keys(project_id);
CREATE INDEX idx_magic_keys_key ON magic_keys(key);
CREATE INDEX idx_surveys_project ON surveys(project_id);
CREATE INDEX idx_surveys_created_at ON surveys(created_at);
```

---

## Billing Integration

### Plan Limits

| Feature | Free | Pro | Team | Enterprise |
|---------|------|-----|------|------------|
| Organizations | 1 | 1 | 3 | Unlimited |
| Projects per org | 3 | 10 | 50 | Unlimited |
| Members per org | 1 | 1 | 10 | Unlimited |
| Magic Keys per project | 2 | 10 | 50 | Unlimited |
| Surveys per month | 10 | 100 | 500 | Unlimited |
| Storage | 1 GB | 10 GB | 100 GB | Custom |
| Video retention | 7 days | 30 days | 90 days | Custom |
| Screen capture | ❌ | ❌ | ✅ | ✅ |

### Enforcement Points

1. **Create organization**: Check org limit
2. **Create project**: Check project limit per org
3. **Invite member**: Check member limit per org
4. **Create Magic Key**: Check key limit per project
5. **Upload survey**: Check survey count and storage quota
6. **Access screen capture**: Check plan includes feature

---

## Migration Path

### Current State (v1)
- Single Magic Key list in config file
- No organization structure
- Surveys stored locally only

### Target State (v2)
- Database-backed user management
- Organization hierarchy
- Magic Keys per project
- Server-side survey storage

### Migration Steps

1. **Add database schema** (new tables, no breaking changes)
2. **Add user registration/login** (alongside existing auth)
3. **Create "default" organization** for existing users
4. **Migrate config-file Magic Keys** to database
5. **Add project management UI**
6. **Add survey upload API**
7. **Deprecate config-file Magic Keys**
8. **Remove legacy code paths**
