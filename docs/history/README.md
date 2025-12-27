# Historical Bug Fixes and Implementation Notes

This directory contains detailed documentation of historical bug fixes and feature implementations in the wifi-verify project. These documents serve as valuable learning resources and provide insight into the evolution of the codebase.

## Purpose

These documents are preserved for:
- **Learning**: Understanding how complex bugs were diagnosed and fixed
- **Reference**: Detailed technical information about implementation decisions
- **History**: Tracking the evolution of specific features over time

## Documents

### Traceroute Implementation and Fixes
- **TRACEROUTE_FIX_SUMMARY.md** - Comprehensive overview of all traceroute-related fixes, including the critical dangling pointer bug
- **TRACEROUTE_TTL_FIX.md** - Fix for TTL forwarding through the Mux Endpoint layer
- **UDP_LENGTH_MATCHING.md** - Implementation of UDP length-based ICMP packet matching

### Per-Packet Options Implementation
- **FIX_SUMMARY.md** - Summary of UDP options forwarding fix through the entire WebRTC stack
- **ICE_SEND_OPTIONS_FIX.md** - Implementation of send_with_options in the ICE layer
- **DTLS_FORWARDING_FIX.md** - Adding send_with_options forwarding to DTLSConn

### ICMP and Error Handling
- **ICMP_MESSAGE_FIX.md** - Fix for ICMP error message delivery to clients
- **IPV6_TRACKING_FIX.md** - Implementation of IPv6 packet tracking for ICMPv6 support

### Feature Implementations
- **TESTPROBE_IMPLEMENTATION.md** - Separate testprobe data channel for traceroute packets

## Current Documentation

For current, active documentation, see the main [docs](../) directory:
- Setup and configuration guides
- Authentication system documentation
- Feature documentation (UDP packet options, HTTPS setup, etc.)
- API documentation

## Note

These historical documents are not actively maintained but are kept for reference. The information may no longer reflect the current state of the codebase but provides valuable context for understanding past decisions and implementations.
