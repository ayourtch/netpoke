# WiFi-Verify: Go-to-Market Strategy

## Executive Summary

WiFi-Verify enters the market as a **complementary tool to RF survey solutions**, focusing on what RF tools cannot measure: **end-to-end application-layer network performance**. The strategy targets professionals who already understand the value of network assessment and positions WiFi-Verify as the missing piece in their toolkit.

**Core Positioning**: "Measure what applications actually experience, not just what the radio sees."

---

## Market Opportunity

### The Gap in Existing Solutions

| Tool Category | What It Measures | What It Misses |
|---------------|------------------|----------------|
| **RF Survey Tools** (Ekahau, NetSpot) | Signal strength, noise, interference, channel utilization | End-to-end latency, packet loss, path issues, backhaul problems |
| **Consumer Speed Tests** (Speedtest, Fast.com) | Download/upload speed, basic ping | Jitter, packet loss, traceroute, MTU issues, continuous monitoring |
| **Enterprise NPM** (ThousandEyes, Catchpoint) | Comprehensive network metrics | Expensive ($20K+/year), requires agents, complex deployment |

**WiFi-Verify fills the middle**: More than consumer tools, simpler than enterprise, complementary to RF.

### Target Market Size

| Segment | Size (Global) | Addressable |
|---------|---------------|-------------|
| WiFi Installation Companies | 50,000+ | 10,000 |
| MSPs | 40,000+ | 15,000 |
| Enterprise IT Teams | 500,000+ | 50,000 |
| Hospitality/Retail Chains | 1.5M+ locations | 5,000 chains |

**Initial TAM**: ~80,000 potential customers
**Year 1 Target**: 500 paying customers

---

## Positioning Strategy

### Primary Message

> **"The application-layer companion to your RF survey."**

WiFi-Verify doesn't replace Ekahau or NetSpot—it completes the picture by showing how applications actually perform over the network you've surveyed.

### Supporting Messages

1. **For RF Survey Users**:
   > "Your RF survey shows great signal. WiFi-Verify shows whether apps will actually work."

2. **For IT Troubleshooting**:
   > "Users complain about WiFi but your metrics look fine. WiFi-Verify shows what they're actually experiencing."

3. **For Remote Diagnostics**:
   > "Can't be on-site? Send a link. Get full network diagnostics in 60 seconds."

4. **For MSPs/Consultants**:
   > "Include professional network assessments in every engagement without expensive equipment."

### Differentiation Summary

| We Are | We Are Not |
|--------|------------|
| Application-layer measurement | RF signal analyzer |
| Browser-based (zero install) | Native app requiring deployment |
| Complementary to RF tools | Replacement for RF tools |
| Walk-through survey capability | Heatmap generator (yet) |
| Affordable ($49-499/mo) | Enterprise-priced ($20K+/year) |

---

## Target Customer Segments

### Tier 1: Early Adopters (Months 1-6)

#### Segment: WiFi Professionals & Consultants

**Profile**:
- Independent WiFi consultants
- Small WiFi installation companies (1-10 people)
- Already use Ekahau/NetSpot
- Understand value of comprehensive surveys

**Pain Points**:
- "I can show signal strength but can't prove app performance"
- "Clients blame WiFi for problems that are actually backhaul/internet"
- "Need to differentiate from competitors"

**Value Proposition**:
> "Complement your RF survey with application-layer proof. Show clients exactly how their apps will perform."

**Acquisition Channels**:
- WLAN Pros community
- Ekahau user forums
- WiFi-focused YouTube (e.g., WLAN Pros channel)
- LinkedIn (WiFi professionals)

**Pricing Sensitivity**: Medium ($49-149/mo acceptable for valuable tool)

---

#### Segment: Network Engineers & Enthusiasts

**Profile**:
- Technically curious
- Active on Reddit (r/networking, r/homelab)
- Read Hacker News
- Early adopters of new tools

**Pain Points**:
- "Speedtest doesn't give me enough detail"
- "Want to understand my network path"
- "Debugging complex network issues"

