# Issue 037: Implement MetricsRecorder Service

## Summary
Create a service that records probe statistics to the SQLite database for both server-side and client-side metrics.

## Location
- File: `server/src/metrics_recorder.rs` (new file)
- File: `server/src/main.rs` (add mod declaration)

## Current Behavior
Probe statistics are calculated but not persisted to any database.

## Expected Behavior
A `MetricsRecorder` service that:
1. Accepts `DirectionStats` from the existing metrics calculation code
2. Stores metrics in the `survey_metrics` table with session, connection, and direction info
3. Records both server-side (c2s and s2c) and client-side (s2c) metrics

## Impact
Enables persistence of survey metrics for later analysis and export.

## Suggested Implementation

### Step 1: Create metrics recorder module

Create `server/src/metrics_recorder.rs`:

```rust
use crate::database::DbConnection;
use common::protocol::DirectionStats;
use rusqlite::params;

pub struct MetricsRecorder {
    db: DbConnection,
}

impl MetricsRecorder {
    pub fn new(db: DbConnection) -> Self {
        Self { db }
    }

    /// Record server-side probe stats (both directions)
    pub async fn record_probe_stats(
        &self,
        session_id: &str,
        conn_id: &str,
        timestamp_ms: u64,
        c2s_stats: &DirectionStats,
        s2c_stats: &DirectionStats,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db = self.db.lock().await;
        let now_ms = chrono::Utc::now().timestamp_millis() as u64;

        // Insert c2s metrics
        db.execute(
            "INSERT INTO survey_metrics (
                session_id, timestamp_ms, source, conn_id, direction,
                delay_p50_ms, delay_p99_ms, delay_min_ms, delay_max_ms,
                jitter_p50_ms, jitter_p99_ms, jitter_min_ms, jitter_max_ms,
                rtt_p50_ms, rtt_p99_ms, rtt_min_ms, rtt_max_ms,
                loss_rate, reorder_rate, probe_count, baseline_delay_ms,
                created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                session_id, timestamp_ms, "server", conn_id, "c2s",
                c2s_stats.delay_deviation_ms[0], c2s_stats.delay_deviation_ms[1],
                c2s_stats.delay_deviation_ms[2], c2s_stats.delay_deviation_ms[3],
                c2s_stats.jitter_ms[0], c2s_stats.jitter_ms[1],
                c2s_stats.jitter_ms[2], c2s_stats.jitter_ms[3],
                c2s_stats.rtt_ms[0], c2s_stats.rtt_ms[1],
                c2s_stats.rtt_ms[2], c2s_stats.rtt_ms[3],
                c2s_stats.loss_rate, c2s_stats.reorder_rate,
                c2s_stats.probe_count, c2s_stats.baseline_delay_ms,
                now_ms
            ],
        )?;

        // Insert s2c metrics
        db.execute(
            "INSERT INTO survey_metrics (
                session_id, timestamp_ms, source, conn_id, direction,
                delay_p50_ms, delay_p99_ms, delay_min_ms, delay_max_ms,
                jitter_p50_ms, jitter_p99_ms, jitter_min_ms, jitter_max_ms,
                rtt_p50_ms, rtt_p99_ms, rtt_min_ms, rtt_max_ms,
                loss_rate, reorder_rate, probe_count, baseline_delay_ms,
                created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                session_id, timestamp_ms, "server", conn_id, "s2c",
                s2c_stats.delay_deviation_ms[0], s2c_stats.delay_deviation_ms[1],
                s2c_stats.delay_deviation_ms[2], s2c_stats.delay_deviation_ms[3],
                s2c_stats.jitter_ms[0], s2c_stats.jitter_ms[1],
                s2c_stats.jitter_ms[2], s2c_stats.jitter_ms[3],
                s2c_stats.rtt_ms[0], s2c_stats.rtt_ms[1],
                s2c_stats.rtt_ms[2], s2c_stats.rtt_ms[3],
                s2c_stats.loss_rate, s2c_stats.reorder_rate,
                s2c_stats.probe_count, s2c_stats.baseline_delay_ms,
                now_ms
            ],
        )?;

        Ok(())
    }

    /// Record client-side metrics (s2c only)
    pub async fn record_client_metrics(
        &self,
        session_id: &str,
        conn_id: &str,
        timestamp_ms: u64,
        s2c_stats: &DirectionStats,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db = self.db.lock().await;
        let now_ms = chrono::Utc::now().timestamp_millis() as u64;

        db.execute(
            "INSERT INTO survey_metrics (
                session_id, timestamp_ms, source, conn_id, direction,
                delay_p50_ms, delay_p99_ms, delay_min_ms, delay_max_ms,
                jitter_p50_ms, jitter_p99_ms, jitter_min_ms, jitter_max_ms,
                rtt_p50_ms, rtt_p99_ms, rtt_min_ms, rtt_max_ms,
                loss_rate, reorder_rate, probe_count, baseline_delay_ms,
                created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                session_id, timestamp_ms, "client", conn_id, "s2c",
                s2c_stats.delay_deviation_ms[0], s2c_stats.delay_deviation_ms[1],
                s2c_stats.delay_deviation_ms[2], s2c_stats.delay_deviation_ms[3],
                s2c_stats.jitter_ms[0], s2c_stats.jitter_ms[1],
                s2c_stats.jitter_ms[2], s2c_stats.jitter_ms[3],
                s2c_stats.rtt_ms[0], s2c_stats.rtt_ms[1],
                s2c_stats.rtt_ms[2], s2c_stats.rtt_ms[3],
                s2c_stats.loss_rate, s2c_stats.reorder_rate,
                s2c_stats.probe_count, s2c_stats.baseline_delay_ms,
                now_ms
            ],
        )?;

        Ok(())
    }
}
```

### Step 2: Add unit tests

Add tests to verify metrics are recorded correctly:
- Test `record_probe_stats()` inserts 2 rows (c2s and s2c)
- Test `record_client_metrics()` inserts 1 row

### Step 3: Add module declaration

Add to `server/src/main.rs`:
```rust
mod metrics_recorder;
```

## Testing

```bash
cd server
cargo test metrics_recorder::tests
```

## Dependencies
- Issue 033: Add database dependencies
- Issue 034: Create database schema migration
- Issue 035: Implement database module

## Reference
See `docs/plans/2026-02-05-survey-upload-implementation.md` - Task 5 for full details.
See `docs/plans/2026-02-05-survey-upload-feature-design.md` - Metrics Collection & Storage section.

---
*Created: 2026-02-05*
