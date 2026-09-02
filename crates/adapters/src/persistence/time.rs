// SPDX-License-Identifier: Apache-2.0

//! Time, formatted by hand.
//!
//! [`now_secs`] is the **only wall-clock read** in the workspace. Everything
//! that needs a timestamp receives one as an argument, so an analysis is a
//! pure function of its inputs and re-running it against a snapshot produces
//! the same bytes (INV-14, INV-15). A function that quietly consulted the
//! clock would break that without failing a test, which is why there is
//! exactly one of them, it is named here, and
//! `crates/adapters/tests/invariants.rs` scans the source for a second.
//!
//! The one other clock in the workspace is the executor's `Instant::now`,
//! which measures how long an invocation took and lands in
//! `Invocation::duration_ms`. It is monotonic, never a date, and never an
//! input to a recommendation — but it does mean a *recording* is not
//! byte-reproducible across two captures, only a snapshot is.
//!
//! The formatting is written out rather than pulled in: a date crate would add
//! a dependency, and with it a deployment-floor and licence question
//! (`08-distribution-and-project.md`, "Compatibility posture"), for two
//! functions of about twenty lines. UTC only — there is no local-time
//! formatting here, because a timestamp that means different things on two
//! machines is not a fixture identifier.
//!
//! The calendar conversion is the standard proleptic-Gregorian
//! `civil_from_days` algorithm (era arithmetic, 400-year cycles), checked
//! against `date -u -r <seconds>` in the tests below.

use std::time::{SystemTime, UNIX_EPOCH};

/// Seconds in a day.
const DAY: u64 = 86_400;
/// Days from 0000-03-01 (the algorithm's era origin) to 1970-01-01.
const EPOCH_SHIFT: u64 = 719_468;
/// Days in a 400-year Gregorian era.
const ERA_DAYS: u64 = 146_097;

