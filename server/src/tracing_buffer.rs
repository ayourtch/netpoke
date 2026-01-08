/// Circular buffer for tracing/logging with microsecond timestamps
///
/// This module provides a thread-safe circular buffer that stores log entries
/// with microsecond precision timestamps. It can be used as a custom tracing layer.
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::SystemTime;
use tracing::Level;
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::{Layer, Registry};

/// A single log entry with microsecond timestamp
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp_us: u64,
    pub level: Level,
    pub target: String,
    pub message: String,
}

/// Circular buffer for log entries
pub struct TracingBuffer {
    entries: RwLock<Vec<LogEntry>>,
    max_entries: usize,
    next_index: RwLock<usize>,
    enabled: bool,
}

impl TracingBuffer {
    /// Create a new tracing buffer
    pub fn new(max_entries: usize, enabled: bool) -> Self {
        Self {
            entries: RwLock::new(Vec::with_capacity(max_entries)),
            max_entries,
            next_index: RwLock::new(0),
            enabled,
        }
    }

    /// Add a log entry to the buffer
    pub fn add_entry(&self, level: Level, target: String, message: String) {
        if !self.enabled {
            return;
        }

        // Get current time in microseconds since UNIX epoch
        let timestamp_us = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        let entry = LogEntry {
            timestamp_us,
            level,
            target,
            message,
        };

        let mut entries = self.entries.write();
        let mut next_idx = self.next_index.write();

        if entries.len() < self.max_entries {
            // Buffer not yet full, just push
            entries.push(entry);
        } else {
            // Buffer full, overwrite oldest entry (circular)
            entries[*next_idx] = entry;
        }

        *next_idx = (*next_idx + 1) % self.max_entries;
    }

    /// Get all log entries in chronological order
    pub fn get_entries(&self) -> Vec<LogEntry> {
        let entries = self.entries.read();
        let next_idx = *self.next_index.read();

        if entries.len() < self.max_entries {
            // Buffer not yet full, return in order
            entries.clone()
        } else {
            // Buffer is full, need to reorder from oldest to newest
            let mut result = Vec::with_capacity(entries.len());

            // Start from next_idx (oldest) to the end
            for i in next_idx..entries.len() {
                result.push(entries[i].clone());
            }

            // Then from start to next_idx (newest)
            for i in 0..next_idx {
                result.push(entries[i].clone());
            }

            result
        }
    }

    /// Get the number of entries in the buffer
    pub fn len(&self) -> usize {
        self.entries.read().len()
    }

    /// Check if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.entries.read().is_empty()
    }

    /// Clear all entries from the buffer
    pub fn clear(&self) {
        let mut entries = self.entries.write();
        entries.clear();
        *self.next_index.write() = 0;
    }

    /// Check if tracing is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Export entries as formatted text with microsecond timestamps
    ///
    /// Format includes both human-readable datetime (like Wireshark) and Unix epoch
    /// for easy correlation with PCAP files
    pub fn export_as_text(&self) -> String {
        let entries = self.get_entries();
        let mut output = String::new();

        for entry in entries {
            // Convert microseconds to seconds and microseconds parts
            let secs = entry.timestamp_us / 1_000_000;
            let micros = entry.timestamp_us % 1_000_000;

            // Convert to human-readable datetime (like Wireshark shows)
            let datetime = chrono::DateTime::from_timestamp(secs as i64, (micros * 1000) as u32)
                .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());

            // Format: [human_datetime] (epoch.microseconds) LEVEL target - message
            // Example: [2024-01-07 12:34:56.456789 UTC] (1704646496.456789) INFO server::main - Starting server
            output.push_str(&format!(
                "[{}.{:06} UTC] ({}.{:06}) {} {} - {}\n",
                datetime.format("%Y-%m-%d %H:%M:%S"),
                micros,
                secs,
                micros,
                entry.level,
                entry.target,
                entry.message
            ));
        }

        output
    }
}

/// Custom tracing layer that writes to the circular buffer
pub struct TracingBufferLayer {
    buffer: Arc<TracingBuffer>,
}

impl TracingBufferLayer {
    pub fn new(buffer: Arc<TracingBuffer>) -> Self {
        Self { buffer }
    }
}

impl<S> Layer<S> for TracingBufferLayer
where
    S: tracing::Subscriber,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();
        let level = *metadata.level();
        let target = metadata.target().to_string();

        // Extract the message from the event
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        let message = visitor.message;

        self.buffer.add_entry(level, target, message);
    }
}

/// Visitor to extract the message from a tracing event
#[derive(Default)]
struct MessageVisitor {
    message: String,
}

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{:?}", value);
        } else {
            if !self.message.is_empty() {
                self.message.push_str(", ");
            }
            self.message
                .push_str(&format!("{} = {:?}", field.name(), value));
        }
    }
}

/// Service for managing the tracing buffer
#[derive(Clone)]
pub struct TracingService {
    buffer: Arc<TracingBuffer>,
}

impl TracingService {
    pub fn new(max_entries: usize, enabled: bool) -> Self {
        Self {
            buffer: Arc::new(TracingBuffer::new(max_entries, enabled)),
        }
    }

    /// Get a reference to the internal buffer
    pub fn buffer(&self) -> Arc<TracingBuffer> {
        self.buffer.clone()
    }

    /// Export the tracing buffer as text
    pub fn export_as_text(&self) -> String {
        self.buffer.export_as_text()
    }

