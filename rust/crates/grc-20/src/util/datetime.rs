//! RFC 3339 date/time parsing and formatting utilities.
//!
//! Converts between RFC 3339 formatted strings and GRC-20 internal representations:
//! - Date: days since Unix epoch (1970-01-01) + offset in minutes
//! - Time: microseconds since midnight (`time_micros`) + offset in minutes
//! - Datetime: microseconds since Unix epoch (`epoch_micros`) + offset in minutes


const MICROSECONDS_PER_SECOND: i64 = 1_000_000;
const MICROSECONDS_PER_MINUTE: i64 = 60 * MICROSECONDS_PER_SECOND;
const MICROSECONDS_PER_HOUR: i64 = 60 * MICROSECONDS_PER_MINUTE;
const MILLISECONDS_PER_DAY: i64 = 24 * 60 * 60 * 1000;

/// Error type for RFC 3339 parsing failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateTimeParseError {
    pub message: String,
}

impl std::fmt::Display for DateTimeParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for DateTimeParseError {}

/// Parses a timezone offset string (Z, +HH:MM, -HH:MM) and returns offset in minutes.
fn parse_timezone_offset(offset: &str) -> Result<i16, DateTimeParseError> {
    if offset == "Z" || offset == "z" {
        return Ok(0);
    }

    if offset.len() != 6 {
        return Err(DateTimeParseError {
            message: format!("Invalid timezone offset: {}", offset),
        });
    }

    let sign = match offset.chars().next() {
        Some('+') => 1i16,
        Some('-') => -1i16,
        _ => {
            return Err(DateTimeParseError {
                message: format!("Invalid timezone offset: {}", offset),
            })
        }
    };

    if offset.chars().nth(3) != Some(':') {
        return Err(DateTimeParseError {
            message: format!("Invalid timezone offset: {}", offset),
        });
    }

    let hours: i16 = offset[1..3].parse().map_err(|_| DateTimeParseError {
        message: format!("Invalid timezone offset: {}", offset),
    })?;

    let minutes: i16 = offset[4..6].parse().map_err(|_| DateTimeParseError {
        message: format!("Invalid timezone offset: {}", offset),
    })?;

    // Validate hours and minutes (allow 24:00 as special case for ±24:00)
    if hours > 24 || (hours == 24 && minutes != 0) || minutes > 59 {
        return Err(DateTimeParseError {
            message: format!("Invalid timezone offset: {}", offset),
        });
    }

    let total_minutes = sign * (hours * 60 + minutes);
    if total_minutes < -1440 || total_minutes > 1440 {
        return Err(DateTimeParseError {
            message: format!("Timezone offset out of range [-24:00, +24:00]: {}", offset),
        });
    }

    Ok(total_minutes)
}

/// Formats an offset in minutes as a timezone string (Z, +HH:MM, -HH:MM).
fn format_timezone_offset(offset_min: i16) -> String {
    if offset_min == 0 {
        return "Z".to_string();
    }

    let sign = if offset_min >= 0 { '+' } else { '-' };
    let abs_offset = offset_min.abs();
    let hours = abs_offset / 60;
    let minutes = abs_offset % 60;

    format!("{}{:02}:{:02}", sign, hours, minutes)
}

/// Parses fractional seconds string and returns microseconds.
fn parse_fractional_seconds(frac: Option<&str>) -> i64 {
    match frac {
        None => 0,
        Some(s) if s.is_empty() => 0,
        Some(s) => {
            // Pad or truncate to 6 digits (microseconds)
            let mut padded = s.to_string();
            while padded.len() < 6 {
                padded.push('0');
            }
            padded.truncate(6);
            padded.parse().unwrap_or(0)
        }
    }
}

/// Formats microseconds as fractional seconds string, omitting if zero.
fn format_fractional_seconds(us: i64) -> String {
    if us == 0 {
        return String::new();
    }

    // Convert to 6-digit string and trim trailing zeros
    let str = format!("{:06}", us);
    let trimmed = str.trim_end_matches('0');
    format!(".{}", trimmed)
}