**Value Proposition**:
> "Real traceroute from your browser. See exactly what's happening on your network."

**Acquisition Channels**:
- Hacker News launch
- Reddit (organic, helpful participation)
- Technical blog posts
- GitHub (open-source client library)

**Pricing Sensitivity**: High (expect free tier, may convert for advanced features)

---

### Tier 2: Growth Phase (Months 6-12)

#### Segment: Managed Service Providers (MSPs)

**Profile**:
- 5-50 person IT service companies
- Manage multiple client networks
- Already have RMM/PSA tools
- Value efficiency and documentation

**Pain Points**:
- "Truck rolls are expensive"
- "Need to diagnose client issues remotely"
- "Want to include network assessment in service packages"
- "Documentation for compliance/audits"

**Value Proposition**:
> "Diagnose client network issues without site visits. Magic Key links let clients collect data you can analyze."

**Acquisition Channels**:
- MSP communities (ASCII, Channel Futures, CompTIA)
- RMM/PSA marketplace integrations
- MSP-focused webinars
- Partner referrals

**Pricing Sensitivity**: Medium (value ROI over cost, team pricing important)

---

#### Segment: Enterprise IT Operations

**Profile**:
- Large IT departments (50+ people)
- Managing campus/multi-building networks
- Remote/hybrid workforce support
- Compliance requirements

**Pain Points**:
- "WFH employees have WiFi issues we can't diagnose"
- "Need to validate network performance for VoIP/video"
- "Expensive enterprise tools are overkill for this use case"
- "Can't install agents on every device"

**Value Proposition**:
> "Diagnose any employee's network without agents. Works on any device with a browser."

**Acquisition Channels**:
- Direct outreach (LinkedIn, email)
- Conference presence (Gartner IOCS)
- Analyst briefings
- Case studies from MSP customers

**Pricing Sensitivity**: Low (budget available for proven solutions)

---

### Tier 3: Expansion (Year 2+)

#### Segment: Hospitality & Retail Chains

**Profile**:
- Multi-location businesses
- Guest WiFi is critical for experience/reviews
- Franchise or corporate-managed
- Limited technical staff per location

**Pain Points**:
- "WiFi complaints hurt our reviews"
- "Can't survey every location affordably"
- "Need consistent quality across all properties"

**Value Proposition**:
> "Audit network quality at every location. Have any staff member do a 5-minute walkthrough with their phone."

**Acquisition Channels**:
- Hospitality technology conferences
- Industry publications
- Partnerships with POS/PMS vendors
- Direct enterprise sales

**Pricing Sensitivity**: Low (value exceeds cost if solves problem)

---

## Go-to-Market Phases

### Phase 1: Technical Credibility (Months 1-3)

**Objective**: Establish WiFi-Verify as a technically legitimate tool among network professionals.

**Activities**:

1. **Hacker News Launch**
   - Technical deep-dive post: "How We Built Browser-Based Traceroute with WebRTC"
   - Focus on technical innovation, not marketing
   - Engage authentically with comments

2. **Technical Content**
   - Blog: "Why RF Surveys Aren't Enough: The Case for Application-Layer Measurement"
   - Blog: "SCTP, DTLS, and Per-Packet Socket Options: Modifying WebRTC for Network Measurement"
   - Blog: "MTU Discovery Without ICMP: A WebRTC Approach"

3. **Community Participation**
   - Reddit: Helpful answers in r/networking, r/sysadmin, r/wifi
   - WLAN Pros Discord: Introduce tool, get feedback
   - GitHub: Open-source WASM client library

4. **Product Hunt Launch**
   - Target #1 in Developer Tools category
   - Coordinate with early users for upvotes/reviews

**Success Metrics**:
- 10K unique tests
- 1K registered users
- 500 GitHub stars
- 3 technical blog posts ranking in search

---

### Phase 2: Monetization & MSP Focus (Months 3-6)

**Objective**: Convert early adopters to paying customers, focus on MSP segment.

**Activities**:

1. **Pricing Launch**
   - Introduce Pro/Team/Enterprise tiers
   - Grandfather early users with discount