    /// Get statistics about the buffer
    pub fn stats(&self) -> TracingStats {
        TracingStats {
            entries_in_buffer: self.buffer.len(),
            max_entries: self.buffer.max_entries,
            enabled: self.buffer.is_enabled(),
        }
    }

    /// Clear the buffer
    pub fn clear(&self) {
        self.buffer.clear();
    }

    /// Check if tracing is enabled
    pub fn is_enabled(&self) -> bool {
        self.buffer.is_enabled()
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TracingStats {
    pub entries_in_buffer: usize,
    pub max_entries: usize,
    pub enabled: bool,
}

/// Initialize the tracing subscriber with both console output and buffer
pub fn init_tracing_with_buffer(log_level: tracing::Level, tracing_service: &TracingService) {
    use tracing_subscriber::filter::LevelFilter;

    let buffer_layer = TracingBufferLayer::new(tracing_service.buffer());

    // Convert Level to LevelFilter
    let level_filter = match log_level {
        tracing::Level::TRACE => LevelFilter::TRACE,
        tracing::Level::DEBUG => LevelFilter::DEBUG,
        tracing::Level::INFO => LevelFilter::INFO,
        tracing::Level::WARN => LevelFilter::WARN,
        tracing::Level::ERROR => LevelFilter::ERROR,
    };

    let subscriber = Registry::default()
        .with(tracing_subscriber::fmt::layer().with_filter(level_filter))
        .with(buffer_layer.with_filter(level_filter));

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");
}

/// Initialize the tracing subscriber with an EnvFilter directive for fine-grained control
///
/// The filter_directive follows tracing-subscriber's EnvFilter syntax:
/// - "debug" - set all targets to debug level
/// - "info,server=debug" - default to info, but set server crate to debug
/// - "trace,tokio=info,hyper=info" - trace everything except tokio and hyper
pub fn init_tracing_with_filter(
    filter_directive: &str,
    tracing_service: &TracingService,
) -> Result<(), String> {
    use tracing_subscriber::EnvFilter;

    let buffer_layer = TracingBufferLayer::new(tracing_service.buffer());

    // Parse the filter directive
    let env_filter = EnvFilter::try_new(filter_directive)
        .map_err(|e| format!("Invalid filter directive '{}': {}", filter_directive, e))?;

    // Clone the filter for the buffer layer (EnvFilter doesn't implement Clone,
    // so we need to parse it again)
    let buffer_filter = EnvFilter::try_new(filter_directive)
        .map_err(|e| format!("Invalid filter directive '{}': {}", filter_directive, e))?;

    let subscriber = Registry::default()
        .with(tracing_subscriber::fmt::layer().with_filter(env_filter))
        .with(buffer_layer.with_filter(buffer_filter));

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circular_buffer_basic() {
        let buffer = TracingBuffer::new(3, true);

        buffer.add_entry(Level::INFO, "test".to_string(), "message 1".to_string());
        buffer.add_entry(Level::WARN, "test".to_string(), "message 2".to_string());
        buffer.add_entry(Level::ERROR, "test".to_string(), "message 3".to_string());

        let entries = buffer.get_entries();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].message, "message 1");
        assert_eq!(entries[1].message, "message 2");
        assert_eq!(entries[2].message, "message 3");
    }

    #[test]
    fn test_circular_buffer_overflow() {
        let buffer = TracingBuffer::new(3, true);

        buffer.add_entry(Level::INFO, "test".to_string(), "message 1".to_string());
        buffer.add_entry(Level::INFO, "test".to_string(), "message 2".to_string());
        buffer.add_entry(Level::INFO, "test".to_string(), "message 3".to_string());
        buffer.add_entry(Level::INFO, "test".to_string(), "message 4".to_string());
        buffer.add_entry(Level::INFO, "test".to_string(), "message 5".to_string());

        let entries = buffer.get_entries();
        assert_eq!(entries.len(), 3);
        // Should contain the 3 most recent messages
        assert_eq!(entries[0].message, "message 3");
        assert_eq!(entries[1].message, "message 4");
        assert_eq!(entries[2].message, "message 5");
    }

    #[test]
    fn test_disabled_buffer() {
        let buffer = TracingBuffer::new(3, false);

        buffer.add_entry(Level::INFO, "test".to_string(), "message 1".to_string());
        buffer.add_entry(Level::INFO, "test".to_string(), "message 2".to_string());

        let entries = buffer.get_entries();
        assert_eq!(entries.len(), 0, "Disabled buffer should not store entries");
    }

    #[test]
    fn test_clear_buffer() {
        let buffer = TracingBuffer::new(3, true);

        buffer.add_entry(Level::INFO, "test".to_string(), "message 1".to_string());
        buffer.add_entry(Level::INFO, "test".to_string(), "message 2".to_string());

        assert_eq!(buffer.len(), 2);

        buffer.clear();

        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_export_as_text() {
        let buffer = TracingBuffer::new(3, true);

        buffer.add_entry(Level::INFO, "test".to_string(), "message 1".to_string());
        buffer.add_entry(Level::WARN, "test".to_string(), "message 2".to_string());

        let text = buffer.export_as_text();
        assert!(text.contains("INFO"));
        assert!(text.contains("WARN"));
        assert!(text.contains("message 1"));
        assert!(text.contains("message 2"));
    }

    #[test]
    fn test_tracing_service() {
        let service = TracingService::new(10, true);

        let stats = service.stats();
        assert_eq!(stats.max_entries, 10);
        assert_eq!(stats.entries_in_buffer, 0);
        assert!(stats.enabled);
        assert!(service.is_enabled());
    }
}
