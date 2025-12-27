# BlueSky OAuth Implementation Guide

## Overview

This document outlines the complete BlueSky OAuth service discovery flow based on the article "Connecting to a Blue Sky OAuth server Part 1" by Phill Hallam-Baker.

## Key Concepts

### Handles vs DIDs
- **Handles**: Human-readable identifiers (e.g., `@alice.bsky.social`, `@phill.hallambaker.com`)
- **DIDs**: Machine-readable identifiers that describe public keys (e.g., `did:plc:k647x4n6h3jm347u3t5cm6ki`)
- **Important**: Handles can change and are domain-dependent, DIDs are stable

### Service Discovery Flow

The complete OAuth service discovery consists of 4 steps:

#### Step 1: Convert Handle to DID
- **Method**: DNS TXT record lookup
- **Query**: `_atproto.<handle>`
- **Example**: `TXT _atproto.phill.hallambaker.com` returns `"did=did:plc:k647x4n6h3jm347u3t5cm6ki"`
- **Implementation Note**: Don't assume the DID is the first record - parse all TXT records properly

#### Step 2: Resolve the DID
- **Method**: HTTPS GET request to PLC directory
- **URL**: `https://plc.directory/<did>`
- **Example**: `https://plc.directory/did:plc:k647x4n6h3jm347u3t5cm6ki`
- **Returns**: JSON document containing service descriptions

#### Step 3: Fetch Resource Server Metadata
- **Method**: HTTPS GET request
- **URL**: `<serviceEndpoint>/.well-known/oauth-protected-resource`
- **Example**: `https://shimeji.us-east.host.bsky.network/.well-known/oauth-protected-resource`
- **Purpose**: Identifies the PDS (Personal Data Server) and authorization server

#### Step 4: Get Authorization Server Description
- **Method**: HTTPS GET request
- **URL**: `<authorization_server>/.well-known/oauth-authorization-server`
- **Example**: `https://bsky.social/.well-known/oauth-authorization-server`
- **Returns**: OAuth server metadata including endpoints

## Important Implementation Notes

### Security Considerations
- Implement proper caching for service discovery results
- Validate TLS certificates for all HTTPS requests
- Handle DNS resolution failures gracefully
- Follow RFC6763 for TXT record parsing (don't assume first record)

### Performance Optimizations
- Cache DID resolutions (many users share same servers)
- Cache authorization server metadata
- Implement reasonable TTLs for cached data

### Error Handling
- Handle DNS lookup failures
- Handle HTTPS request failures
- Validate JSON responses
- Provide clear error messages to users

## References

- Handle specification: https://atproto.com/specs/handle
- DID Specification: https://atproto.com/specs/did
- PLC DID Documentation: https://github.com/did-method-plc/did-method-plc
- RFC6763 (DNS-based Service Discovery): https://www.rfc-editor.org/rfc/rfc6763

## Example Complete Flow

For handle `@phill.hallambaker.com`:

1. **DNS Query**: `_atproto.phill.hallambaker.com` → `did:plc:k647x4n6h3jm347u3t5cm6ki`
2. **DID Resolution**: `https://plc.directory/did:plc:k647x4n6h3jm347u3t5cm6ki` → Service document
3. **Resource Metadata**: `https://shimeji.us-east.host.bsky.network/.well-known/oauth-protected-resource` → Authorization server URL
4. **Auth Server Metadata**: `https://bsky.social/.well-known/oauth-authorization-server` → OAuth endpoints

This dynamic discovery enables support for custom domains and prevents hardcoded dependencies on bsky.social infrastructure.