/// Returns true if the given year is a leap year.
fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Returns the number of days in a given month (1-indexed).
fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// Calculates days since Unix epoch for a given date.
fn date_to_days(year: i32, month: u32, day: u32) -> i32 {
    // Use a well-known algorithm for converting dates to days since epoch
    // This is based on the algorithm from Howard Hinnant
    let y = if month <= 2 {
        year - 1
    } else {
        year
    } as i64;

    let m = if month <= 2 {
        month as i64 + 9
    } else {
        month as i64 - 3
    };

    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32; // year of era
    let doy = (153 * m as u32 + 2) / 5 + day - 1; // day of year
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // day of era

    (era * 146097 + doe as i64 - 719468) as i32
}

/// Converts days since Unix epoch to (year, month, day).
fn days_to_date(days: i32) -> (i32, u32, u32) {
    // Howard Hinnant's algorithm in reverse
    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32; // day of era
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // year of era
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year
    let mp = (5 * doy + 2) / 153; // month index
    let d = doy - (153 * mp + 2) / 5 + 1; // day
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // month

    let year = if m <= 2 { y + 1 } else { y } as i32;
    (year, m, d)
}

// =====================
// DATE functions
// =====================

/// Parses an RFC 3339 date string (YYYY-MM-DD with optional timezone) and returns
/// days since Unix epoch and offset in minutes.
pub fn parse_date_rfc3339(date_str: &str) -> Result<(i32, i16), DateTimeParseError> {
    // Match YYYY-MM-DD with optional timezone offset
    let (date_part, offset_str) = if date_str.len() >= 10 {
        let date = &date_str[..10];
        let rest = &date_str[10..];
        if rest.is_empty() {
            (date, None)
        } else {
            (date, Some(rest))
        }
    } else {
        return Err(DateTimeParseError {
            message: format!("Invalid RFC 3339 date: {}", date_str),
        });
    };

    // Validate format: YYYY-MM-DD
    if date_part.len() != 10
        || date_part.chars().nth(4) != Some('-')
        || date_part.chars().nth(7) != Some('-')
    {
        return Err(DateTimeParseError {
            message: format!("Invalid RFC 3339 date: {}", date_str),
        });
    }

    let year: i32 = date_part[..4].parse().map_err(|_| DateTimeParseError {
        message: format!("Invalid year in date: {}", date_str),
    })?;

    let month: u32 = date_part[5..7].parse().map_err(|_| DateTimeParseError {
        message: format!("Invalid month in date: {}", date_str),
    })?;

    let day: u32 = date_part[8..10].parse().map_err(|_| DateTimeParseError {
        message: format!("Invalid day in date: {}", date_str),
    })?;

    // Validate month and day
    if month < 1 || month > 12 {
        return Err(DateTimeParseError {
            message: format!("Invalid month in date: {}", date_str),
        });
    }
    if day < 1 || day > days_in_month(year, month) {
        return Err(DateTimeParseError {
            message: format!("Invalid day in date: {}", date_str),
        });
    }

    let days = date_to_days(year, month, day);
    let offset_min = match offset_str {
        Some(s) => parse_timezone_offset(s)?,
        None => 0,
    };

    Ok((days, offset_min))
}

/// Formats days since Unix epoch as RFC 3339 date string.
pub fn format_date_rfc3339(days: i32, offset_min: i16) -> String {
    let (year, month, day) = days_to_date(days);
    let offset = format_timezone_offset(offset_min);
    format!("{:04}-{:02}-{:02}{}", year, month, day, offset)
}

// =====================
// TIME functions
// =====================

