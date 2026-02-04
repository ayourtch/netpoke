# Fix Issues Prompt

## Overview
Review and fix issues in `docs/issues/open/`. Follow the workflow in `docs/issues/README.md`.

## Core Workflow
1. Read `docs/issues/README.md` for the issue tracking process
2. Verify each issue still exists before fixing
3. Make minimal changes to resolve issues
4. Move resolved issues to `docs/issues/resolved/` with `git mv`
5. Add "Resolution" section documenting changes and verification

## Critical Rules

### DO NOT Modify Reference Code
**NEVER change files in `tmp/camera-standalone-for-cross-check/`** - these are reference implementations for cross-checking. They may contain intentional bugs or outdated patterns for comparison purposes.

### Build System
- Client: Rust WASM built with `wasm-pack`
- Build: `cd client && wasm-pack build --target web --out-dir ../server/static/pkg`
- Setup: `rustup target add wasm32-unknown-unknown && cargo install wasm-pack`

### WASM Exports
- Functions marked `#[wasm_bindgen]` are automatically exported
- Verify exports: `grep "export function" server/static/pkg/netpoke_client.js`
- HTML imports must match Rust function signatures exactly

## Universal Patterns

1. **Verify Before Changing**: Many issues are already fixed. Check current code first.

2. **Signatures Must Match**: JavaScript calls to WASM functions must have exact parameter matches. Verify with a build.

3. **Build Early, Build Often**: A single WASM build catches multiple signature mismatches and export issues.

4. **Deprecate, Don't Delete**: For outdated code, add warnings instead of removing. Preserves history.

5. **Platform Quirks Matter**: iOS Safari has strict timing requirements (e.g., event listeners must be registered synchronously after permission grants).

6. **Group Related Changes**: Issues touching the same system can often be solved together efficiently.

## Quick Commands

```bash
# Build WASM
cd client && wasm-pack build --target web --out-dir ../server/static/pkg

# Move resolved issue
git mv docs/issues/open/NNN-description.md docs/issues/resolved/

# Verify function exports
grep "export function" server/static/pkg/netpoke_client.js
```

## Self-Improvement

After each session, review this prompt. Instead of adding more content:

1. **Generalize**: Replace specific examples with broader patterns
2. **Consolidate**: Merge similar lessons into single principles
3. **Delete**: Remove outdated or overly specific advice
4. **Simplify**: Make instructions clearer and more concise

**Goal**: Keep this file short and maximally useful. Wisdom comes from finding universal truths, not accumulating details.
