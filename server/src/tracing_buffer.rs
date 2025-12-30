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
    pub fn export_as_text(&self) -> String {
        let entries = self.get_entries();
        let mut output = String::new();

        for entry in entries {
            // Convert microseconds to human readable format
            let secs = entry.timestamp_us / 1_000_000;
            let micros = entry.timestamp_us % 1_000_000;
            
            // Format: timestamp level target - message
            output.push_str(&format!(
                "{}.{:06} {} {} - {}\n",
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
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: Context<'_, S>,
    ) {
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
            self.message.push_str(&format!("{} = {:?}", field.name(), value));
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
pub fn init_tracing_with_buffer(
    log_level: tracing::Level,
    tracing_service: &TracingService,
) {
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
    
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set tracing subscriber");
}
