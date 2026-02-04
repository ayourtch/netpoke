# Issue 005: Test Metadata Not Populated in Recordings

## Summary
The RecordingMetadata struct includes `test_metadata` field to store network test data with the recording, but this field is never populated with actual test information.

## Location
- File: `client/src/recorder/types.rs` - Field definition (line 53)
- File: `client/src/recorder/state.rs` - Where metadata is created (lines 273-288)

## Current Behavior
In `client/src/recorder/types.rs`:
```rust
pub struct RecordingMetadata {
    // ... other fields ...
    #[serde(default)]
    pub test_metadata: Option<serde_json::Value>,
}
```

In `client/src/recorder/state.rs` `stop_recording()`:
```rust
let metadata = RecordingMetadata {
    // ... other fields ...
    test_metadata: None,  // Always None!
};
```

## Expected Behavior
Per the design document `docs/plans/2026-02-04-camera-recording-integration-design.md`:
```javascript
testMetadata: {
    ipv4Active: true,
    ipv6Active: false,
    testStartTime: "2026-02-04T14:23:30.000Z",
    testEndTime: "2026-02-04T14:25:13.500Z"
}
```

The recording should capture the network test state including:
- Whether IPv4/IPv6 connections were active
- Test start and end times
- Optionally: summary statistics from the test

## Impact
- **Priority: Medium**
- Recording files won't contain any information about the network test that was running
- Users cannot correlate recordings with specific test results
- The whole point of integrating recording with network testing is partially defeated

## Suggested Implementation
1. Create a function to gather current test metadata from the measurement state:
   ```rust
   fn get_current_test_metadata() -> Option<serde_json::Value> {
       // Access the global measurement state to determine:
       // - Are IPv4/IPv6 tests active?
       // - When did the test start?
       // - What are the current metrics?
       
       let json = serde_json::json!({
           "ipv4Active": is_ipv4_active(),
           "ipv6Active": is_ipv6_active(),
           "testStartTime": get_test_start_time(),
           "testEndTime": current_timestamp_utc(),
           // Could also include summary metrics
       });
       
       Some(json)
   }
   ```

2. In `stop_recording()`, call this function:
   ```rust
   let metadata = RecordingMetadata {
       // ... other fields ...
       test_metadata: get_current_test_metadata(),
   };
   ```

3. This requires exposing some state from the main measurement code to the recorder module. Consider:
   - Adding a shared state accessor
   - Or passing test state when starting/stopping recording
   - Or using a callback mechanism

## Resolution
Fixed in commit 9ab2ea2 (2026-02-04).

**Changes made:**
1. Added `TEST_START_TIME` thread_local variable to track when testing starts in `client/src/lib.rs`
2. Created `get_test_metadata()` exported function that:
   - Checks if testing is active
   - Returns test metadata with IPv4/IPv6 status, test start/end times
   - Returns NULL if no test is active
3. Modified `set_testing_active()` to record start time when testing begins
4. Updated `stop_recording()` in `client/src/recorder/state.rs` to call `get_test_metadata()` and populate the `test_metadata` field

The recording metadata now includes actual test state information that can be correlated with network measurements.

---
*Created: 2026-02-04*
*Resolved: 2026-02-04*
