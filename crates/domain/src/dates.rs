//! Event dates: UTC plus the place's time zone (decision #008), entered and shown in local
//! time, and formatted by each person's date format (#048).
//!
//! - **Stored:** milliseconds since 1970 UTC ([`UnixMillis`]) and the IANA zone where it
//!   happened ([`TimeZone`]), so an event keeps its local time even if the person travels.
//! - **Entered:** a local date (`2026-11-02`) and time (`15:30`), as `<input type="date">` and
//!   `<input type="time">` give them, in the event's zone. A time that doesn't exist (the hour
//!   skipped when clocks go forward) moves forward past the gap; a time that happens twice (when
//!   clocks go back) means the first one: the usual, "compatible" rule.
//! - **Zones** come from `jiff`'s built-in copy of the IANA time-zone database, the same on the
//!   NAS, the desktop app, and in the browser.
//! - **Shown:** "30 Oct 2026", "10/30/2026", "30/10/2026", or "2026-10-30" (the person's
//!   choice), and times as "3:30 pm".
//!
//! # Examples
//!
//! ```
//! use domain::dates::{local_to_utc, utc_to_local};
//! use domain::location::TimeZone;
//!
//! let asheville = TimeZone::parse("America/New_York").unwrap();
//! let at = local_to_utc("2026-11-02", "15:30", &asheville).unwrap();
//! assert_eq!(at.as_millis(), 1_793_651_400_000); // 20:30 UTC
//! assert_eq!(utc_to_local(at, &asheville).unwrap(), ("2026-11-02".into(), "15:30".into()));
//! ```

use serde::{Deserialize, Serialize};

use crate::i18n::{MessageId, Translator, messages};
use crate::location::TimeZone;
use crate::preferences::DateFormat;
use crate::time::UnixMillis;

/// Why a date, time, or zone was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DateError {
    /// Not a date (`2026-11-02`).
    BadDate,
    /// Not a time (`15:30`).
    BadTime,
    /// Not a time zone the database knows.
    UnknownZone,
    /// An end date before the start.
    EndBeforeStart,
    /// Outside the years the app handles (1900–2200).
    OutOfRange,
}

impl DateError {
    /// The message explaining the problem.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::BadDate => messages::DATE_BAD_DATE,
            Self::BadTime => messages::DATE_BAD_TIME,
            Self::UnknownZone => messages::DATE_UNKNOWN_ZONE,
            Self::EndBeforeStart => messages::DATE_END_BEFORE_START,
            Self::OutOfRange => messages::DATE_OUT_OF_RANGE,
        }
    }
}

/// The earliest and latest years an event may have (records go back a long way; the plan's
/// limits elsewhere are 1800–2200 for model years).
const YEARS: core::ops::RangeInclusive<i16> = 1900..=2200;

/// The zone's rules from the built-in database.
fn rules(zone: &TimeZone) -> Result<jiff::tz::TimeZone, DateError> {
    jiff::tz::db()
        .get(zone.as_str())
        .map_err(|_| DateError::UnknownZone)
}

/// Whether the time-zone database knows `zone` (e.g. `America/New_York`).
#[must_use]
pub fn is_known_zone(zone: &TimeZone) -> bool {
    rules(zone).is_ok()
}

/// A local date and time in `zone`, as UTC.
///
/// # Errors
///
/// [`DateError`] if the date, time, or zone isn't valid, or the year is out of range.
pub fn local_to_utc(date: &str, time: &str, zone: &TimeZone) -> Result<UnixMillis, DateError> {
    let date: jiff::civil::Date = date.trim().parse().map_err(|_| DateError::BadDate)?;
    if !YEARS.contains(&date.year()) {
        return Err(DateError::OutOfRange);
    }
    let time: jiff::civil::Time = time.trim().parse().map_err(|_| DateError::BadTime)?;
    let zoned = date
        .to_datetime(time)
        .to_zoned(rules(zone)?)
        .map_err(|_| DateError::OutOfRange)?;
    Ok(UnixMillis::from_millis(zoned.timestamp().as_millisecond()))
}

/// The moment `at` in `zone`.
fn zoned(at: UnixMillis, zone: &TimeZone) -> Result<jiff::Zoned, DateError> {
    let timestamp =
        jiff::Timestamp::from_millisecond(at.as_millis()).map_err(|_| DateError::OutOfRange)?;
    Ok(timestamp.to_zoned(rules(zone)?))
}

