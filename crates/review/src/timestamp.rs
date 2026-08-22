//! A moment in UTC, written the one way every record writes it.
//!
//! The core never asks what time it is. A timestamp arrives as a parameter,
//! already made by the boundary, and this type only knows how to read and
//! write the `2026-08-21T21:40:01Z` shape the wire format uses.

use std::fmt;

/// Seconds in a day.
const DAY: i64 = 86_400;

/// A UTC instant with second precision, as it appears on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp {
    unix: i64,
}

/// Why a timestamp could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseTimestampError(String);

impl fmt::Display for ParseTimestampError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "not a timestamp of the form 2026-08-21T21:40:01Z: {}",
            self.0
        )
    }
}

impl std::error::Error for ParseTimestampError {}

impl Timestamp {
    /// The instant that many seconds after 1970-01-01T00:00:00Z.
    #[must_use]
    pub const fn from_unix(unix: i64) -> Self {
        Self { unix }
    }

    /// Seconds since 1970-01-01T00:00:00Z.
    #[must_use]
    pub const fn unix(self) -> i64 {
        self.unix
    }

    /// Whole seconds from `earlier` to this instant, negative if it came first.
    #[must_use]
    pub const fn seconds_since(self, earlier: Self) -> i64 {
        self.unix - earlier.unix
    }

    /// Read `YYYY-MM-DDTHH:MM:SSZ`, and nothing looser.
    ///
    /// # Errors
    ///
    /// Anything that is not exactly that shape with a real calendar date.
    pub fn parse(text: &str) -> Result<Self, ParseTimestampError> {
        let fail = || ParseTimestampError(text.to_owned());
        let bytes = text.as_bytes();
        if bytes.len() != 20
            || bytes[4] != b'-'
            || bytes[7] != b'-'
            || bytes[10] != b'T'
            || bytes[13] != b':'
            || bytes[16] != b':'
            || bytes[19] != b'Z'
        {
            return Err(fail());
        }
        let field = |range: std::ops::Range<usize>| -> Result<i64, ParseTimestampError> {
            let digits = text.get(range).ok_or_else(fail)?;
            if !digits.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(fail());
            }
            digits.parse().map_err(|_| fail())
        };
        let (year, month, day) = (field(0..4)?, field(5..7)?, field(8..10)?);
        let (hour, minute, second) = (field(11..13)?, field(14..16)?, field(17..19)?);
        if !(1..=12).contains(&month)
            || day < 1
            || day > days_in_month(year, month)
            || hour > 23
            || minute > 59
            || second > 59
        {
            return Err(fail());
        }
        let days = days_from_civil(year, month, day);
        Ok(Self {
            unix: days * DAY + hour * 3600 + minute * 60 + second,
        })
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let days = self.unix.div_euclid(DAY);
        let seconds = self.unix.rem_euclid(DAY);
        let (year, month, day) = civil_from_days(days);
        write!(
            f,
            "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
            seconds / 3600,
            seconds % 3600 / 60,
            seconds % 60
        )
    }
}

fn is_leap(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days since 1970-01-01 for a proleptic Gregorian date. Howard Hinnant's algorithm.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_index = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The inverse of [`days_from_civil`].
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_the_wire_shape() -> Result<(), ParseTimestampError> {
        for text in [
            "2026-08-21T21:40:01Z",
            "1970-01-01T00:00:00Z",
            "2000-02-29T23:59:59Z",
            "2024-12-31T00:00:00Z",
        ] {
            assert_eq!(Timestamp::parse(text)?.to_string(), text);
        }
        Ok(())
    }

    #[test]
    fn counts_from_the_epoch() -> Result<(), ParseTimestampError> {
        assert_eq!(Timestamp::parse("1970-01-02T00:00:01Z")?.unix(), DAY + 1);
        assert_eq!(
            Timestamp::from_unix(1_755_812_401).to_string(),
            "2025-08-21T21:40:01Z"
        );
        Ok(())
    }

    #[test]
    fn orders_chronologically() -> Result<(), ParseTimestampError> {
        let earlier = Timestamp::parse("2026-08-21T21:40:01Z")?;
        let later = Timestamp::parse("2026-08-21T21:40:02Z")?;
        assert!(earlier < later);
        assert_eq!(later.seconds_since(earlier), 1);
        Ok(())
    }

    #[test]
    fn refuses_anything_looser() {
        for text in [
            "2026-08-21T21:40:01",
            "2026-08-21 21:40:01Z",
            "2026-8-21T21:40:01Z",
            "2026-13-01T00:00:00Z",
            "2026-02-30T00:00:00Z",
            "2026-08-21T24:00:00Z",
            "",
            "2026-08-21T21:40:01+00:00",
        ] {
            assert!(Timestamp::parse(text).is_err(), "{text} should not parse");
        }
    }
}