2. **MSP Channel Development**
   - Partner with 2-3 MSP communities for promotion
   - Create MSP-specific case study
   - Develop "Remote Network Assessment" positioning

3. **Integration Development**
   - ConnectWise marketplace listing
   - Datto integration (or partnership discussion)
   - API documentation for RMM integration

4. **Webinar Series**
   - "Remote Network Diagnostics: Reduce Truck Rolls by 40%"
   - "Adding Network Assessment to Your Service Stack"
   - Partner webinars with MSP communities

5. **Referral Program**
   - 20% revenue share for MSP referrals
   - Partner portal for tracking

**Success Metrics**:
- 100 paying customers
- $15K MRR
- 2 RMM integrations live
- 5 case studies published

---

### Phase 3: Survey Feature & Enterprise (Months 6-12)

**Objective**: Launch survey feature, begin enterprise sales motion.

**Activities**:

1. **Survey Feature Launch**
   - Full camera + sensor + network integration
   - Survey playback UI
   - Organization/project structure

2. **WiFi Professional Campaign**
   - "The Application-Layer Companion to Your RF Survey"
   - Side-by-side comparison content (RF + WiFi-Verify)
   - Partnerships with Ekahau resellers

3. **Enterprise Sales**
   - Hire 1 Account Executive
   - Target 10 enterprise prospects
   - Develop enterprise pricing/packaging

4. **Conference Presence**
   - WirelessLAN Professionals Conference (booth, demo)
   - Network Field Day (influencer demo)
   - BICSI Fall (WiFi installers)

5. **Screen Capture (Premium)**
   - Launch Ekahau screen capture feature
   - Position as premium add-on
   - Target serious WiFi professionals

**Success Metrics**:
- 500 paying customers
- $50K MRR
- 3 enterprise deals ($20K+ ACV each)
- Survey feature used by 30% of active users

---

## Pricing Strategy

### Tiered Model

| Tier | Price | Target Segment | Key Features |
|------|-------|----------------|--------------|
| **Free** | $0 | Enthusiasts, trial | 5 tests/day, basic metrics |
| **Pro** | $49/mo | Consultants, solo IT | Unlimited tests, traceroute, MTU, 10 surveys/mo |
| **Team** | $149/mo | MSPs, IT teams (5 users) | Magic Keys, survey management, API, 50 surveys/mo |
| **Enterprise** | $499/mo+ | Large IT, hospitality | SSO, unlimited users/surveys, screen capture, SLA |

### Pricing Psychology

1. **Free tier exists for virality** (but limited enough to convert)
2. **Pro is "obvious value"** for anyone doing this professionally
3. **Team includes collaboration** (Magic Keys are key differentiator)
4. **Enterprise is negotiable** (value-based, not feature-based)

### Annual Discounts
- 2 months free on annual plans (17% discount)
- Required for Enterprise tier

---

## Marketing Channels

### Owned Channels

| Channel | Purpose | Frequency |
|---------|---------|-----------|
| Blog | SEO, thought leadership | 2 posts/month |
| Newsletter | Nurture, product updates | Monthly |
| Documentation | Self-service education | Continuous |
| YouTube | Tutorials, demos | 2 videos/month |

### Earned Channels

| Channel | Approach |
|---------|----------|
| Hacker News | Technical posts, not promotional |
| Reddit | Helpful participation, occasional tool mention |
| Twitter/X | Network engineering discussions |
| Podcasts | Guest appearances (Packet Pushers, etc.) |

### Paid Channels (Phase 2+)

| Channel | Budget | Target |
|---------|--------|--------|
| Google Ads | $2K/mo | "network diagnostic tool", "wifi survey tool" |
| LinkedIn Ads | $3K/mo | IT Managers, Network Engineers |
| Retargeting | $1K/mo | Free users with 3+ tests |

---

## Content Strategy

### SEO Target Keywords

