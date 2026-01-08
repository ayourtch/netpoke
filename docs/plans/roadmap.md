# WiFi-Verify: Development Roadmap

## Overview

This roadmap outlines the development phases and milestones for WiFi-Verify, from the current state to a fully-featured platform. The roadmap is organized into six phases spanning approximately 18-24 months.

**Current State**: Working prototype with modified WebRTC stack enabling browser-based traceroute, MTU discovery, and network measurement. Camera-tracker survey prototype exists but is not integrated with the main measurement flow.

---

## Phase 1: Foundation (Months 1-3)

### Objective
Stabilize the core platform, integrate survey capture, and prepare for initial public launch.

### Milestones

#### 1.1 Survey Integration (Month 1-2)
- [ ] Integrate `camera-tracker.html` with main `nettest.html` interface
- [ ] Add `survey_session_id` to all WebRTC control messages
- [ ] Implement IndexedDB storage for video/sensor data during surveys
- [ ] Create survey upload API endpoint (`POST /api/survey/upload`)
- [ ] Basic survey list and retrieval endpoints

**Deliverable**: Users can perform a walk-through survey that captures video, sensors, and network metrics with a common session ID.

#### 1.2 Local-First Storage (Month 2)
- [ ] Implement IndexedDB schema for pending surveys
- [ ] Separate video blob storage for memory efficiency
- [ ] Survey status tracking (pending, uploading, uploaded, failed)
- [ ] Local survey preview before upload
- [ ] Delete local survey functionality

**Deliverable**: Surveys are reliably stored locally and can be managed before upload.

#### 1.3 Upload Flow (Month 2-3)
- [ ] Chunked upload with resumability (5MB chunks)
- [ ] Upload progress tracking and UI
- [ ] Pause/resume upload capability
- [ ] Network-aware upload (warn on cellular)
- [ ] Server-side chunk assembly and validation

**Deliverable**: Large survey files can be reliably uploaded with progress indication.

#### 1.4 Technical Documentation (Month 3)
- [ ] API documentation (OpenAPI/Swagger)
- [ ] WebRTC protocol documentation
- [ ] Deployment guide
- [ ] Development setup guide

**Deliverable**: New developers can onboard and external integrators can use the API.

### Success Criteria
- Survey capture integrated with network testing
- Upload success rate >95%
- End-to-end survey flow working on iOS Safari, Chrome, Firefox

---

## Phase 2: Organization Model (Months 3-5)

### Objective
Implement the multi-tenant organization structure to enable team usage and project-based organization.

### Milestones

#### 2.1 Database Schema (Month 3-4)
- [ ] Design and implement database schema (PostgreSQL recommended)
- [ ] Users table with OAuth provider support
- [ ] Organizations table with plan tiers
- [ ] Organization memberships with roles (Owner/Admin/Member)
- [ ] Projects table linked to organizations
- [ ] Surveys table linked to projects
- [ ] Migration from config-file Magic Keys to database

**Deliverable**: Database foundation for multi-tenant operations.

#### 2.2 User Management (Month 4)
- [ ] User registration flow
- [ ] User profile page
- [ ] Account settings (password change, email update)
- [ ] User deletion/data export (GDPR compliance)

**Deliverable**: Full user lifecycle management.

#### 2.3 Organization Management (Month 4-5)
- [ ] Create organization flow
- [ ] Organization settings page
- [ ] Member invitation via email
- [ ] Role management UI
- [ ] Member removal
- [ ] Organization deletion with data cleanup

**Deliverable**: Teams can create and manage organizations.

#### 2.4 Project Management (Month 5)
- [ ] Create project within organization
- [ ] Project settings and metadata (name, description, location)
- [ ] Project dashboard with survey list
- [ ] Project deletion with associated data

**Deliverable**: Work can be organized into discrete projects.

### Success Criteria
- Organizations can be created with multiple members
- Projects contain surveys from multiple team members
- Role-based access control working correctly

---

## Phase 3: Magic Key System (Months 5-7)

### Objective
Enable shareable, time-limited access for field surveys without full account creation.

### Milestones

#### 3.1 Magic Key CRUD (Month 5-6)
- [ ] Create Magic Key within project
- [ ] Magic Key configuration (name, expiration, usage limit)
- [ ] List Magic Keys for project
- [ ] Revoke/deactivate Magic Key
- [ ] Magic Key usage tracking

**Deliverable**: Project owners can create and manage Magic Keys.

