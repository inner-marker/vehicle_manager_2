//! The tracing layer that turns each event into a [`LogEntry`] and hands it to the sinks.

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::sync::Arc;

use domain::time::Clock;
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};

use super::entry::{LogEntry, LogLevel};

/// Somewhere captured entries go: the ring buffer, and on native the log file.
///
/// Sinks must never log through `tracing` themselves: that would re-enter the
/// capture layer and could deadlock on the sink's own lock.
pub trait EntrySink: Send + Sync {
    /// Receives one captured entry.
    fn accept(&self, entry: &LogEntry);
}

/// A `tracing` layer that builds one [`LogEntry`] per event and passes it to every sink.
///
/// It sits after the level filter, so it only sees events enabled at the current
/// per-area levels.
pub struct CaptureLayer {
    clock: Arc<dyn Clock + Send + Sync>,
    sinks: Vec<Arc<dyn EntrySink>>,
}

impl CaptureLayer {
    /// Creates a layer that timestamps entries with `clock` and sends them to `sinks`.
    pub fn new(clock: Arc<dyn Clock + Send + Sync>, sinks: Vec<Arc<dyn EntrySink>>) -> Self {
        Self { clock, sinks }
    }
}

impl<S: Subscriber> Layer<S> for CaptureLayer {
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        let metadata = event.metadata();
        let mut visitor = FieldCollector::default();
        event.record(&mut visitor);

        let entry = LogEntry {
            time_ms: self.clock.now().as_millis(),
            level: LogLevel::from(metadata.level()),
            area: metadata.target().to_owned(),
            message: visitor.message,
            fields: visitor.fields,
        };
        for sink in &self.sinks {
            sink.accept(&entry);
        }
    }
}

/// Collects an event's `message` and other fields, keeping each value's JSON type.
#[derive(Default)]
struct FieldCollector {
    message: String,
    fields: BTreeMap<String, serde_json::Value>,
}

impl FieldCollector {
    fn insert(&mut self, field: &Field, value: serde_json::Value) {
        self.fields.insert(field.name().to_owned(), value);
    }
}

/// `tracing` names the format-string part of `info!("...")` "message".
const MESSAGE_FIELD: &str = "message";

impl Visit for FieldCollector {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == MESSAGE_FIELD {
            value.clone_into(&mut self.message);
        } else {
            self.insert(field, serde_json::Value::from(value));
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.insert(field, serde_json::Value::from(value));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.insert(field, serde_json::Value::from(value));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.insert(field, serde_json::Value::from(value));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.insert(field, serde_json::Value::from(value));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        // Formatted messages (`info!("x = {x}")`) and `?value` fields arrive here.
        let text = format!("{value:?}");
        if field.name() == MESSAGE_FIELD {
            self.message = text;
        } else {
            self.insert(field, serde_json::Value::from(text));
        }
    }
}

#[cfg(test)]
mod tests {
    use domain::log_area;
    use domain::test_support::ManualClock;
    use domain::time::UnixMillis;
    use tracing_subscriber::layer::SubscriberExt;

    use super::*;
    use crate::logging::ring_buffer::RingBuffer;

    #[test]
    fn capture_records_level_area_message_and_fields() {
        let clock = Arc::new(ManualClock::new(UnixMillis::from_millis(1_700_000_000_000)));
        let buffer = Arc::new(RingBuffer::new(10));
        let capture = CaptureLayer::new(clock, vec![Arc::clone(&buffer) as Arc<dyn EntrySink>]);
        let subscriber = tracing_subscriber::registry().with(capture);

        tracing::subscriber::with_default(subscriber, || {
            let device = "phone";
            tracing::warn!(target: log_area::SYNC, attempt = 3, ok = false, device, "push failed: {}", "timeout");
        });

        let entries = buffer.snapshot();
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.time_ms, 1_700_000_000_000);
        assert_eq!(entry.level, LogLevel::Warn);
        assert_eq!(entry.area, "sync");
        assert_eq!(entry.message, "push failed: timeout");
        assert_eq!(entry.fields["attempt"], serde_json::json!(3));
        assert_eq!(entry.fields["ok"], serde_json::json!(false));
        assert_eq!(entry.fields["device"], serde_json::json!("phone"));
    }
}
