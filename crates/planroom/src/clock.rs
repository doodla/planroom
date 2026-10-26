//! The wall clock, as RFC 3339 UTC with microseconds.
//!
//! Hand-rolled: the civil-from-days conversion is a dozen lines, where a date crate would add a
//! dependency for one format.

use std::time::{SystemTime, UNIX_EPOCH};

use planroom_contract::Timestamp;

/// Microseconds since the Unix epoch.
pub fn unix_micros() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_micros() as u64)
}

/// Now, as `2026-10-26T06:57:01.123456Z`.
pub fn now() -> Timestamp {
    Timestamp(rfc3339(unix_micros()))
}

/// `micros` since the epoch as RFC 3339 UTC.
pub fn rfc3339(micros: u64) -> String {
    let secs = micros / 1_000_000;
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (y, m, d) = civil_from_days(days as i64);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{:06}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60,
        micros % 1_000_000
    )
}

/// Howard Hinnant's `civil_from_days`: days since 1970-01-01 to (year, month, day).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_instants_format_exactly() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00.000000Z");
        assert_eq!(rfc3339(951_782_400_000_001), "2000-02-29T00:00:00.000001Z");
        assert_eq!(rfc3339(1_793_000_000_123_456), "2026-10-26T07:33:20.123456Z");
    }
}