| Keyword | Volume | Difficulty | Intent |
|---------|--------|------------|--------|
| "browser traceroute" | 1.2K/mo | Low | High |
| "wifi jitter test" | 1.8K/mo | Low | High |
| "network path test" | 890/mo | Low | High |
| "mtu discovery tool" | 720/mo | Low | High |
| "remote network diagnostics" | 590/mo | Low | High |
| "application layer monitoring" | 2.4K/mo | Medium | Medium |

### Content Pillars

1. **Technical Deep-Dives**
   - How the technology works
   - WebRTC internals
   - Network measurement methodology

2. **Use Case Guides**
   - "How to Diagnose Remote User WiFi Issues"
   - "Validating WiFi Performance After Installation"
   - "Network Assessment for VoIP Readiness"

3. **Comparison Content**
   - "WiFi-Verify vs Speedtest: When You Need More"
   - "RF Surveys + Application Testing: A Complete Approach"
   - "ThousandEyes vs WiFi-Verify: Choosing the Right Tool"

4. **Industry Insights**
   - Trends in network performance
   - Remote work infrastructure
   - WiFi 6/6E/7 performance reality

---

## Partnerships

### Technology Partnerships

| Partner Type | Examples | Value |
|--------------|----------|-------|
| WiFi vendors | Ubiquiti, Aruba, Meraki | Community access, potential integration |
| RMM/PSA | ConnectWise, Datto | Distribution channel |
| WebRTC platforms | Twilio, Vonage | Mutual promotion, use case validation |

### Channel Partnerships

| Partner Type | Examples | Model |
|--------------|----------|-------|
| MSP communities | ASCII, CompTIA | Sponsorship, webinars |
| WiFi consultants | Independent | Referral revenue share |
| IT distributors | Ingram, Tech Data | Reseller (Enterprise tier) |

### Content Partnerships

| Partner | Opportunity |
|---------|-------------|
| Packet Pushers | Podcast sponsorship, guest appearance |
| NetworkChuck | YouTube review/demo |
| WLAN Pros | Event sponsorship, content collaboration |

---

## Success Metrics & Milestones

### Year 1 Milestones

| Milestone | Target Date | Metric |
|-----------|-------------|--------|
| Hacker News front page | Month 2 | 200+ points |
| 1,000 registered users | Month 3 | Organic signups |
| First 10 paying customers | Month 4 | $500 MRR |
| Product Hunt Top 5 | Month 4 | 500+ upvotes |
| 100 paying customers | Month 6 | $10K MRR |
| First enterprise deal | Month 8 | $20K ACV |
| Survey feature launch | Month 8 | 30% feature adoption |
| 500 paying customers | Month 12 | $50K MRR |

### Key Performance Indicators (KPIs)

| KPI | Target | Measurement |
|-----|--------|-------------|
| MRR Growth | 20% MoM | Stripe/billing system |
| Free-to-Paid Conversion | 5% | User analytics |
| Customer Acquisition Cost | <$200 | Marketing spend / new customers |
| Net Revenue Retention | >100% | Expansion revenue / churn |
| Survey Completion Rate | >80% | Survey started / survey uploaded |

---

## Risks & Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Ookla/Cloudflare launches similar | Medium | High | Technical moat (modified WebRTC), vertical focus |
| Slow enterprise sales cycle | High | Medium | Focus on SMB/MSP for early revenue |
| Survey feature complexity | Medium | Medium | Phased rollout, MVP first |
| WebRTC browser changes | Low | High | Monitor standards, maintain compatibility |
| Ekahau sees us as competitor | Low | Low | Position as complementary, seek partnership |

---

## Budget Allocation (Year 1)

| Category | Q1-Q2 | Q3-Q4 | Total |
|----------|-------|-------|-------|
| Content/SEO | $5K | $10K | $15K |
| Paid Ads | $0 | $15K | $15K |
| Events/Conferences | $2K | $15K | $17K |
| Tools/Infrastructure | $3K | $5K | $8K |
| Partnerships/Sponsorships | $5K | $10K | $15K |
| **Total** | **$15K** | **$55K** | **$70K** |

*Assumes bootstrapped/seed stage. Scale with revenue.*