/// `at` in `zone`, as the local date (`2026-11-02`) and time (`15:30`) a form shows.
///
/// # Errors
///
/// [`DateError`] if the zone isn't known or `at` is out of range.
pub fn utc_to_local(at: UnixMillis, zone: &TimeZone) -> Result<(String, String), DateError> {
    let local = zoned(at, zone)?;
    Ok((
        local.date().to_string(),
        format!("{:02}:{:02}", local.hour(), local.minute()),
    ))
}

/// A month's short name.
const fn month_name(month: i8) -> MessageId {
    match month {
        1 => messages::MONTH_1,
        2 => messages::MONTH_2,
        3 => messages::MONTH_3,
        4 => messages::MONTH_4,
        5 => messages::MONTH_5,
        6 => messages::MONTH_6,
        7 => messages::MONTH_7,
        8 => messages::MONTH_8,
        9 => messages::MONTH_9,
        10 => messages::MONTH_10,
        11 => messages::MONTH_11,
        _ => messages::MONTH_12,
    }
}

/// `at`'s date in `zone`, in the person's format: "30 Oct 2026", "10/30/2026", "30/10/2026", or
/// "2026-10-30".
///
/// # Errors
///
/// [`DateError`] if the zone isn't known or `at` is out of range.
pub fn format_date(
    at: UnixMillis,
    zone: &TimeZone,
    format: DateFormat,
    translator: &Translator,
) -> Result<String, DateError> {
    let date = zoned(at, zone)?.date();
    let (y, m, d) = (date.year(), date.month(), date.day());
    Ok(match format {
        DateFormat::DayMonthNameYear => format!("{d} {} {y}", translator.tr(month_name(m))),
        DateFormat::MonthDayYear => format!("{m:02}/{d:02}/{y}"),
        DateFormat::DayMonthYear => format!("{d:02}/{m:02}/{y}"),
        DateFormat::YearMonthDay => format!("{y}-{m:02}-{d:02}"),
    })
}

/// `at`'s time in `zone`, as "3:30 pm".
///
/// # Errors
///
/// [`DateError`] if the zone isn't known or `at` is out of range.
pub fn format_time(
    at: UnixMillis,
    zone: &TimeZone,
    translator: &Translator,
) -> Result<String, DateError> {
    let local = zoned(at, zone)?;
    let hour = local.hour();
    let half = translator.tr(if hour < 12 {
        messages::TIME_AM
    } else {
        messages::TIME_PM
    });
    let hour12 = match hour % 12 {
        0 => 12,
        other => other,
    };
    Ok(format!("{hour12}:{:02} {half}", local.minute()))
}

/// The month and year of `at` in `zone`, for a heading: "November 2026", and the year and
/// month as numbers, to group by.
///
/// # Errors
///
/// [`DateError`] if the zone isn't known or `at` is out of range.
pub fn month_heading(
    at: UnixMillis,
    zone: &TimeZone,
    translator: &Translator,
) -> Result<(String, (i16, i8)), DateError> {
    let date = zoned(at, zone)?.date();
    let name = translator.tr(long_month_name(date.month()));
    Ok((
        format!("{name} {}", date.year()),
        (date.year(), date.month()),
    ))
}

/// A month's full name.
const fn long_month_name(month: i8) -> MessageId {
    match month {
        1 => messages::MONTH_LONG_1,
        2 => messages::MONTH_LONG_2,
        3 => messages::MONTH_LONG_3,
        4 => messages::MONTH_LONG_4,
        5 => messages::MONTH_LONG_5,
        6 => messages::MONTH_LONG_6,
        7 => messages::MONTH_LONG_7,
        8 => messages::MONTH_LONG_8,
        9 => messages::MONTH_LONG_9,
        10 => messages::MONTH_LONG_10,
        11 => messages::MONTH_LONG_11,
        _ => messages::MONTH_LONG_12,
    }
}

/// An event's dates: when it started, when it ended if it spans several days, and the zone
/// where it happened (plan §3 Event dates).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventDates {
    /// When it started (UTC).
    pub start: UnixMillis,
    /// When it ended, for a multi-day event (UTC); at or after `start`.
    pub end: Option<UnixMillis>,
    /// Where it happened's time zone.
    pub zone: TimeZone,
}