/// Parses an RFC 3339 time string (HH:MM:SS[.ssssss][Z|+HH:MM]) and returns
/// microseconds since midnight and offset in minutes.
pub fn parse_time_rfc3339(time_str: &str) -> Result<(i64, i16), DateTimeParseError> {
    // Minimum length is 8 (HH:MM:SS)
    if time_str.len() < 8 {
        return Err(DateTimeParseError {
            message: format!("Invalid RFC 3339 time: {}", time_str),
        });
    }

    // Validate basic format
    if time_str.chars().nth(2) != Some(':') || time_str.chars().nth(5) != Some(':') {
        return Err(DateTimeParseError {
            message: format!("Invalid RFC 3339 time: {}", time_str),
        });
    }

    let hours: i64 = time_str[..2].parse().map_err(|_| DateTimeParseError {
        message: format!("Invalid hours in time: {}", time_str),
    })?;

    let minutes: i64 = time_str[3..5].parse().map_err(|_| DateTimeParseError {
        message: format!("Invalid minutes in time: {}", time_str),
    })?;

    let seconds: i64 = time_str[6..8].parse().map_err(|_| DateTimeParseError {
        message: format!("Invalid seconds in time: {}", time_str),
    })?;

    // Validate ranges
    if hours > 23 {
        return Err(DateTimeParseError {
            message: format!("Invalid hours in time: {}", time_str),
        });
    }
    if minutes > 59 {
        return Err(DateTimeParseError {
            message: format!("Invalid minutes in time: {}", time_str),
        });
    }
    if seconds > 59 {
        return Err(DateTimeParseError {
            message: format!("Invalid seconds in time: {}", time_str),
        });
    }

    // Parse optional fractional seconds and timezone
    let rest = &time_str[8..];
    let (fractional, offset_str) = if rest.starts_with('.') {
        // Find where fractional seconds end
        let frac_end = rest[1..]
            .find(|c: char| !c.is_ascii_digit())
            .map(|i| i + 1)
            .unwrap_or(rest.len());

        let frac = &rest[1..frac_end];
        let tz = if frac_end < rest.len() {
            Some(&rest[frac_end..])
        } else {
            None
        };
        (Some(frac), tz)
    } else if rest.is_empty() {
        (None, None)
    } else {
        (None, Some(rest))
    };

    let microseconds = parse_fractional_seconds(fractional);
    let time_micros = hours * MICROSECONDS_PER_HOUR
        + minutes * MICROSECONDS_PER_MINUTE
        + seconds * MICROSECONDS_PER_SECOND
        + microseconds;

    // Validate total is within day
    if time_micros > 86_399_999_999 {
        return Err(DateTimeParseError {
            message: format!("Time exceeds maximum (23:59:59.999999): {}", time_str),
        });
    }

    let offset_min = match offset_str {
        Some(s) => parse_timezone_offset(s)?,
        None => 0,
    };

    Ok((time_micros, offset_min))
}

/// Formats microseconds since midnight as RFC 3339 time string.
pub fn format_time_rfc3339(time_micros: i64, offset_min: i16) -> String {
    let hours = time_micros / MICROSECONDS_PER_HOUR;
    let remaining1 = time_micros % MICROSECONDS_PER_HOUR;
    let minutes = remaining1 / MICROSECONDS_PER_MINUTE;
    let remaining2 = remaining1 % MICROSECONDS_PER_MINUTE;
    let seconds = remaining2 / MICROSECONDS_PER_SECOND;
    let microseconds = remaining2 % MICROSECONDS_PER_SECOND;

    let frac = format_fractional_seconds(microseconds);
    let offset = format_timezone_offset(offset_min);

    format!("{:02}:{:02}:{:02}{}{}", hours, minutes, seconds, frac, offset)
}

// =====================
// DATETIME functions
// =====================

