//! Diagnostics: the connection test panel and the basic in-app log viewer (plan §5 Debug log).
//!
//! Shows this device's own log (the ring buffer), newest first, filtered by minimum
//! level and area. Entries load **after the screen mounts**: with fullstack, the
//! server renders the page first and the browser then takes it over, and each has
//! its own log buffer, so reading it during that first render would mismatch.
//! Copy/export, search, auto-scroll, and live refresh come later.

use dioxus::prelude::*;
use domain::i18n::{MessageArg, MessageId, messages};
use domain::log_area;
use domain::time_utils::format_utc_iso8601;

use crate::logging::{ExportFilter, LogEntry, LogHandles, LogLevel, format_fields};
use crate::ui::icons::{Icon, IconView};
use crate::ui::screens::ConnectionPanel;
use crate::ui::{tr, tr_with};

/// The log viewer screen.
// rsx! expands event handlers to fully qualified `dioxus_elements::events::on*` calls,
// and the prelude glob also imports those functions, so rustc flags the macro's own
// paths. Not fixable in our code (explicit imports would depend on Dioxus internals).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn Diagnostics() -> Element {
    let handles = try_use_context::<LogHandles>();
    let mut entries = use_signal(Vec::<LogEntry>::new);
    let mut min_level = use_signal(|| LogLevel::Info);
    let mut area = use_signal(|| None::<String>);

    // Runs only in the browser/app after mounting, never during server rendering.
    let handles_for_load = handles.clone();
    use_effect(move || {
        if let Some(handles) = &handles_for_load {
            entries.set(handles.ring_buffer.snapshot());
        }
    });

    let Some(handles) = handles else {
        return rsx! {
            ConnectionPanel {}
            p { class: "placeholder", {tr(messages::LOG_VIEWER_UNAVAILABLE)} }
        };
    };

    let visible = visible_entries(&entries.read(), min_level(), area().as_deref());
    let count = tr_with(
        messages::LOG_VIEWER_ENTRY_COUNT,
        &[(
            "count",
            MessageArg::from(i64::try_from(visible.len()).unwrap_or(i64::MAX)),
        )],
    );

    rsx! {
        ConnectionPanel {}
        div { class: "log-controls",
            label {
                {tr(messages::LOG_VIEWER_MIN_LEVEL)}
                select {
                    onchange: move |event| {
                        if let Some(level) = level_from_name(&event.value()) {
                            min_level.set(level);
                        }
                    },
                    for level in LogLevel::ALL {
                        option {
                            key: "{level.as_str()}",
                            value: level.as_str(),
                            selected: level == min_level(),
                            {tr(level_label(level))}
                        }
                    }
                }
            }
            label {
                {tr(messages::LOG_VIEWER_AREA)}
                select {
                    onchange: move |event| {
                        let value = event.value();
                        area.set(if value.is_empty() { None } else { Some(value) });
                    },
                    option { value: "", selected: area().is_none(), {tr(messages::LOG_VIEWER_ALL_AREAS)} }
                    for name in log_area::ALL {
                        option {
                            key: "{name}",
                            value: name,
                            selected: area().as_deref() == Some(name),
                            {area_label(name)}
                        }
                    }
                }
            }
            button {
                class: "refresh",
                onclick: move |_| {
                    entries.set(handles.ring_buffer.snapshot());
                    tracing::debug!(target: log_area::UI, "log viewer refreshed");
                },
                IconView { icon: Icon::Refresh }
                span { {tr(messages::ACTION_REFRESH)} }
            }
            span { class: "log-count", {count} }
        }

        if visible.is_empty() {
            p { class: "placeholder", {tr(messages::LOG_VIEWER_EMPTY)} }
        } else {
            table { class: "log-table",
                thead {
                    tr {
                        th { {tr(messages::LOG_VIEWER_TIME)} }
                        th { {tr(messages::LOG_VIEWER_LEVEL)} }
                        th { {tr(messages::LOG_VIEWER_AREA)} }
                        th { {tr(messages::LOG_VIEWER_MESSAGE)} }
                    }
                }
                tbody {
                    for (index, entry) in visible.iter().enumerate() {
                        tr { key: "{index}", class: "level-{entry.level.as_str()}",
                            td { class: "log-time", {format_utc_iso8601(entry.time())} }
                            td { {tr(level_label(entry.level))} }
                            td { {area_label(&entry.area)} }
                            td {
                                {entry.message.clone()}
                                span { class: "log-fields", " {format_fields(entry)}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The entries to show: at or above `min_level`, in `area` if one is chosen, newest first.
pub(crate) fn visible_entries(
    entries: &[LogEntry],
    min_level: LogLevel,
    area: Option<&str>,
) -> Vec<LogEntry> {
    let filter = ExportFilter {
        min_level,
        areas: area.map(|name| vec![name.to_owned()]),
        ..ExportFilter::default()
    };
    // The ring buffer is oldest first; the viewer shows newest first.
    entries
        .iter()
        .rev()
        .filter(|entry| filter.matches(entry))
        .cloned()
        .collect()
}

fn level_from_name(name: &str) -> Option<LogLevel> {
    LogLevel::ALL
        .into_iter()
        .find(|level| level.as_str() == name)
}

const fn level_label(level: LogLevel) -> MessageId {
    match level {
        LogLevel::Error => messages::LOG_LEVEL_ERROR,
        LogLevel::Warn => messages::LOG_LEVEL_WARN,
        LogLevel::Info => messages::LOG_LEVEL_INFO,
        LogLevel::Debug => messages::LOG_LEVEL_DEBUG,
        LogLevel::Trace => messages::LOG_LEVEL_TRACE,
    }
}

/// The translated name of one of our log areas.
fn area_message(area: &str) -> Option<MessageId> {
    match area {
        log_area::APP => Some(messages::LOG_AREA_APP),
        log_area::UI => Some(messages::LOG_AREA_UI),
        log_area::SYNC => Some(messages::LOG_AREA_SYNC),
        log_area::DATABASE => Some(messages::LOG_AREA_DATABASE),
        log_area::AUTH => Some(messages::LOG_AREA_AUTH),
        _ => None,
    }
}

/// The area's translated name, or its raw target for dependency logs (e.g. `dioxus_core`).
fn area_label(area: &str) -> String {
    area_message(area).map_or_else(|| area.to_owned(), tr)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn entry(time_ms: i64, level: LogLevel, area: &str) -> LogEntry {
        LogEntry {
            time_ms,
            level,
            area: area.to_owned(),
            message: format!("{area} at {time_ms}"),
            fields: BTreeMap::new(),
        }
    }

    #[test]
    fn log_viewer_filters_by_level_and_area_newest_first() {
        let entries = [
            entry(1, LogLevel::Info, log_area::SYNC),
            entry(2, LogLevel::Debug, log_area::SYNC),
            entry(3, LogLevel::Warn, log_area::UI),
            entry(4, LogLevel::Error, log_area::SYNC),
        ];
        let times = |shown: Vec<LogEntry>| shown.iter().map(|e| e.time_ms).collect::<Vec<_>>();

        assert_eq!(
            times(visible_entries(&entries, LogLevel::Info, None)),
            [4, 3, 1]
        );
        assert_eq!(
            times(visible_entries(
                &entries,
                LogLevel::Debug,
                Some(log_area::SYNC)
            )),
            [4, 2, 1]
        );
    }

    #[test]
    fn every_log_area_has_a_translated_label() {
        for area in log_area::ALL {
            assert!(area_message(area).is_some(), "no label for {area}");
            assert_ne!(
                area_label(area),
                area,
                "label for {area} should be translated"
            );
        }
        assert_eq!(
            area_label("dioxus_core"),
            "dioxus_core",
            "dependency targets show as-is"
        );
    }

    #[test]
    fn level_names_parse_back_to_levels() {
        for level in LogLevel::ALL {
            assert_eq!(level_from_name(level.as_str()), Some(level));
        }
        assert_eq!(level_from_name("LOUD"), None);
    }
}
