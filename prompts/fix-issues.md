# Fix Issues Prompt

## Overview
This prompt directs you to review and fix issues tracked in `docs/issues/open/`. The issues follow a structured format and workflow documented in `docs/issues/README.md`.

## Instructions
1. Read `docs/issues/README.md` to familiarize yourself with the issue tracking process
2. Fix as many of the issues in `docs/issues/open/` as possible
3. If you spot additional discrepancies when working, open new issues according to the process
4. Move resolved issues from `docs/issues/open/` to `docs/issues/resolved/` using `git mv`
5. Add a "Resolution" section to each resolved issue documenting what was done

## Important Context

### Example code when dealing with camera+sensors+screenshare integration
- There is already tested and WORKING code before integration - do not be afraid to check!
- The code is located in tmp/camera-standalone-for-cross-check/* 

### Build System
- The client is a Rust WASM module built with `wasm-pack`
- Install wasm-pack: `cargo install wasm-pack`
- Build client: `cd client && wasm-pack build --target web --out-dir ../server/static/pkg`
- First build requires: `rustup target add wasm32-unknown-unknown`
- Build time: Initial build ~3 minutes, subsequent builds ~10 seconds

### WASM Exports
- Functions marked with `#[wasm_bindgen]` are automatically exported even from private modules
- No need to re-export from lib.rs - wasm_bindgen handles this
- Verify exports by checking the generated `server/static/pkg/netpoke_client.js` file
- The HTML imports functions from the WASM module at runtime

### Common Issue Patterns Found

1. **Already Resolved Issues**
   - Many issues may already be fixed in the current codebase
   - Always verify the issue still exists before making changes
   - Check the actual code, not just the issue description
   - Example: Issues 002, 018, 019 were already resolved

2. **Partial Implementations**
   - Some features may be mostly complete with only small gaps
   - Example: Issue 003 had 4/5 sensor callbacks already implemented
   - Verify what exists first, then add only what's missing

3. **Configuration vs Code Issues**
   - Some "issues" are actually configuration or expected behavior
   - Example: Issue 020 (missing source maps) is harmless and expected
   - Consider if the "issue" actually needs fixing or just documentation

### Code Organization
- Client code: `client/src/`
  - `lib.rs` - Main exports and global state
  - `recorder/` - Recording subsystem
    - `ui.rs` - UI event handlers and state
    - `state.rs` - Recording state machine
    - `canvas_renderer.rs` - Canvas compositing
    - `sensors.rs` - Sensor data management
- Server code: `server/src/`
- Static files: `server/static/`
  - `nettest.html` - Main UI
  - `pkg/` - Generated WASM artifacts (gitignored)

### Testing Approach
1. Build the client WASM module to verify code compiles
2. Check generated exports in `pkg/netpoke_client.js`
3. For UI changes, consider manual testing with the server running
4. Document verification steps in issue resolution

### Issue Resolution Process
1. Verify the issue still exists in current code
2. Make minimal changes to fix the issue
3. Build and verify the fix works
4. Move issue file from `open/` to `resolved/` with `git mv`
5. Add "Resolution" section with:
   - Status (Resolved / Already Resolved / Documented)
   - Changes made (or why no changes needed)
   - Files modified
   - Verification steps
6. Update the *Resolved* date at the bottom

### Lessons Learned from This Session

1. **Check Existing Code First**: 3 out of 7 issues were already resolved. Always verify before coding.

2. **wasm_bindgen Exports Work From Private Modules**: Functions marked with `#[wasm_bindgen]` are automatically exported even if the module is private. No need for re-exports in lib.rs unless there's a name collision.

3. **HTML and Rust Must Agree**: The HTML imports specific function names from the WASM module. Both sides must match. Check `nettest.html` for `import { ... } from module` statements.

4. **Build to Verify**: Don't assume changes work. Build the WASM module and check the generated JS wrapper to verify exports.

5. **Some Issues Aren't Code Issues**: Low-priority issues may be expected behavior (like missing source maps). Document rather than "fix" these.

6. **Integration is Key**: Changes often span multiple files:
   - UI changes need HTML + Rust UI code + state management
   - Example: Metrics display needed HTML elements + UI update functions + render loop integration

7. **Issue Descriptions May Be Outdated**: The codebase evolves. Issue descriptions may reference code that no longer exists or has been refactored.

### Lessons Learned - Session 2 (Issues 021-025)

8. **Verify Features Before Assuming Missing**: Issue 025 (chart compositing) was already fully implemented. Check the actual code in detail before starting work.

9. **iOS Safari Has Strict Timing Requirements**: Event listeners for motion/orientation sensors MUST be registered in the same synchronous task as permission grant on iOS. Any `await` in between breaks the context and sensors won't work.

10. **Combine Related Issues**: Issues 021 (magnetometer export) and 024 (sensor permission timing) were naturally solved together since both involve sensor listener setup. Look for natural groupings.

11. **Function Signature Mismatches Are Critical**: Issue 022 (wrong parameter count) would have caused runtime errors. Always verify WASM function signatures match JavaScript calls.

12. **Deprecation Over Deletion**: For legacy files (Issue 023), adding a deprecation banner is better than refactoring or deleting. Preserves history while guiding users to the correct path.

13. **Standalone Test Files Need Maintenance**: Reference implementations in `tmp/camera-standalone-for-cross-check/` should be kept in sync with main code, especially function signatures.

14. **WASM Build Confirms Multiple Issues**: A single WASM build verifies multiple changes at once. Build early and often to catch signature mismatches and export issues.

### Quick Reference Commands

```bash
# Install prerequisites
rustup target add wasm32-unknown-unknown
cargo install wasm-pack

# Build client WASM
cd client
wasm-pack build --target web --out-dir ../server/static/pkg

# Verify exports
grep "export function" ../server/static/pkg/netpoke_client.js

# Move resolved issue
git mv docs/issues/open/NNN-description.md docs/issues/resolved/

# Check for duplicate function definitions
grep -r "fn function_name" client/src/
```

### Time Estimates
- Reading and understanding all open issues: ~15 minutes
- Verifying if issues still exist: ~10 minutes per issue
- Implementing small fixes: ~5-10 minutes per fix
- Building and verifying: ~3-10 minutes per build
- Documenting resolutions: ~5 minutes per issue
- Total for 7 issues: ~2-3 hours

## Self-Improvement Process

After completing work on issues, always update this prompt with new insights:

### What to Add

1. **New Lessons Learned**: Document any new patterns, gotchas, or best practices discovered during this session. Number them sequentially to build on previous sessions.

2. **Updated Context**: If issue patterns change or new subsystems are added:
   - Update the "Code Organization" section
   - Add new directories or important files
   - Document new architectural patterns

3. **Process Improvements**: If you discover a better way to:
   - Verify issues
   - Test changes
   - Document resolutions
   - Build or deploy
   
   Add it to the appropriate section.

4. **New Command Patterns**: Add any new commands to the "Quick Reference Commands" section.

5. **Revised Time Estimates**: If actual time differs significantly from estimates, update the "Time Estimates" section.

### How to Update This Prompt

1. **After Each Session**: Before finalizing your work:
   - Review what you learned
   - Add new lessons learned with sequential numbering (e.g., "Session 3: Issues 026-030")
   - Update any sections that are now outdated
   - Add any new common patterns you discovered

2. **Keep It Actionable**: Focus on concrete, actionable advice. Avoid:
   - Vague generalizations
   - Obvious statements
   - Information that's already documented elsewhere

3. **Preserve History**: Don't delete old lessons unless they're wrong. Instead:
   - Mark as outdated if needed: "~~Old approach~~ (superseded by...)"
   - Add clarifications: "Note: This changed in..."
   - Keep the evolution visible

4. **Format Consistently**:
   - Use "Session N" headings for new lesson groups
   - Number lessons sequentially across all sessions
   - Use code blocks for examples
   - Include file paths and line numbers where relevant

### Example Session Update

```markdown
### Lessons Learned - Session 3 (Issues 026-030)

15. **New Pattern Discovered**: [Description]

16. **Common Pitfall**: [What to watch out for]

17. **Better Approach**: [Improved method for X]
```

### Commit This Update

After updating this prompt:
```bash
git add prompts/fix-issues.md
git commit -m "Update fix-issues prompt with Session N lessons learned"
```

Include the prompt update in your final PR for the session.