/// Parses an RFC 3339 datetime string and returns microseconds since Unix epoch
/// and offset in minutes.
pub fn parse_datetime_rfc3339(datetime_str: &str) -> Result<(i64, i16), DateTimeParseError> {
    // Minimum length is 19 (YYYY-MM-DDTHH:MM:SS)
    if datetime_str.len() < 19 {
        return Err(DateTimeParseError {
            message: format!("Invalid RFC 3339 datetime: {}", datetime_str),
        });
    }

    // Check for T or space separator
    let sep = datetime_str.chars().nth(10);
    if sep != Some('T') && sep != Some(' ') {
        return Err(DateTimeParseError {
            message: format!("Invalid RFC 3339 datetime: {}", datetime_str),
        });
    }

    // Parse date part
    let date_part = &datetime_str[..10];
    if date_part.chars().nth(4) != Some('-') || date_part.chars().nth(7) != Some('-') {
        return Err(DateTimeParseError {
            message: format!("Invalid RFC 3339 datetime: {}", datetime_str),
        });
    }

    let year: i32 = date_part[..4].parse().map_err(|_| DateTimeParseError {
        message: format!("Invalid year in datetime: {}", datetime_str),
    })?;

    let month: u32 = date_part[5..7].parse().map_err(|_| DateTimeParseError {
        message: format!("Invalid month in datetime: {}", datetime_str),
    })?;

    let day: u32 = date_part[8..10].parse().map_err(|_| DateTimeParseError {
        message: format!("Invalid day in datetime: {}", datetime_str),
    })?;

    // Validate month and day
    if month < 1 || month > 12 {
        return Err(DateTimeParseError {
            message: format!("Invalid month in datetime: {}", datetime_str),
        });
    }
    if day < 1 || day > days_in_month(year, month) {
        return Err(DateTimeParseError {
            message: format!("Invalid day in datetime: {}", datetime_str),
        });
    }

    // Parse time part
    let time_part = &datetime_str[11..];
    if time_part.len() < 8
        || time_part.chars().nth(2) != Some(':')
        || time_part.chars().nth(5) != Some(':')
    {
        return Err(DateTimeParseError {
            message: format!("Invalid RFC 3339 datetime: {}", datetime_str),
        });
    }

    let hours: i64 = time_part[..2].parse().map_err(|_| DateTimeParseError {
        message: format!("Invalid hours in datetime: {}", datetime_str),
    })?;

    let minutes: i64 = time_part[3..5].parse().map_err(|_| DateTimeParseError {
        message: format!("Invalid minutes in datetime: {}", datetime_str),
    })?;

    let seconds: i64 = time_part[6..8].parse().map_err(|_| DateTimeParseError {
        message: format!("Invalid seconds in datetime: {}", datetime_str),
    })?;

    // Validate ranges
    if hours > 23 {
        return Err(DateTimeParseError {
            message: format!("Invalid hours in datetime: {}", datetime_str),
        });
    }
    if minutes > 59 {
        return Err(DateTimeParseError {
            message: format!("Invalid minutes in datetime: {}", datetime_str),
        });
    }
    if seconds > 59 {
        return Err(DateTimeParseError {
            message: format!("Invalid seconds in datetime: {}", datetime_str),
        });
    }

    // Parse optional fractional seconds and timezone
    let rest = &time_part[8..];
    let (fractional, offset_str) = if rest.starts_with('.') {
        // Find where fractional seconds end
        let frac_end = rest[1..]
            .find(|c: char| !c.is_ascii_digit())
            .map(|i| i + 1)
            .unwrap_or(rest.len());

        let frac = &rest[1..frac_end];
        let tz = if frac_end < rest.len() {
            Some(&rest[frac_end..])
        } else {
            None
        };
        (Some(frac), tz)
    } else if rest.is_empty() {
        (None, None)
    } else {
        (None, Some(rest))
    };

    let offset_min = match offset_str {
        Some(s) => parse_timezone_offset(s)?,
        None => 0,
    };

    let microseconds = parse_fractional_seconds(fractional);

    // Calculate epoch microseconds
    // First, get days since epoch for the date
    let days = date_to_days(year, month, day) as i64;

    // Calculate epoch_micros for the local time components
    let epoch_micros_utc = days * MILLISECONDS_PER_DAY * 1000
        + hours * MICROSECONDS_PER_HOUR
        + minutes * MICROSECONDS_PER_MINUTE
        + seconds * MICROSECONDS_PER_SECOND
        + microseconds;

    // Adjust for timezone offset: local time = UTC + offset, so UTC = local - offset
    let offset_us = offset_min as i64 * MICROSECONDS_PER_MINUTE;
    let epoch_micros = epoch_micros_utc - offset_us;

    Ok((epoch_micros, offset_min))
}

