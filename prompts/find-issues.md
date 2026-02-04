# Find Issues Prompt

## Overview
Compare reference implementations against integrated code to find and document discrepancies. Follow the workflow in `docs/issues/README.md`.

## Core Task
Compare `tmp/camera-standalone-for-cross-check/` (working reference) with `client/src/recorder/` and `server/static/` (integrated). Document issues found.

## Comparison Points

**Reference**: `tmp/camera-standalone-for-cross-check/` (working code)
**Integrated**: `client/src/recorder/` + `server/static/` (target code)

Key files: `lib.rs` (WASM exports), HTML files (imports/setup), sensor/UI modules

Context: `docs/plans/` and `docs/issues/session-summary-*.md`

## Universal Patterns to Check

1. **Function Signatures**: WASM exports must match JavaScript calls exactly (parameter count, types, order)

2. **Missing Integrations**: Functions exported but not imported, or event listeners not registered

3. **Module Paths**: Integrated uses `crate::recorder::*` namespace vs standalone `crate::*`

4. **Platform Requirements**: iOS Safari needs event listeners added synchronously with permission grants (no `await` between)

5. **State Management**: Compare initialization patterns (eager vs lazy) and check for race conditions

6. **Feature Completeness**: New features in integrated code may exist but not be wired up

## Issue Documentation

Follow `docs/issues/README.md` for complete process. Key principles:

- **Specific**: Include file paths, line numbers, code snippets
- **Explain Why**: Not just what's wrong, but why it matters
- **Actionable**: Provide clear implementation steps
- **Context**: Show both incorrect and correct versions

## Quick Commands

```bash
# Find next issue number
ls docs/issues/open/ docs/issues/resolved/ | grep -oE '^[0-9]+' | sort -n | tail -1

# Create new issue
# Use format: NNN-short-description.md in docs/issues/open/

# Verify WASM exports match HTML imports
grep "export function" server/static/pkg/netpoke_client.js
grep "const {" server/static/nettest.html
```

## Testing Strategy

1. Build and verify compilation first
2. Test on iOS Safari for platform-specific issues (sensor permissions)
3. Check browser console for JavaScript errors
4. Verify sensor data in recordings (download motion data JSON)

## Self-Improvement

After each session, review this prompt. Instead of adding more content:

1. **Generalize**: Replace specific examples with broader patterns
2. **Consolidate**: Merge similar lessons into single principles
3. **Delete**: Remove outdated or overly specific advice
4. **Simplify**: Make instructions clearer and more concise

**Goal**: Keep this file short and maximally useful. Wisdom comes from finding universal truths, not accumulating details.
