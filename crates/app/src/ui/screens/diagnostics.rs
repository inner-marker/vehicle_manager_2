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
            About {}
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
        About {}
        OfflineCopy {}
        crate::ui::protective::ProtectionDiagnostics {}
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
            // Only the list scrolls sideways on a narrow screen, never the page (issue #23).
            div { class: "log-table-scroll",
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
}

/// About (#060): the app's version and exact build (phase-3.md 1c), the server's when it
/// differs, the place data's version, and the data credits.
///
/// The web app and its server come from one image, so they share a build; the **Server** row
/// shows on the web only when the server has moved on since the page loaded. The desktop app
/// is built on its own, so it always shows it.
#[component]
fn About() -> Element {
    let mut info = use_signal(|| None::<Option<domain::api::PlaceDataInfo>>);
    let mut server = use_signal(|| None::<domain::api::Handshake>);
    // After mounting only (never during server rendering).
    use_effect(move || {
        spawn(async move {
            if let Ok(domain::api::PlacesOutcome::DataInfo { info: data }) =
                crate::api::get_place_data().await
            {
                info.set(Some(data));
            }
            if let Ok(handshake) = crate::api::handshake().await {
                server.set(Some(handshake));
            }
        });
    });
    let data = info().flatten();
    let version = data.as_ref().map_or_else(
        || tr(messages::ABOUT_PLACE_DATA_UNAVAILABLE),
        |data| data.data_version.clone(),
    );
    let credits = data
        .as_ref()
        .map(|data| data.attribution.clone())
        .filter(|credit| !credit.is_empty())
        .unwrap_or_else(|| tr(messages::ABOUT_CREDITS_DEFAULT));
    let own = crate::build_info::this_build();
    let label = |build: &domain::build_info::BuildInfo| {
        build.label(
            &tr(messages::ABOUT_BUILD_LOCAL),
            &tr(messages::ABOUT_BUILD_MODIFIED),
        )
    };
    let own_build = label(&own);
    let server_row = server().and_then(|handshake| {
        let differs = handshake.build != own || handshake.app_version != env!("CARGO_PKG_VERSION");
        (cfg!(feature = "desktop") || differs)
            .then(|| format!("{} · {}", handshake.app_version, label(&handshake.build)))
    });
    rsx! {
        section { class: "about",
            h3 { class: "section-heading", {tr(messages::ABOUT_HEADING).to_uppercase()} }
            dl { class: "about-list",
                dt { {tr(messages::ABOUT_APP)} }
                dd { {env!("CARGO_PKG_VERSION")} }
                dt { {tr(messages::ABOUT_BUILD)} }
                dd { {own_build} }
                if let Some(server_row) = server_row {
                    dt { {tr(messages::ABOUT_SERVER)} }
                    dd { {server_row} }
                }
                dt { {tr(messages::ABOUT_PLACE_DATA)} }
                dd { if info().is_some() { {version} } else { {tr(messages::LOADING)} } }
                dt { {tr(messages::ABOUT_CREDITS)} }
                dd { {credits} }
            }
        }
    }
}