#### 3.2 Magic Key Authentication (Month 6)
- [ ] URL format: `/survey?key=<magic_key>`
- [ ] Session cookie generation: `survey_{key}_{timestamp}_{uuid}`
- [ ] Validation middleware (key exists, active, not expired, within usage limit)
- [ ] Limited UI for Magic Key users (survey only, no project management)

**Deliverable**: Magic Key links work for unauthenticated survey collection.

#### 3.3 Magic Key Survey Flow (Month 6-7)
- [ ] Magic Key surveys automatically linked to project
- [ ] Magic Key usage counter increment
- [ ] Survey appears in project for authenticated users
- [ ] Magic Key metadata attached to survey (which key, when used)

**Deliverable**: Field technicians can collect surveys via Magic Key links.

#### 3.4 Magic Key UI Polish (Month 7)
- [ ] Shareable link generation with copy button
- [ ] QR code generation for in-person sharing
- [ ] Email template for sending Magic Key links
- [ ] Magic Key expiration warnings

**Deliverable**: Smooth workflow for sharing Magic Keys with field teams.

### Success Criteria
- Magic Key surveys appear in correct project
- Expired/revoked keys properly rejected
- Usage limits enforced

---

## Phase 4: Survey Playback & Analysis (Months 7-10)

### Objective
Build the survey playback UI that correlates video, sensors, and network metrics on a synchronized timeline.

### Milestones

#### 4.1 Survey Viewer MVP (Month 7-8)
- [ ] Video playback with standard controls
- [ ] Timeline scrubbing
- [ ] Basic metric display (latency, loss, jitter at current time)
- [ ] Video-metric synchronization via timestamps

**Deliverable**: Basic survey review capability.

#### 4.2 Chart Integration (Month 8-9)
- [ ] Time-series latency chart (Chart.js)
- [ ] Packet loss chart
- [ ] Jitter chart
- [ ] Chart follows video playback position
- [ ] Click-to-seek from chart to video
- [ ] Zoom/pan on charts

**Deliverable**: Visual correlation between video and metrics.

#### 4.3 Event Markers (Month 9)
- [ ] Automatic markers for high latency events
- [ ] Automatic markers for packet loss events
- [ ] Automatic markers for path changes (if traceroute during survey)
- [ ] Manual annotation capability
- [ ] Jump-to-marker functionality

**Deliverable**: Quick identification of problem areas.

#### 4.4 Sensor Visualization (Month 9-10)
- [ ] GPS track on map (if available)
- [ ] Compass heading indicator
- [ ] Acceleration/movement graph
- [ ] Sensor data correlation with video position

**Deliverable**: Full spatial context for survey review.

#### 4.5 Export & Reporting (Month 10)
- [ ] PDF report generation
- [ ] Summary statistics
- [ ] Key screenshots from video
- [ ] Metric summaries per location (if GPS)
- [ ] CSV export for raw data

**Deliverable**: Shareable reports for clients.

### Success Criteria
- Survey playback smooth on desktop browsers
- Video-metric sync accurate to <100ms
- PDF reports generated successfully

---

## Phase 5: Premium Features (Months 10-14)

### Objective
Add differentiated features for higher-tier subscriptions.

### Milestones

#### 5.1 Screen Capture (Month 10-11)
- [ ] `getDisplayMedia()` integration for screen capture
- [ ] Window selection UI
- [ ] Dual-stream recording (camera + screen)
- [ ] Separate video tracks in storage
- [ ] Side-by-side or PiP playback

**Deliverable**: Capture Ekahau or other RF tool alongside camera.

**Note**: Screen capture requires full authentication (not Magic Key).

#### 5.2 Comparative Analysis (Month 11-12)
- [ ] Compare two surveys side-by-side
- [ ] Before/after analysis view
- [ ] Metric diff highlighting
- [ ] Overlay GPS tracks from multiple surveys

**Deliverable**: Show network improvement after changes.

#### 5.3 Advanced Path Analysis (Month 12-13)
- [ ] Traceroute during survey mode
- [ ] Path visualization at each survey point
- [ ] Path change detection and alerting
- [ ] ECMP visualization (if multiple connections)

**Deliverable**: Network path context during walkthrough.

#### 5.4 API Access (Month 13-14)
- [ ] API key management
- [ ] Rate limiting per tier
- [ ] Webhook notifications for survey completion
- [ ] Integration documentation
- [ ] SDKs for Python, JavaScript

**Deliverable**: Programmatic access for integrations.

### Success Criteria
- Screen capture works on Chrome, Firefox, Edge
- API used by at least 3 beta customers
- Comparative analysis generates meaningful diffs

---

## Phase 6: Enterprise & Scale (Months 14-18+)

### Objective
Enterprise-grade features for large deployments.

### Milestones

