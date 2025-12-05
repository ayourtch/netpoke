use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientMetrics {
    // Throughput in bytes/sec for [1s, 10s, 60s] windows
    pub c2s_throughput: [f64; 3],
    pub s2c_throughput: [f64; 3],

    // Delay in milliseconds
    pub c2s_delay_avg: [f64; 3],
    pub s2c_delay_avg: [f64; 3],

    // Jitter (std dev of delay) in milliseconds
    pub c2s_jitter: [f64; 3],
    pub s2c_jitter: [f64; 3],

    // Loss rate as percentage
    pub c2s_loss_rate: [f64; 3],
    pub s2c_loss_rate: [f64; 3],

    // Reordering rate as percentage
    pub c2s_reorder_rate: [f64; 3],
    pub s2c_reorder_rate: [f64; 3],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_metrics_default() {
        let metrics = ClientMetrics::default();
        assert_eq!(metrics.c2s_throughput, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_client_metrics_serialization() {
        let mut metrics = ClientMetrics::default();
        metrics.c2s_throughput = [1000.0, 900.0, 850.0];

        let json = serde_json::to_string(&metrics).unwrap();
        let deserialized: ClientMetrics = serde_json::from_str(&json).unwrap();

        assert_eq!(metrics.c2s_throughput, deserialized.c2s_throughput);
    }
}
