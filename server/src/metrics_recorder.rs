//! Metrics recorder service for persisting survey probe statistics
//!
//! Records both server-side and client-side network metrics to the SQLite database
//! for later analysis and export.

use crate::database::DbConnection;
use common::DirectionStats;
use rusqlite::params;

/// Service for recording survey metrics to the database
pub struct MetricsRecorder {
    db: DbConnection,
}

impl MetricsRecorder {
    /// Create a new MetricsRecorder with the given database connection
    pub fn new(db: DbConnection) -> Self {
        Self { db }
    }

    /// Record server-side probe stats (both c2s and s2c directions)
    ///
    /// # Arguments
    /// * `session_id` - Survey session identifier
    /// * `conn_id` - Connection identifier for multi-path testing
    /// * `timestamp_ms` - Timestamp in milliseconds
    /// * `c2s_stats` - Client-to-server direction statistics
    /// * `s2c_stats` - Server-to-client direction statistics
    pub async fn record_probe_stats(
        &self,
        session_id: &str,
        conn_id: &str,
        timestamp_ms: u64,
        c2s_stats: &DirectionStats,
        s2c_stats: &DirectionStats,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
                session_id,
                timestamp_ms,
                "server",
                conn_id,
                "c2s",
                c2s_stats.delay_deviation_ms[0],
                c2s_stats.delay_deviation_ms[1],
                c2s_stats.delay_deviation_ms[2],
                c2s_stats.delay_deviation_ms[3],
                c2s_stats.jitter_ms[0],
                c2s_stats.jitter_ms[1],
                c2s_stats.jitter_ms[2],
                c2s_stats.jitter_ms[3],
                c2s_stats.rtt_ms[0],
                c2s_stats.rtt_ms[1],
                c2s_stats.rtt_ms[2],
                c2s_stats.rtt_ms[3],
                c2s_stats.loss_rate,
                c2s_stats.reorder_rate,
                c2s_stats.probe_count,
                c2s_stats.baseline_delay_ms,
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
                session_id,
                timestamp_ms,
                "server",
                conn_id,
                "s2c",
                s2c_stats.delay_deviation_ms[0],
                s2c_stats.delay_deviation_ms[1],
                s2c_stats.delay_deviation_ms[2],
                s2c_stats.delay_deviation_ms[3],
                s2c_stats.jitter_ms[0],
                s2c_stats.jitter_ms[1],
                s2c_stats.jitter_ms[2],
                s2c_stats.jitter_ms[3],
                s2c_stats.rtt_ms[0],
                s2c_stats.rtt_ms[1],
                s2c_stats.rtt_ms[2],
                s2c_stats.rtt_ms[3],
                s2c_stats.loss_rate,
                s2c_stats.reorder_rate,
                s2c_stats.probe_count,
                s2c_stats.baseline_delay_ms,
                now_ms
            ],
        )?;

        Ok(())
    }

    /// Record client-side metrics (s2c direction only, as seen by the client)
    ///
    /// # Arguments
    /// * `session_id` - Survey session identifier
    /// * `conn_id` - Connection identifier for multi-path testing
    /// * `timestamp_ms` - Timestamp in milliseconds
    /// * `s2c_stats` - Server-to-client statistics as measured by the client
    pub async fn record_client_metrics(
        &self,
        session_id: &str,
        conn_id: &str,
        timestamp_ms: u64,
        s2c_stats: &DirectionStats,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
                session_id,
                timestamp_ms,
                "client",
                conn_id,
                "s2c",
                s2c_stats.delay_deviation_ms[0],
                s2c_stats.delay_deviation_ms[1],
                s2c_stats.delay_deviation_ms[2],
                s2c_stats.delay_deviation_ms[3],
                s2c_stats.jitter_ms[0],
                s2c_stats.jitter_ms[1],
                s2c_stats.jitter_ms[2],
                s2c_stats.jitter_ms[3],
                s2c_stats.rtt_ms[0],
                s2c_stats.rtt_ms[1],
                s2c_stats.rtt_ms[2],
                s2c_stats.rtt_ms[3],
                s2c_stats.loss_rate,
                s2c_stats.reorder_rate,
                s2c_stats.probe_count,
                s2c_stats.baseline_delay_ms,
                now_ms
            ],
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::init_database;
    use tempfile::NamedTempFile;

    fn create_test_stats() -> DirectionStats {
        DirectionStats {
            delay_deviation_ms: [1.0, 2.0, 0.5, 3.0],
            rtt_ms: [10.0, 15.0, 8.0, 20.0],
            jitter_ms: [0.5, 1.0, 0.2, 1.5],
            loss_rate: 0.01,
            reorder_rate: 0.0,
            probe_count: 100,
            baseline_delay_ms: 5.0,
        }
    }

    #[tokio::test]
    async fn test_record_probe_stats() {
        let temp_file = NamedTempFile::new().unwrap();
        let db = init_database(temp_file.path()).unwrap();

        // Create a test session first
        {
            let conn = db.lock().await;
            let now_ms = chrono::Utc::now().timestamp_millis() as u64;
            conn.execute(
                "INSERT INTO survey_sessions (session_id, magic_key, start_time, last_update_time, created_at)
                 VALUES (?, ?, ?, ?, ?)",
                params!["test-session", "TEST-001", now_ms, now_ms, now_ms],
            )
            .unwrap();
        }

        let recorder = MetricsRecorder::new(db.clone());
        let c2s_stats = create_test_stats();
        let s2c_stats = create_test_stats();

        recorder
            .record_probe_stats("test-session", "conn-1", 1234567890, &c2s_stats, &s2c_stats)
            .await
            .unwrap();

        // Verify 2 rows were inserted (c2s and s2c)
        let conn = db.lock().await;
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM survey_metrics WHERE session_id = ?",
                params!["test-session"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_record_client_metrics() {
        let temp_file = NamedTempFile::new().unwrap();
        let db = init_database(temp_file.path()).unwrap();

        // Create a test session first
        {
            let conn = db.lock().await;
            let now_ms = chrono::Utc::now().timestamp_millis() as u64;
            conn.execute(
                "INSERT INTO survey_sessions (session_id, magic_key, start_time, last_update_time, created_at)
                 VALUES (?, ?, ?, ?, ?)",
                params!["test-session", "TEST-001", now_ms, now_ms, now_ms],
            )
            .unwrap();
        }

        let recorder = MetricsRecorder::new(db.clone());
        let s2c_stats = create_test_stats();

        recorder
            .record_client_metrics("test-session", "conn-1", 1234567890, &s2c_stats)
            .await
            .unwrap();

        // Verify 1 row was inserted
        let conn = db.lock().await;
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM survey_metrics WHERE session_id = ? AND source = 'client'",
                params!["test-session"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }
}
