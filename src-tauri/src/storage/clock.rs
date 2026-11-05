//! Time helpers. Timestamps are stored as UTC milliseconds since 1970 (plain integers): they sort
//! and index correctly, take 8 bytes, and have no time-zone or format ambiguity. The frontend turns
//! them into local dates for display.

use std::time::{SystemTime, UNIX_EPOCH};

/// Milliseconds since the Unix epoch. A clock set before 1970 gives 0 rather than a panic.
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

/// `YYYYMMDD-HHMMSS` in UTC, for file names. Uses the standard days-to-civil-date conversion so no
/// date library is needed.
pub fn file_stamp(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Days since 1970-01-01 to (year, month, day); valid for the whole proleptic Gregorian range.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}{m:02}{d:02}-{:02}{:02}{:02}",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_stamp_matches_known_dates() {
        assert_eq!(file_stamp(0), "19700101-000000");
        assert_eq!(file_stamp(951_782_400_000), "20000229-000000"); // leap day
        assert_eq!(file_stamp(1_700_000_000_000), "20231114-221320");
        assert_eq!(file_stamp(1_767_225_599_000), "20251231-235959");
    }

    #[test]
    fn now_is_after_2024() {
        assert!(now_ms() > 1_700_000_000_000);
    }
}