/// Formats microseconds since Unix epoch as RFC 3339 datetime string.
pub fn format_datetime_rfc3339(epoch_micros: i64, offset_min: i16) -> String {
    // Adjust for timezone offset: local time = UTC + offset
    let offset_us = offset_min as i64 * MICROSECONDS_PER_MINUTE;
    let local_us = epoch_micros + offset_us;

    // Convert to days and time-of-day
    let us_per_day = MILLISECONDS_PER_DAY * 1000;

    // Handle negative microseconds (before epoch)
    let (days, time_micros) = if local_us >= 0 {
        let days = (local_us / us_per_day) as i32;
        let time_micros = local_us % us_per_day;
        (days, time_micros)
    } else {
        // For negative values, we need to adjust
        let days = ((local_us + 1) / us_per_day - 1) as i32;
        let time_micros = ((local_us % us_per_day) + us_per_day) % us_per_day;
        (days, time_micros)
    };

    let (year, month, day) = days_to_date(days);

    let hours = time_micros / MICROSECONDS_PER_HOUR;
    let remaining1 = time_micros % MICROSECONDS_PER_HOUR;
    let minutes = remaining1 / MICROSECONDS_PER_MINUTE;
    let remaining2 = remaining1 % MICROSECONDS_PER_MINUTE;
    let seconds = remaining2 / MICROSECONDS_PER_SECOND;
    let microseconds = remaining2 % MICROSECONDS_PER_SECOND;

    let frac = format_fractional_seconds(microseconds);
    let offset = format_timezone_offset(offset_min);

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{}{}",
        year, month, day, hours, minutes, seconds, frac, offset
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_date_basic() {
        let (days, offset) = parse_date_rfc3339("1970-01-01").unwrap();
        assert_eq!(days, 0);
        assert_eq!(offset, 0);

        let (days, offset) = parse_date_rfc3339("1970-01-01Z").unwrap();
        assert_eq!(days, 0);
        assert_eq!(offset, 0);

        let (days, offset) = parse_date_rfc3339("2024-03-15").unwrap();
        assert_eq!(days, 19797);
        assert_eq!(offset, 0);

        let (days, offset) = parse_date_rfc3339("2024-03-15+05:30").unwrap();
        assert_eq!(days, 19797);
        assert_eq!(offset, 330);
    }

    #[test]
    fn test_format_date() {
        assert_eq!(format_date_rfc3339(0, 0), "1970-01-01Z");
        assert_eq!(format_date_rfc3339(19797, 0), "2024-03-15Z");
        assert_eq!(format_date_rfc3339(19797, 330), "2024-03-15+05:30");
        assert_eq!(format_date_rfc3339(19797, -300), "2024-03-15-05:00");
    }

    #[test]
    fn test_date_roundtrip() {
        let dates = [
            "1970-01-01Z",
            "2024-03-15Z",
            "2024-03-15+05:30",
            "2024-12-31-08:00",
            "2000-02-29Z", // leap year
        ];

        for date in dates {
            let (days, offset) = parse_date_rfc3339(date).unwrap();
            let formatted = format_date_rfc3339(days, offset);
            assert_eq!(date, formatted, "Roundtrip failed for {}", date);
        }
    }

    #[test]
    fn test_parse_time_basic() {
        let (time_micros, offset) = parse_time_rfc3339("00:00:00").unwrap();
        assert_eq!(time_micros, 0);
        assert_eq!(offset, 0);

        let (time_micros, offset) = parse_time_rfc3339("14:30:00Z").unwrap();
        assert_eq!(time_micros, 52_200_000_000);
        assert_eq!(offset, 0);

        let (time_micros, offset) = parse_time_rfc3339("14:30:00.5Z").unwrap();
        assert_eq!(time_micros, 52_200_500_000);
        assert_eq!(offset, 0);

        let (time_micros, offset) = parse_time_rfc3339("14:30:00.123456+05:30").unwrap();
        assert_eq!(time_micros, 52_200_123_456);
        assert_eq!(offset, 330);
    }

    #[test]
    fn test_format_time() {
        assert_eq!(format_time_rfc3339(0, 0), "00:00:00Z");
        assert_eq!(format_time_rfc3339(52_200_000_000, 0), "14:30:00Z");
        assert_eq!(format_time_rfc3339(52_200_500_000, 0), "14:30:00.5Z");
        assert_eq!(format_time_rfc3339(52_200_123_456, 330), "14:30:00.123456+05:30");
    }

    #[test]
    fn test_time_roundtrip() {
        let times = [
            "00:00:00Z",
            "14:30:00Z",
            "14:30:00.5Z",
            "14:30:00.123456Z",
            "23:59:59.999999Z",
            "14:30:00+05:30",
            "14:30:00-08:00",
        ];

        for time in times {
            let (time_micros, offset) = parse_time_rfc3339(time).unwrap();
            let formatted = format_time_rfc3339(time_micros, offset);
            assert_eq!(time, formatted, "Roundtrip failed for {}", time);
        }
    }

    #[test]
    fn test_parse_datetime_basic() {
        let (epoch_micros, offset) = parse_datetime_rfc3339("1970-01-01T00:00:00Z").unwrap();
        assert_eq!(epoch_micros, 0);
        assert_eq!(offset, 0);

        let (epoch_micros, offset) = parse_datetime_rfc3339("2024-03-15T14:30:00Z").unwrap();
        assert_eq!(epoch_micros, 1710513000000000);
        assert_eq!(offset, 0);

        let (epoch_micros, offset) = parse_datetime_rfc3339("2024-03-15T14:30:00.123456Z").unwrap();
        assert_eq!(epoch_micros, 1710513000123456);
        assert_eq!(offset, 0);
    }

    #[test]
    fn test_format_datetime() {
        assert_eq!(format_datetime_rfc3339(0, 0), "1970-01-01T00:00:00Z");
        assert_eq!(
            format_datetime_rfc3339(1710513000000000, 0),
            "2024-03-15T14:30:00Z"
        );
        assert_eq!(
            format_datetime_rfc3339(1710513000123456, 0),
            "2024-03-15T14:30:00.123456Z"
        );
    }

    #[test]
    fn test_datetime_roundtrip() {
        let datetimes = [
            "1970-01-01T00:00:00Z",
            "2024-03-15T14:30:00Z",
            "2024-03-15T14:30:00.5Z",
            "2024-03-15T14:30:00.123456Z",
            "2024-12-31T23:59:59.999999Z",
        ];

        for datetime in datetimes {
            let (epoch_micros, offset) = parse_datetime_rfc3339(datetime).unwrap();
            let formatted = format_datetime_rfc3339(epoch_micros, offset);
            assert_eq!(datetime, formatted, "Roundtrip failed for {}", datetime);
        }
    }

    #[test]
    fn test_datetime_with_offset() {
        // 2024-03-15T14:30:00+05:30 should be 2024-03-15T09:00:00Z
        let (epoch_micros, offset) = parse_datetime_rfc3339("2024-03-15T14:30:00+05:30").unwrap();
        assert_eq!(offset, 330);
        // The epoch_micros should be 5.5 hours less than 2024-03-15T14:30:00Z
        let (utc_epoch_micros, _) = parse_datetime_rfc3339("2024-03-15T09:00:00Z").unwrap();
        assert_eq!(epoch_micros, utc_epoch_micros);

        // Formatting should preserve the offset
        let formatted = format_datetime_rfc3339(epoch_micros, offset);
        assert_eq!(formatted, "2024-03-15T14:30:00+05:30");
    }

    #[test]
    fn test_negative_epoch() {
        // Before Unix epoch
        let (epoch_micros, offset) = parse_datetime_rfc3339("1969-12-31T23:59:59Z").unwrap();
        assert_eq!(epoch_micros, -1_000_000);
        assert_eq!(offset, 0);

        let formatted = format_datetime_rfc3339(epoch_micros, offset);
        assert_eq!(formatted, "1969-12-31T23:59:59Z");
    }

    #[test]
    fn test_invalid_dates() {
        assert!(parse_date_rfc3339("2024-13-01").is_err()); // invalid month
        assert!(parse_date_rfc3339("2024-00-01").is_err()); // invalid month
        assert!(parse_date_rfc3339("2024-02-30").is_err()); // invalid day
        assert!(parse_date_rfc3339("2023-02-29").is_err()); // not a leap year
        assert!(parse_date_rfc3339("not-a-date").is_err());
    }

    #[test]
    fn test_invalid_times() {
        assert!(parse_time_rfc3339("24:00:00").is_err()); // invalid hour
        assert!(parse_time_rfc3339("14:60:00").is_err()); // invalid minute
        assert!(parse_time_rfc3339("14:30:60").is_err()); // invalid second
        assert!(parse_time_rfc3339("not:a:time").is_err());
    }

    #[test]
    fn test_timezone_offset_edge_cases() {
        assert!(parse_timezone_offset("+24:00").is_ok());
        assert!(parse_timezone_offset("-24:00").is_ok());
        assert!(parse_timezone_offset("+24:01").is_err()); // out of range
        assert!(parse_timezone_offset("-24:01").is_err()); // out of range
    }
}
