# Issue 066: Per-Magic-Key Maximum Measuring Time Limit

## Summary

There is no per-magic-key limit on how long a measurement test can run. The server always returns a fixed `DEFAULT_MEASURING_TIME_MS = 10_000_000` (~2.7 hours) regardless of which magic key is used. The DEMO magic key should default to 120 seconds.

## Location

- `server/src/data_channels.rs`: `GetMeasuringTime` handler, `DEFAULT_MEASURING_TIME_MS` constant
- `auth/src/config.rs`: `MagicKeyConfig` struct
- `server_config.toml.example`: Magic key configuration section

## Current Behavior

1. The `GetMeasuringTime` handler always returns `DEFAULT_MEASURING_TIME_MS = 10_000_000` (~2.7 hours)
2. There is no way to configure per-magic-key measuring time limits
3. There is no server-side enforcement of measuring time limits
4. The DEMO magic key has no special duration treatment

## Expected Behavior

1. Each magic key should have an optional maximum measuring time
2. A global `max_measuring_time_seconds` default should be configurable
3. Per-key overrides should be possible (e.g., DEMO key → 120 seconds)
4. The server should return the appropriate limit in `MeasuringTimeResponse`
5. The server should enforce the limit by auto-stopping probe streams when the duration is exceeded

## Impact

- No resource protection for shared/demo deployments
- Users with the DEMO key can run unlimited duration tests
- Server resources can be consumed indefinitely

## Root Cause Analysis

The measuring time feature was implemented with a hardcoded constant and no connection to the authentication/magic key system.

## Suggested Implementation

1. Add `max_measuring_time_seconds` field to `MagicKeyConfig` with a sensible default (e.g., 3600)
2. Add `magic_key_limits` HashMap to `MagicKeyConfig` for per-key overrides
3. Store the magic key config on `AppState` and `ClientSession` so it's accessible in data channel handlers
4. Update `GetMeasuringTime` handler to look up the session's magic key and return the appropriate limit
5. Add server-side enforcement: track `probe_started_at` and auto-stop when duration exceeded
6. Default the DEMO key to 120 seconds in config defaults