impl EventDates {
    /// Dates, checked: a known zone, and an end at or after the start.
    ///
    /// # Errors
    ///
    /// [`DateError::UnknownZone`] or [`DateError::EndBeforeStart`].
    pub fn new(
        start: UnixMillis,
        end: Option<UnixMillis>,
        zone: TimeZone,
    ) -> Result<Self, DateError> {
        if !is_known_zone(&zone) {
            return Err(DateError::UnknownZone);
        }
        if end.is_some_and(|end| end < start) {
            return Err(DateError::EndBeforeStart);
        }
        Ok(Self { start, end, zone })
    }

    /// The dates in the person's format: one date, or "3 Nov 2026 – 12 Nov 2026".
    ///
    /// # Errors
    ///
    /// [`DateError`] if the zone isn't known or a date is out of range.
    pub fn format(&self, format: DateFormat, translator: &Translator) -> Result<String, DateError> {
        let start = format_date(self.start, &self.zone, format, translator)?;
        match self.end {
            Some(end) => {
                let end = format_date(end, &self.zone, format, translator)?;
                Ok(if end == start {
                    start
                } else {
                    format!("{start} – {end}")
                })
            }
            None => Ok(start),
        }
    }
}

/// Today's date (`2026-11-02`) in `zone` at `now`, to start a form with.
///
/// # Errors
///
/// [`DateError`] if the zone isn't known or `now` is out of range.
pub fn today(now: UnixMillis, zone: &TimeZone) -> Result<String, DateError> {
    utc_to_local(now, zone).map(|(date, _)| date)
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn zone(name: &str) -> TimeZone {
        TimeZone::parse(name).unwrap()
    }

    #[test]
    fn local_times_become_utc_in_their_zone() {
        let new_york = zone("America/New_York");
        // EDT (UTC−4) in summer, EST (UTC−5) in winter.
        let summer = local_to_utc("2026-07-04", "12:00", &new_york).unwrap();
        assert_eq!(summer.as_millis(), 1_783_180_800_000);
        let winter = local_to_utc("2026-01-15", "12:00", &new_york).unwrap();
        assert_eq!(winter.as_millis(), 1_768_496_400_000);
        let utc = local_to_utc("2026-01-15", "17:00", &zone("UTC")).unwrap();
        assert_eq!(utc, winter, "noon in New York in January is 17:00 UTC");
    }

    #[test]
    fn the_skipped_hour_moves_forward_and_the_repeated_hour_is_the_first() {
        let new_york = zone("America/New_York");
        // 2026-03-08 02:30 doesn't exist (clocks go 02:00 → 03:00): it becomes 03:30 EDT.
        let gap = local_to_utc("2026-03-08", "02:30", &new_york).unwrap();
        assert_eq!(utc_to_local(gap, &new_york).unwrap().1, "03:30");
        // 2026-11-01 01:30 happens twice: the first (EDT, 05:30 UTC).
        let fold = local_to_utc("2026-11-01", "01:30", &new_york).unwrap();
        let first = local_to_utc("2026-11-01", "05:30", &zone("UTC")).unwrap();
        assert_eq!(fold, first);
    }

    #[test]
    fn bad_dates_times_and_zones_are_refused() {
        let utc = zone("UTC");
        assert_eq!(
            local_to_utc("2026-02-30", "12:00", &utc),
            Err(DateError::BadDate)
        );
        assert_eq!(
            local_to_utc("02/11/2026", "12:00", &utc),
            Err(DateError::BadDate)
        );
        assert_eq!(
            local_to_utc("2026-11-02", "25:00", &utc),
            Err(DateError::BadTime)
        );
        assert_eq!(
            local_to_utc("1850-01-01", "12:00", &utc),
            Err(DateError::OutOfRange)
        );
        assert_eq!(
            local_to_utc("2026-11-02", "12:00", &zone("Mars/Olympus_Mons")),
            Err(DateError::UnknownZone)
        );
        assert!(is_known_zone(&zone("America/Argentina/Buenos_Aires")));
        assert!(!is_known_zone(&zone("Nowhere/Special")));
    }

    #[test]
    fn a_month_heading_is_in_the_events_zone() {
        let english = Translator::english();
        // 1 Nov 2026 02:00 UTC is still 31 October in New York.
        let at = UnixMillis::from_millis(1_793_498_400_000);
        assert_eq!(
            month_heading(at, &zone("UTC"), english).unwrap(),
            ("November 2026".to_owned(), (2026, 11))
        );
        assert_eq!(
            month_heading(at, &zone("America/New_York"), english).unwrap(),
            ("October 2026".to_owned(), (2026, 10))
        );
    }

    #[test]
    fn dates_and_times_are_shown_in_the_persons_format() {
        let tr = Translator::english();
        let new_york = zone("America/New_York");
        // 2026-10-30 23:30 in New York is already the 31st in UTC: the event's zone decides.
        let at = local_to_utc("2026-10-30", "23:30", &new_york).unwrap();
        let shown = |format| format_date(at, &new_york, format, tr).unwrap();
        assert_eq!(shown(DateFormat::DayMonthNameYear), "30 Oct 2026");
        assert_eq!(shown(DateFormat::MonthDayYear), "10/30/2026");
        assert_eq!(shown(DateFormat::DayMonthYear), "30/10/2026");
        assert_eq!(shown(DateFormat::YearMonthDay), "2026-10-30");
        assert_eq!(format_time(at, &new_york, tr).unwrap(), "11:30 pm");
        let midnight = local_to_utc("2026-10-30", "00:05", &new_york).unwrap();
        assert_eq!(format_time(midnight, &new_york, tr).unwrap(), "12:05 am");
        let noon = local_to_utc("2026-10-30", "12:00", &new_york).unwrap();
        assert_eq!(format_time(noon, &new_york, tr).unwrap(), "12:00 pm");
    }

    #[test]
    fn multi_day_events_end_on_or_after_they_start() {
        let tr = Translator::english();
        let utc = zone("UTC");
        let start = local_to_utc("2026-03-03", "09:00", &utc).unwrap();
        let end = local_to_utc("2026-03-12", "17:00", &utc).unwrap();
        let dates = EventDates::new(start, Some(end), utc.clone()).unwrap();
        assert_eq!(
            dates.format(DateFormat::DayMonthNameYear, tr).unwrap(),
            "3 Mar 2026 – 12 Mar 2026"
        );
        let same_day = EventDates::new(start, Some(start), utc.clone()).unwrap();
        assert_eq!(
            same_day.format(DateFormat::YearMonthDay, tr).unwrap(),
            "2026-03-03"
        );
        assert_eq!(
            EventDates::new(end, Some(start), utc),
            Err(DateError::EndBeforeStart)
        );
        assert_eq!(
            EventDates::new(start, None, zone("Nowhere/Special")),
            Err(DateError::UnknownZone)
        );
    }

    #[test]
    fn today_is_the_zones_date() {
        let utc = zone("UTC");
        // 02:00 UTC on the 2nd is still the 1st in New York.
        let now = local_to_utc("2026-11-02", "02:00", &utc).unwrap();
        assert_eq!(today(now, &zone("America/New_York")).unwrap(), "2026-11-01");
        assert_eq!(today(now, &utc).unwrap(), "2026-11-02");
    }

    const ZONES: [&str; 6] = [
        "UTC",
        "America/New_York",
        "Europe/London",
        "Asia/Kolkata",
        "Australia/Lord_Howe",
        "Pacific/Chatham",
    ];

    proptest! {
        #[test]
        fn local_times_round_trip_except_in_a_skipped_hour(
            zone_index in 0..ZONES.len(),
            day in 0i64..73_000,
            minute in 0i64..1_440,
        ) {
            // A day from 1900 onwards and a minute of it, as local text.
            let date = jiff::civil::date(1900, 1, 1)
                .checked_add(jiff::Span::new().days(day))
                .unwrap();
            prop_assume!(YEARS.contains(&date.year()));
            let time = format!("{:02}:{:02}", minute / 60, minute % 60);
            let zone = zone(ZONES[zone_index]);
            let at = local_to_utc(&date.to_string(), &time, &zone).unwrap();
            let (back_date, back_time) = utc_to_local(at, &zone).unwrap();
            if (back_date.as_str(), back_time.as_str()) != (date.to_string().as_str(), time.as_str()) {
                // Only a time that doesn't exist moves, and only forward, by at most two hours.
                let typed = local_to_utc(&date.to_string(), &time, &TimeZone::parse("UTC").unwrap()).unwrap();
                let shown = local_to_utc(&back_date, &back_time, &TimeZone::parse("UTC").unwrap()).unwrap();
                let moved = shown.as_millis() - typed.as_millis();
                prop_assert!(moved > 0 && moved <= 2 * 3_600_000, "{date} {time} → {back_date} {back_time}");
            }
        }
    }
}
