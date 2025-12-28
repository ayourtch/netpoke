# Analyze Network Flow Implementation

## Summary
Successfully merged the "Analyze Path" functionality into a unified "Analyze Network" button that performs network analysis in two sequential phases:

1. **Phase 1 (Traceroute)**: Establishes WebRTC connections and performs traceroute on all connections
2. **Phase 2 (Measurements)**: Continues with delay/reordering/loss measurements on the same connections

## Changes Made

### 1. Rust Client (`client/src/lib.rs`)

#### New Functions Added:
- `analyze_network()` - Entry point for single connection (delegates to analyze_network_with_count(1))
- `analyze_network_with_count(conn_count: u8)` - Main implementation that:
  - Establishes IPv4 and IPv6 WebRTC connections with traceroute mode
  - Waits 30 seconds for traceroute data collection (PATH_ANALYSIS_TIMEOUT_MS)
  - Starts measurement calculation loops (every 100ms)
  - Starts UI update loops (every 500ms)
  - Keeps connections alive for ongoing measurements

#### Key Implementation Details:
- Reuses existing WebRTC connection infrastructure
- Connections are created with MODE_TRACEROUTE to enable server-side traceroute functionality
- After traceroute phase, the same connections transition to measurement mode
- Uses `std::mem::forget()` to keep connections alive indefinitely for measurements
- Maintains wake lock throughout both phases to prevent device sleep

### 2. HTML UI (`server/static/nettest.html`)

#### Button Changes:
- **Removed**: Two separate buttons
  - "Start Measurement" button
  - "Analyze Path" button
- **Added**: Single unified button
  - "Analyze Network" button

#### JavaScript Changes:
- Updated imports to include `analyze_network` and `analyze_network_with_count` from WASM module
- Replaced `startMeasurement()` and `analyzePath()` with new `analyzeNetwork()` function
- Updated status messages to show:
  - Phase 1: "Analyzing network paths (traceroute) with X connection(s)..."
  - Phase 2: "Running network measurements with X connection(s)..."
- Simplified button state management (only one button to enable/disable)
- Maintains traceroute visualization clearing and server messages reset

### 3. WASM Build
- Successfully built WASM client with new functions exported
- Generated TypeScript definitions include:
  - `analyze_network(): Promise<void>`
  - `analyze_network_with_count(conn_count: number): Promise<void>`

## User Experience Flow

1. User clicks "Analyze Network" button
2. Button is disabled during operation
3. Status shows "Phase 1: Analyzing network paths (traceroute)..."
4. WebRTC connections establish for both IPv4 and IPv6
5. Traceroute data is collected for 30 seconds
6. Traceroute visualization updates in real-time
7. After 30 seconds, status updates to "Phase 2: Running network measurements..."
8. Network metrics (delay, jitter, loss, reordering) begin displaying
9. Measurements continue indefinitely until page refresh or user leaves

## Benefits

1. **Simplified UX**: Single button instead of two, clearer workflow
2. **Complete Analysis**: Users automatically get both path analysis and performance measurements
3. **Efficient**: Reuses connections between phases, no need to establish connections twice
4. **Informative**: Users see the progression through phases
5. **Backward Compatible**: Old functions (`analyze_path`, `start_measurement`) remain available if needed

## Testing Recommendations

To test the implementation:

1. Build the WASM client: `cd client && ./build.sh`
2. Start the server: `cargo run --release --bin wifi-verify-server`
3. Open browser to `http://localhost:3000/static/nettest.html`
4. Click "Analyze Network" button
5. Verify:
   - Traceroute visualization appears and updates
   - After ~30 seconds, metrics tables start updating
   - Network measurements continue running
   - Wake lock activates and shows in status

## Technical Notes

- Traceroute timeout: 30 seconds (PATH_ANALYSIS_TIMEOUT_MS = 30000)
- Metric calculation interval: 100ms
- UI update interval: 500ms
- Supports 1-16 connections per address family (ECMP testing)
- Connections persist via `std::mem::forget()` - intentional memory leak for long-running measurements
