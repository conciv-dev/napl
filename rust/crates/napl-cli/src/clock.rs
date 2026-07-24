//! The clock seam: `now()` reads `NAPL_FIXED_NOW` at the CLI entry, falling
//! back to the real UTC time in ISO-8601 with milliseconds.

use std::time::{SystemTime, UNIX_EPOCH};

/// The current timestamp: `NAPL_FIXED_NOW` when set, else real UTC.
#[must_use]
pub fn now() -> String {
    if let Ok(fixed) = std::env::var("NAPL_FIXED_NOW") {
        return fixed;
    }
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    iso_from_millis(duration.as_millis() as u64)
}

fn iso_from_millis(millis: u64) -> String {
    let secs = millis / 1000;
    let ms = millis % 1000;
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (hour, minute, second) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (year, month, day) = civil_from_days(i64::try_from(days).unwrap_or_default());
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{ms:03}Z")
}

// Howard Hinnant's days-from-civil, inverted.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = if m <= 2 { y + 1 } else { y };
    (year, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_is_unix_zero() {
        assert_eq!(iso_from_millis(0), "1970-01-01T00:00:00.000Z");
    }

    #[test]
    fn known_timestamp_round_trips() {
        // 2026-07-23T00:00:00.000Z == 1784764800 seconds since epoch.
        assert_eq!(
            iso_from_millis(1_784_764_800_000),
            "2026-07-23T00:00:00.000Z"
        );
    }

    #[test]
    fn millis_component_is_kept() {
        assert_eq!(iso_from_millis(1_234), "1970-01-01T00:00:01.234Z");
    }
}
