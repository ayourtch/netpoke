# NetPoke Documentation

This directory contains all documentation for the netpoke project.

## Table of Contents

### Setup and Configuration
- **[HTTPS_SETUP.md](HTTPS_SETUP.md)** - Guide for enabling HTTPS support with certificates
- **[VERIFICATION_GUIDE.md](VERIFICATION_GUIDE.md)** - Quick guide for verifying traceroute functionality works correctly
- **[GITHUB_ACTIONS.md](GITHUB_ACTIONS.md)** - GitHub Actions workflows for deployment and automation

### Authentication
- **[AUTHENTICATION.md](AUTHENTICATION.md)** - Complete authentication setup guide
- **[AUTH_LIBRARY.md](AUTH_LIBRARY.md)** - Documentation for the reusable authentication library
- **[OAUTH_IMPLEMENTATION.md](OAUTH_IMPLEMENTATION.md)** - OAuth2 implementation summary and architecture
- **[PLAIN_LOGIN_EXAMPLE.md](PLAIN_LOGIN_EXAMPLE.md)** - Example configuration for plain username/password authentication
- **[ALLOWED_USERS_GUIDE.md](ALLOWED_USERS_GUIDE.md)** - Guide for configuring allowed users access control

### Technical Features
- **[UDP_PACKET_OPTIONS.md](UDP_PACKET_OPTIONS.md)** - Comprehensive guide to per-packet UDP socket options (TTL, TOS, DF bit)
- **[DIAGNOSTICS.md](DIAGNOSTICS.md)** - Server diagnostics endpoint for troubleshooting WebRTC connection issues

### Planning and Design Documents
- **[plans/](plans/)** - Design documents and implementation plans for features
  - Network measurement system design and implementation
  - Dashboard cleanup button design
  - Future feature plans

### Historical Documentation
- **[history/](history/)** - Historical bug fixes and implementation notes
  - Traceroute implementation fixes
  - Per-packet options implementation
  - ICMP and error handling fixes
  - See [history/README.md](history/README.md) for details

## Patch Documentation

Patch-specific documentation is located in the [patches/](../patches/) directory:
- **[patches/README.md](../patches/README.md)** - Overview of all vendored crate modifications
- **patches/webrtc-ice/** - ICE layer patches and documentation

## Vendored Dependencies

Documentation for vendored dependencies (README.md, CHANGELOG.md) remains in the respective [vendored/](../vendored/) directories.

## Getting Started

1. For initial setup, start with [AUTHENTICATION.md](AUTHENTICATION.md) to configure access control
2. For HTTPS deployment, see [HTTPS_SETUP.md](HTTPS_SETUP.md)
3. For technical details on the UDP options feature, see [UDP_PACKET_OPTIONS.md](UDP_PACKET_OPTIONS.md)
4. For troubleshooting traceroute, see [VERIFICATION_GUIDE.md](VERIFICATION_GUIDE.md)
5. For diagnosing WebRTC connection issues, see [DIAGNOSTICS.md](DIAGNOSTICS.md)

## Contributing

When adding new documentation:
- Place user-facing documentation in this `docs/` directory
- Place historical/legacy bug fix documentation in `docs/history/`
- Place patch-specific documentation with the patches in `patches/`
- Keep vendor documentation with the vendored code in `vendored/`