#### 6.1 SSO Integration (Month 14-15)
- [ ] SAML 2.0 support
- [ ] OIDC support
- [ ] JIT user provisioning
- [ ] Group/role mapping

**Deliverable**: Enterprise identity integration.

#### 6.2 Advanced Administration (Month 15-16)
- [ ] Organization admin dashboard
- [ ] Usage analytics
- [ ] Storage quotas and management
- [ ] Audit logging
- [ ] Data retention policies

**Deliverable**: Enterprise governance capabilities.

#### 6.3 White-Labeling (Month 16-17)
- [ ] Custom domain support
- [ ] Logo/branding customization
- [ ] Custom email templates
- [ ] Remove WiFi-Verify branding (Enterprise tier)

**Deliverable**: Reseller and enterprise branding.

#### 6.4 Distributed Architecture (Month 17-18)
- [ ] Multi-region server deployment
- [ ] Edge node management
- [ ] Geographic server selection
- [ ] Cross-region survey correlation

**Deliverable**: Global scale deployment.

#### 6.5 Advanced Integrations (Month 18+)
- [ ] ConnectWise/Datto integration
- [ ] ServiceNow integration
- [ ] Slack/Teams notifications
- [ ] Zapier/Make connector

**Deliverable**: Ecosystem integration for enterprise workflows.

### Success Criteria
- SSO working with major providers (Okta, Azure AD)
- Multi-region deployment operational
- First white-label customer live

---

## Technical Debt & Infrastructure

### Ongoing Throughout All Phases

#### Testing
- [ ] Unit test coverage >70%
- [ ] Integration test suite
- [ ] End-to-end test automation (Playwright)
- [ ] Performance benchmarking
- [ ] Cross-browser testing matrix

#### Infrastructure
- [ ] CI/CD pipeline (GitHub Actions)
- [ ] Automated deployments
- [ ] Monitoring and alerting
- [ ] Log aggregation
- [ ] Database backups

#### Security
- [ ] Security audit (external)
- [ ] Penetration testing
- [ ] Dependency vulnerability scanning
- [ ] HTTPS enforcement
- [ ] Rate limiting and abuse prevention

#### Performance
- [ ] WebRTC connection optimization
- [ ] Video encoding optimization
- [ ] Database query optimization
- [ ] CDN for static assets
- [ ] Survey upload optimization

---

## Summary Timeline

```
Month   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15  16  17  18
        │   │   │   │   │   │   │   │   │   │   │   │   │   │   │   │   │   │
Phase 1 ████████████████
        Foundation
        
Phase 2             ████████████████
                    Organization Model
                    
Phase 3                     ████████████████
                            Magic Key System
                            
Phase 4                             ████████████████████████
                                    Survey Playback
                                    
Phase 5                                         ████████████████████████
                                                Premium Features
                                                
Phase 6                                                     ████████████████████
                                                            Enterprise & Scale
```

---

## GTM Alignment

| Roadmap Phase | GTM Phase | Key Deliverables |
|---------------|-----------|------------------|
| Phase 1-2 (Months 1-5) | Technical Credibility | Working survey flow, HN launch material |
| Phase 3 (Months 5-7) | Monetization & MSP Focus | Magic Keys for MSP workflow |
| Phase 4 (Months 7-10) | Survey Feature & Enterprise | Full survey playback, reports |
| Phase 5-6 (Months 10-18) | Enterprise Expansion | SSO, API, integrations |

---

## Risk Mitigation

| Risk | Impact | Mitigation |
|------|--------|------------|
| Survey upload complexity | Phase 1 delay | Start with simplified upload, iterate |
| Database migration complexity | Phase 2 delay | Parallel run with config file initially |
| Magic Key security issues | Trust/reputation | Security review before launch |
| Video playback performance | User experience | Lazy loading, adaptive streaming |
| Screen capture browser support | Feature adoption | Clear browser requirements, graceful degradation |

---

## Resource Requirements

### Phase 1-3 (Foundation)
- 1-2 full-stack developers
- Focus: Rust backend, TypeScript/WASM client

### Phase 4 (Playback)
- Add: 1 frontend specialist
- Focus: Video player, Chart.js, timeline UX

### Phase 5-6 (Enterprise)
- Add: 1 DevOps/infrastructure engineer
- Add: 1 integration developer (for RMM/PSA integrations)

---

## Next Steps

1. **Immediate**: Complete Phase 1.1 (Survey Integration)
2. **This Month**: Define database schema for Phase 2
3. **This Quarter**: Prepare for Hacker News technical launch
4. **This Quarter**: Begin user research with target MSP segment
