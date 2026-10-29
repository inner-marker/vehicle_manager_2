//! Runtime control of per-area log levels (e.g. `sync` at `debug` while chasing a problem).
//!
//! Levels are not persisted: every start uses [`default_levels`], so a debugging
//! session never leaves debug logging switched on by accident.

use tracing_subscriber::Registry;
use tracing_subscriber::filter::{LevelFilter, Targets};
use tracing_subscriber::reload;

/// Default level for every area and dependency. Files keep info and above by
/// default (plan §5 Debug log, Retention).
pub const DEFAULT_LEVEL: LevelFilter = LevelFilter::INFO;

/// The startup filter: [`DEFAULT_LEVEL`] for everything.
pub fn default_levels() -> Targets {
    Targets::new().with_default(DEFAULT_LEVEL)
}

/// Changes which events are captured, while the app runs.
///
/// Wraps the reload handle of the global level filter, which sits directly on the
/// registry so it filters every layer (console, ring buffer, file) at once.
#[derive(Clone)]
pub struct LevelControl {
    handle: reload::Handle<Targets, Registry>,
}

impl LevelControl {
    /// Wraps the handle returned when the reloadable filter was created.
    pub fn new(handle: reload::Handle<Targets, Registry>) -> Self {
        Self { handle }
    }

    /// Sets the minimum level captured for one area (a `domain::log_area` name).
    ///
    /// # Errors
    ///
    /// Returns an error if the logging subscriber has been dropped.
    pub fn set_area_level(&self, area: &str, level: LevelFilter) -> Result<(), reload::Error> {
        self.handle
            .modify(|levels| *levels = std::mem::take(levels).with_target(area, level))
    }

    /// Restores [`default_levels`] for every area.
    ///
    /// # Errors
    ///
    /// Returns an error if the logging subscriber has been dropped.
    pub fn reset(&self) -> Result<(), reload::Error> {
        self.handle.reload(default_levels())
    }

    /// The level currently captured for `area`.
    ///
    /// # Errors
    ///
    /// Returns an error if the logging subscriber has been dropped.
    pub fn area_level(&self, area: &str) -> Result<LevelFilter, reload::Error> {
        // Check from most to least verbose; the first enabled level is the threshold.
        const MOST_VERBOSE_FIRST: [tracing::Level; 5] = [
            tracing::Level::TRACE,
            tracing::Level::DEBUG,
            tracing::Level::INFO,
            tracing::Level::WARN,
            tracing::Level::ERROR,
        ];
        self.handle.with_current(|levels| {
            MOST_VERBOSE_FIRST
                .iter()
                .find(|level| levels.would_enable(area, level))
                .map_or(LevelFilter::OFF, |level| LevelFilter::from_level(*level))
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use domain::log_area;
    use domain::test_support::ManualClock;
    use domain::time::UnixMillis;
    use tracing_subscriber::layer::SubscriberExt;

    use super::*;
    use crate::logging::capture::{CaptureLayer, EntrySink};
    use crate::logging::ring_buffer::RingBuffer;

    #[test]
    fn area_level_change_takes_effect_and_reset_restores_default() {
        let buffer = Arc::new(RingBuffer::new(10));
        let (filter, handle) = reload::Layer::new(default_levels());
        let control = LevelControl::new(handle);
        let capture = CaptureLayer::new(
            Arc::new(ManualClock::new(UnixMillis::EPOCH)),
            vec![Arc::clone(&buffer) as Arc<dyn EntrySink>],
        );
        let subscriber = tracing_subscriber::registry().with(filter).with(capture);

        tracing::subscriber::with_default(subscriber, || {
            tracing::debug!(target: log_area::SYNC, "hidden at the default level");

            control
                .set_area_level(log_area::SYNC, LevelFilter::DEBUG)
                .unwrap();
            tracing::debug!(target: log_area::SYNC, "visible after raising sync");
            tracing::debug!(target: log_area::AUTH, "other areas stay at info");

            control.reset().unwrap();
            tracing::debug!(target: log_area::SYNC, "hidden again after reset");
        });

        let messages: Vec<String> = buffer.snapshot().into_iter().map(|e| e.message).collect();
        assert_eq!(messages, ["visible after raising sync"]);
    }

    #[test]
    fn area_level_reports_the_current_threshold() {
        let (_filter, handle) = reload::Layer::<Targets, Registry>::new(default_levels());
        let control = LevelControl::new(handle);

        assert_eq!(
            control.area_level(log_area::SYNC).unwrap(),
            LevelFilter::INFO
        );
        control
            .set_area_level(log_area::SYNC, LevelFilter::DEBUG)
            .unwrap();
        assert_eq!(
            control.area_level(log_area::SYNC).unwrap(),
            LevelFilter::DEBUG
        );
        assert_eq!(control.area_level(log_area::UI).unwrap(), LevelFilter::INFO);
    }
}