/// The desktop's **Offline copy** (phase-3.md 4e; sketch option 2B): what it's doing, its last
/// pull, how many records it holds, its size and folder, and **Sync now**.
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn OfflineCopy() -> Element {
    use crate::local_copy::{CopyStatus, STATUS};

    let mut details = use_signal(|| None::<crate::local_copy::CopyDetails>);
    // This computer's files (6e, 3B), and the limit on photos.
    let mut files = use_signal(|| None::<storage::local_files::LocalFilesSummary>);
    let mut limit = use_signal(crate::copy_files::photo_limit);
    // Again whenever the status changes (a pull finished, the copy opened or closed), or the
    // outbox does (a photo added or sent).
    use_effect(move || {
        let _changed = (STATUS(), crate::local_copy::OUTBOX(), limit());
        spawn(async move {
            details.set(crate::local_copy::details().await);
            files.set(crate::copy_files::summary().await);
        });
    });
    let pick_limit = move |event: FormEvent| {
        let Some(picked) = event
            .value()
            .parse::<usize>()
            .ok()
            .and_then(|index| domain::attachment::PhotoLimit::ALL.get(index).copied())
        else {
            return;
        };
        spawn(async move {
            crate::copy_files::set_photo_limit(picked).await;
            limit.set(picked);
        });
    };
    let status = match STATUS() {
        CopyStatus::Closed => tr(messages::COPY_CLOSED),
        CopyStatus::Downloading { cursor, of } => tr_with(
            messages::COPY_DOWNLOADING,
            &[
                (
                    "done",
                    MessageArg::from(i64::try_from(cursor).unwrap_or(i64::MAX)),
                ),
                (
                    "total",
                    MessageArg::from(i64::try_from(of).unwrap_or(i64::MAX)),
                ),
            ],
        ),
        CopyStatus::UpToDate { .. } => tr(messages::COPY_UP_TO_DATE),
        CopyStatus::Offline { .. } => tr(messages::COPY_OFFLINE),
        CopyStatus::OtherSchema { server, app } => tr_with(
            messages::COPY_OTHER_SCHEMA,
            &[
                ("server", MessageArg::from(server)),
                ("app", MessageArg::from(app)),
            ],
        ),
        CopyStatus::Failed => tr(messages::COPY_FAILED),
        CopyStatus::Unwell => tr(messages::COPY_UNWELL),
    };
    let shown = details();
    let last_pull = shown
        .as_ref()
        .and_then(|shown| shown.last_pull)
        .map_or_else(
            || tr(messages::COPY_NEVER),
            |at| {
                tr_with(
                    messages::MESSAGES_TIME_UTC,
                    &[(
                        "time",
                        MessageArg::from(domain::time_utils::format_utc_minute(at)),
                    )],
                )
            },
        );
    let outbox = crate::local_copy::OUTBOX();
    let oldest = outbox.oldest_waiting.map(|at| {
        tr_with(
            messages::MESSAGES_TIME_UTC,
            &[(
                "time",
                MessageArg::from(domain::time_utils::format_utc_minute(at)),
            )],
        )
    });
    let stopped = crate::local_copy::PUSH_STOPPED()
        .filter(|_| outbox.waiting > 0)
        .map(crate::ui::outbox::stop_text);
    rsx! {
        section { class: "about",
            h3 { class: "section-heading", {tr(messages::COPY_HEADING).to_uppercase()} }
            dl { class: "about-list",
                dt { {tr(messages::COPY_STATUS)} }
                dd { {status} }
                if let Some(shown) = shown {
                    dt { {tr(messages::COPY_LAST_PULL)} }
                    dd { {last_pull} }
                    dt { {tr(messages::COPY_RECORDS)} }
                    dd { "{shown.records}" }
                    dt { {tr(messages::COPY_SIZE)} }
                    dd { {megabytes(shown.bytes)} }
                    dt { {tr(messages::COPY_FOLDER)} }
                    dd { class: "mono", {shown.folder.display().to_string()} }
                }
                // Changes made here (5e): waiting, refused, the oldest, why sending stopped.
                dt { {tr(messages::COPY_WAITING)} }
                dd { "{outbox.waiting}" }
                dt { {tr(messages::COPY_REFUSED)} }
                dd { "{outbox.refused}" }
                if let Some(oldest) = oldest {
                    dt { {tr(messages::COPY_OLDEST)} }
                    dd { {oldest} }
                }
                if let Some(stopped) = stopped {
                    dt { {tr(messages::COPY_SENDING)} }
                    dd { {stopped} }
                }
                // This computer's photos and PDFs (6e, 3B).
                if let Some(files) = files() {
                    dt { {tr(messages::COPY_FILES)} }
                    dd {
                        {tr_with(messages::COPY_FILES_VALUE, &[
                            ("count", MessageArg::from(i64::try_from(files.files).unwrap_or(i64::MAX))),
                            ("size", MessageArg::from(megabytes(files.bytes).as_str())),
                        ])}
                    }
                    dt { {tr(messages::COPY_PHOTOS_KEPT)} }
                    dd {
                        {tr_with(messages::COPY_PHOTOS_KEPT_VALUE, &[
                            ("used", MessageArg::from(megabytes(files.photo_bytes).as_str())),
                            ("limit", MessageArg::from(tr(limit().label()).as_str())),
                        ])}
                    }
                }
                dt { {tr(messages::COPY_PHOTO_LIMIT)} }
                dd {
                    // The value is the choice's index in PhotoLimit::ALL.
                    select { class: "photo-limit", "aria-label": tr(messages::COPY_PHOTO_LIMIT), onchange: pick_limit,
                        for (index, choice) in domain::attachment::PhotoLimit::ALL.iter().enumerate() {
                            option { key: "{index}", value: "{index}", selected: *choice == limit(), {tr(choice.label())} }
                        }
                    }
                }
            }
            button {
                class: "secondary-button",
                r#type: "button",
                onclick: move |_| crate::local_copy::pull_soon(),
                {tr(messages::COPY_SYNC_NOW)}
            }
        }
    }
}

/// Nothing on the web: it keeps no copy.
#[cfg(not(all(feature = "desktop", not(target_arch = "wasm32"))))]
#[component]
fn OfflineCopy() -> Element {
    rsx! {}
}

/// `bytes` as "4.2 MB".
#[cfg_attr(
    all(not(test), not(all(feature = "desktop", not(target_arch = "wasm32")))),
    expect(dead_code, reason = "only the desktop shows its copy's size")
)]
fn megabytes(bytes: u64) -> String {
    let tenths = bytes.saturating_add(52_428) / 104_857;
    format!("{}.{} MB", tenths / 10, tenths % 10)
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

    #[test]
    fn a_copys_size_reads_in_megabytes_to_a_tenth() {
        assert_eq!(megabytes(0), "0.0 MB");
        assert_eq!(megabytes(4_400_000), "4.2 MB");
        assert_eq!(megabytes(1_048_576), "1.0 MB");
    }

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