/// The current time, in seconds since the Unix epoch.
///
/// The one clock read in the workspace. A clock set before the epoch reports
/// `0` rather than failing: the timestamp labels a capture, and refusing to
/// take a snapshot because a machine's clock is wrong would be the wrong
/// trade.
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Formats seconds since the epoch as RFC 3339 UTC, `YYYY-MM-DDTHH:MM:SSZ`.
pub fn rfc3339_utc(secs_since_epoch: u64) -> String {
    let (year, month, day) = civil_from_days(secs_since_epoch / DAY);
    let (hour, minute, second) = clock_from_seconds(secs_since_epoch % DAY);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Formats seconds since the epoch as a UTC calendar date, `YYYY-MM-DD`. This
/// is the form a dated fact carries (INV-20).
pub fn today_from(secs_since_epoch: u64) -> String {
    let (year, month, day) = civil_from_days(secs_since_epoch / DAY);
    format!("{year:04}-{month:02}-{day:02}")
}

/// A compact stamp for a file name: `YYYYMMDDTHHMMSSZ`, no separators, sorts
/// chronologically as text.
pub fn compact_stamp(secs_since_epoch: u64) -> String {
    let (year, month, day) = civil_from_days(secs_since_epoch / DAY);
    let (hour, minute, second) = clock_from_seconds(secs_since_epoch % DAY);
    format!("{year:04}{month:02}{day:02}T{hour:02}{minute:02}{second:02}Z")
}

/// Splits the seconds within a day.
fn clock_from_seconds(within_day: u64) -> (u64, u64, u64) {
    (within_day / 3600, (within_day / 60) % 60, within_day % 60)
}

/// Proleptic-Gregorian civil date from days since 1970-01-01.
///
/// Shifting the origin to 0000-03-01 makes a leap day the last day of the
/// year, which is what removes every special case from the arithmetic below.
/// Only non-negative days are representable here, which is all a `u64`
/// timestamp can produce.
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    let z = days + EPOCH_SHIFT;
    let era = z / ERA_DAYS;
    let day_of_era = z % ERA_DAYS; // [0, 146096]
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    // March is month 0 in this frame; the 153-day, five-month pattern of
    // month lengths repeats exactly, so the month is a division.
    let shifted_month = (5 * day_of_year + 2) / 153; // [0, 11]
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1; // [1, 31]
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = if month <= 2 { year + 1 } else { year };
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_instants_format_exactly() {
        // Every expectation below was produced by
        // `date -u -r <seconds> +%Y-%m-%dT%H:%M:%SZ` on 2026-09-02.
        for (secs, expected) in [
            (0_u64, "1970-01-01T00:00:00Z"),
            (1, "1970-01-01T00:00:01Z"),
            (86_399, "1970-01-01T23:59:59Z"),
            (86_400, "1970-01-02T00:00:00Z"),
            (951_782_400, "2000-02-29T00:00:00Z"),
            (1_709_164_800, "2024-02-29T00:00:00Z"),
            (1_756_800_000, "2025-09-02T08:00:00Z"),
            // The day after 28 February in a non-leap year.
            (1_772_323_908, "2026-03-01T00:11:48Z"),
            (1_788_701_508, "2026-09-06T13:31:48Z"),
            (4_102_444_799, "2099-12-31T23:59:59Z"),
        ] {
            assert_eq!(rfc3339_utc(secs), expected, "at {secs}");
        }
    }

    #[test]
    fn a_date_is_the_prefix_of_its_instant() {
        for secs in [0_u64, 1_756_800_000, 1_788_701_508, 4_102_444_799] {
            let instant = rfc3339_utc(secs);
            assert_eq!(today_from(secs), instant[..10]);
            assert_eq!(instant.len(), 20);
        }
        assert_eq!(today_from(1_756_800_000), "2025-09-02");
    }

    #[test]
    fn a_compact_stamp_is_the_instant_without_separators() {
        for secs in [0_u64, 1_756_800_000, 1_788_701_508] {
            let expected: String = rfc3339_utc(secs)
                .chars()
                .filter(char::is_ascii_alphanumeric)
                .collect();
            assert_eq!(compact_stamp(secs), expected);
        }
        assert_eq!(compact_stamp(1_756_800_000), "20250902T080000Z");
    }

    #[test]
    fn every_day_of_a_leap_year_round_trips_through_the_calendar() {
        // 2024-01-01T00:00:00Z. Walking a whole leap year catches an
        // off-by-one in the month table that a handful of spot checks would
        // miss.
        let start = 1_704_067_200_u64;
        let lengths: [u64; 12] = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        let mut elapsed = 0_u64;
        for (month, length) in (1_u64..=12).zip(lengths) {
            for day_of_month in 1..=length {
                assert_eq!(
                    civil_from_days((start + elapsed * DAY) / DAY),
                    (2024, month, day_of_month),
                    "{elapsed} days into 2024"
                );
                elapsed += 1;
            }
        }
        assert_eq!(elapsed, 366, "2024 is a leap year");
        assert_eq!(civil_from_days((start + elapsed * DAY) / DAY), (2025, 1, 1));
    }

    #[test]
    fn seconds_within_a_day_split_correctly() {
        assert_eq!(clock_from_seconds(0), (0, 0, 0));
        assert_eq!(clock_from_seconds(3_599), (0, 59, 59));
        assert_eq!(clock_from_seconds(3_600), (1, 0, 0));
        assert_eq!(clock_from_seconds(86_399), (23, 59, 59));
    }

    #[test]
    fn the_only_clock_read_answers_with_a_plausible_instant() {
        // Not an assertion about *when* the test runs: only that the one
        // clock read in the workspace produces a formattable instant in this
        // century, so a wired-up snapshot carries a usable timestamp.
        let stamp = rfc3339_utc(now_secs());
        assert_eq!(stamp.len(), 20);
        assert!(stamp.starts_with("20"), "{stamp}");
        assert!(stamp.ends_with('Z'));
    }
}
