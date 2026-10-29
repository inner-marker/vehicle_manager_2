//! Fixed-capacity in-memory buffer of recent log entries, feeding the live log viewer.

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard, PoisonError};

use super::capture::EntrySink;
use super::entry::LogEntry;

/// Entries kept in memory on every platform (plan §5 Debug log, Retention).
pub const RING_BUFFER_CAPACITY: usize = 5_000;

/// The most recent log entries, oldest first; the oldest are dropped once full.
///
/// Shared as `Arc<RingBuffer>` between the capture layer (which pushes) and the
/// log viewer and export (which read snapshots).
#[derive(Debug)]
pub struct RingBuffer {
    capacity: usize,
    entries: Mutex<VecDeque<LogEntry>>,
}

impl RingBuffer {
    /// Creates an empty buffer holding at most `capacity` entries (at least one).
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            capacity,
            entries: Mutex::new(VecDeque::with_capacity(capacity)),
        }
    }

    /// Adds an entry, dropping the oldest one if the buffer is full.
    pub fn push(&self, entry: LogEntry) {
        let mut entries = self.lock();
        if entries.len() == self.capacity {
            entries.pop_front();
        }
        entries.push_back(entry);
    }

    /// Returns a copy of the current entries, oldest first.
    pub fn snapshot(&self) -> Vec<LogEntry> {
        self.lock().iter().cloned().collect()
    }

    /// Number of entries currently held.
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    /// Whether the buffer holds no entries.
    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }

    /// Locks the entries. A panic while holding the lock can only leave a fully
    /// pushed or popped queue behind, so a poisoned lock is safe to keep using.
    fn lock(&self) -> MutexGuard<'_, VecDeque<LogEntry>> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl EntrySink for RingBuffer {
    fn accept(&self, entry: &LogEntry) {
        self.push(entry.clone());
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::logging::entry::LogLevel;

    fn entry(time_ms: i64) -> LogEntry {
        LogEntry {
            time_ms,
            level: LogLevel::Info,
            area: "app".to_owned(),
            message: format!("entry {time_ms}"),
            fields: BTreeMap::new(),
        }
    }

    #[test]
    fn ring_buffer_evicts_oldest_entries_beyond_capacity() {
        let buffer = RingBuffer::new(3);
        for time_ms in 1..=5 {
            buffer.push(entry(time_ms));
        }

        let kept: Vec<i64> = buffer.snapshot().iter().map(|e| e.time_ms).collect();
        assert_eq!(kept, [3, 4, 5]);
        assert_eq!(buffer.len(), 3);
    }

    #[test]
    fn ring_buffer_snapshot_is_oldest_first_and_independent() {
        let buffer = RingBuffer::new(10);
        assert!(buffer.is_empty());
        buffer.push(entry(1));
        buffer.push(entry(2));

        let snapshot = buffer.snapshot();
        buffer.push(entry(3));

        assert_eq!(snapshot.len(), 2, "later pushes must not change a snapshot");
        assert_eq!(snapshot[0].time_ms, 1);
    }

    #[test]
    fn ring_buffer_capacity_is_at_least_one() {
        let buffer = RingBuffer::new(0);
        buffer.push(entry(1));
        buffer.push(entry(2));
        assert_eq!(buffer.snapshot()[0].time_ms, 2);
    }
}
