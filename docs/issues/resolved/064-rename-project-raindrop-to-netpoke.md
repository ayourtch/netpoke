# Issue 064: Rename Project Raindrops to NetPoke

## Summary
All references to "Project Raindrops" throughout the codebase should be renamed to "NetPoke" with the descriptor "clientless network tester" where appropriate.

## Location
- `auth/src/views.rs` - Login and access denied page HTML
- `auth/src/lib.rs` - Doc comments
- `auth/src/providers/github.rs` - User-Agent header
- `server/static/public/index.html` - HTML title and headings
- `server/static/public/client-metadata.json` - Client name
- `docs/AUTH_LIBRARY.md`, `docs/AUTHENTICATION.md`, `docs/OAUTH_IMPLEMENTATION.md`, `docs/PLAIN_LOGIN_EXAMPLE.md`, `docs/plans/design-style-guide.md`
- `AGENTS.md`

## Current Behavior
The authentication system and various UI pages are branded as "Project Raindrops" which is a legacy name.

## Expected Behavior
All "Project Raindrops" references should be replaced with "NetPoke" and the product should be described as a "clientless network tester" where a subtitle or description is used.

## Impact
- **Priority: Medium**
- Branding inconsistency — the product is called NetPoke but auth pages still say "Project Raindrops"

## Suggested Implementation
1. Replace "Project Raindrops" with "NetPoke" in all source code, HTML, and documentation
2. Replace "Project-Raindrops-Auth" User-Agent with "NetPoke-Auth"
3. Update subtitles/descriptions to use "Clientless Network Tester" where appropriate
4. Update AGENTS.md branding guidelines

---
*Created: 2026-02-07*

## Resolution

Renamed all "Project Raindrops" references to "NetPoke" across the entire codebase.

### Files modified:
- `auth/src/views.rs` — Login page title, heading, subtitle ("Clientless Network Tester"), footer; Access denied page title and footer
- `auth/src/lib.rs` — Doc comments updated
- `auth/src/providers/github.rs` — User-Agent changed from "Project-Raindrops-Auth" to "NetPoke-Auth"
- `server/static/public/index.html` — Title, headings, and footers updated
- `server/static/public/client-metadata.json` — Client name updated
- `docs/AUTH_LIBRARY.md` — Title and feature description
- `docs/AUTHENTICATION.md` — Route description and views description
- `docs/OAUTH_IMPLEMENTATION.md` — Branding references
- `docs/PLAIN_LOGIN_EXAMPLE.md` — Theme reference
- `docs/plans/design-style-guide.md` — Product names, login/access denied page descriptions
- `AGENTS.md` — Branding guidelines updated

### Verified:
- `cargo check -p netpoke-auth` — compiles successfully
- `cargo check -p netpoke-server` — compiles successfully
- No remaining "raindrop" references in source code, HTML, or documentation

---
*Resolved: 2026-02-07*
