//! Date and calendar helpers that work on [`UnixMillis`] without any time-zone database.
//!
//! These are exact integer calculations, so they behave identically on every
//! platform (including WASM) and never read the clock.

use crate::time::UnixMillis;

/// Milliseconds in one day.
const MILLIS_PER_DAY: i64 = 86_400_000;

/// Formats an instant as an ISO-8601 UTC timestamp with millisecond precision,
/// e.g. `2023-11-14T22:13:20.000Z`.
///
/// Meant for machine-facing text such as log exports. User-facing dates are
/// formatted by locale and in the event's own time zone instead.
///
/// # Examples
///
/// ```
/// use domain::time::UnixMillis;
/// use domain::time_utils::format_utc_iso8601;
///
/// let instant = UnixMillis::from_millis(1_700_000_000_123);
/// assert_eq!(format_utc_iso8601(instant), "2023-11-14T22:13:20.123Z");
/// ```
pub fn format_utc_iso8601(instant: UnixMillis) -> String {
    let millis = instant.as_millis();
    // Euclidean division keeps the time of day non-negative for instants before 1970.
    let days_since_epoch = millis.div_euclid(MILLIS_PER_DAY);
    let millis_of_day = millis.rem_euclid(MILLIS_PER_DAY);

    let (year, month, day) = civil_from_days(days_since_epoch);
    let hour = millis_of_day / 3_600_000;
    let minute = millis_of_day / 60_000 % 60;
    let second = millis_of_day / 1_000 % 60;
    let milli = millis_of_day % 1_000;

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{milli:03}Z")
}

/// Converts days since 1970-01-01 to a proleptic Gregorian `(year, month, day)`.
///
/// Howard Hinnant's `civil_from_days` algorithm
/// (<https://howardhinnant.github.io/date_algorithms.html>). It works in 400-year
/// "eras" of 146,097 days, with years starting on March 1 so the leap day falls
/// at the end of the year.
fn civil_from_days(days_since_epoch: i64) -> (i64, i64, i64) {
    // Shift the epoch from 1970-01-01 to 0000-03-01 (719,468 days earlier).
    let days = days_since_epoch + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days.rem_euclid(146_097); // [0, 146096]
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365; // [0, 399]
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100); // [0, 365]
    let month_index = (5 * day_of_year + 2) / 153; // [0, 11], March = 0
    let day = day_of_year - (153 * month_index + 2) / 5 + 1; // [1, 31]
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    }; // [1, 12]
    // January and February belong to the next calendar year in the March-based count.
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format(millis: i64) -> String {
        format_utc_iso8601(UnixMillis::from_millis(millis))
    }

    #[test]
    fn formats_the_epoch() {
        assert_eq!(format(0), "1970-01-01T00:00:00.000Z");
    }

    #[test]
    fn formats_milliseconds_and_time_of_day() {
        assert_eq!(format(1_700_000_000_123), "2023-11-14T22:13:20.123Z");
        assert_eq!(format(MILLIS_PER_DAY - 1), "1970-01-01T23:59:59.999Z");
    }

    #[test]
    fn formats_instants_before_1970() {
        assert_eq!(format(-1), "1969-12-31T23:59:59.999Z");
        assert_eq!(format(-MILLIS_PER_DAY), "1969-12-31T00:00:00.000Z");
    }

    #[test]
    fn handles_leap_days_and_century_rules() {
        // 2024 is a leap year; 2000 is (divisible by 400); 2100 is not (divisible by 100).
        assert_eq!(format(1_709_164_800_000), "2024-02-29T00:00:00.000Z");
        assert_eq!(format(951_782_400_000), "2000-02-29T00:00:00.000Z");
        assert_eq!(format(4_107_542_400_000), "2100-03-01T00:00:00.000Z");
    }

    #[test]
    fn formats_year_boundaries() {
        assert_eq!(format(1_704_067_199_999), "2023-12-31T23:59:59.999Z");
        assert_eq!(format(1_704_067_200_000), "2024-01-01T00:00:00.000Z");
    }
}
