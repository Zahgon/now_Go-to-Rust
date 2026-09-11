//! Rendering a [`Time`] through a Go reference layout — a transcription of
//! Go's `Time.Format`.

use super::layout::{next_std_chunk, Std};
use super::{Time, LONG_DAY_NAMES, LONG_MONTH_NAMES, SHORT_DAY_NAMES, SHORT_MONTH_NAMES};

/// Go's `appendInt`: a decimal integer, sign-prefixed when negative and
/// zero-padded to at least `width` digits.
fn append_int(out: &mut String, x: i64, width: usize) {
    let (neg, u) = if x < 0 { (true, x.unsigned_abs()) } else { (false, x as u64) };
    let digits = u.to_string();
    if neg {
        out.push('-');
    }
    for _ in digits.len()..width {
        out.push('0');
    }
    out.push_str(&digits);
}

/// Go's `appendNano`.
///
/// The `.000` form emits exactly `digits` digits. The `.999` form drops
/// trailing zeros and, if nothing is left, the separator too; a zero fraction
/// disappears entirely.
fn append_nano(out: &mut String, nanosec: u32, digits: usize, comma: bool, trim: bool) {
    if trim && (digits == 0 || nanosec == 0) {
        return;
    }
    let all = format!("{nanosec:09}");
    let n = digits.min(9);
    let mut frac = &all[..n];
    if trim {
        frac = frac.trim_end_matches('0');
        if frac.is_empty() {
            return;
        }
    }
    out.push(if comma { ',' } else { '.' });
    out.push_str(frac);
}

/// Renders the zone offset for the numeric-zone layout elements.
fn append_zone_offset(out: &mut String, std: Std, offset: i32) {
    // Go cheats and takes the `Z` variants to mean "as formatted for ISO 8601",
    // so a zero offset renders as a bare `Z`.
    let is_iso = matches!(
        std,
        Std::Iso8601Tz
            | Std::Iso8601ColonTz
            | Std::Iso8601ShortTz
            | Std::Iso8601SecondsTz
            | Std::Iso8601ColonSecondsTz
    );
    if offset == 0 && is_iso {
        out.push('Z');
        return;
    }
    let mut zone = offset / 60;
    let mut absoffset = offset;
    if zone < 0 {
        out.push('-');
        zone = -zone;
        absoffset = -absoffset;
    } else {
        out.push('+');
    }
    append_int(out, (zone / 60) as i64, 2);
    if matches!(
        std,
        Std::Iso8601ColonTz | Std::NumColonTz | Std::Iso8601ColonSecondsTz | Std::NumColonSecondsTz
    ) {
        out.push(':');
    }
    if !matches!(std, Std::NumShortTz | Std::Iso8601ShortTz) {
        append_int(out, (zone % 60) as i64, 2);
    }
    if matches!(
        std,
        Std::Iso8601SecondsTz
            | Std::NumSecondsTz
            | Std::NumColonSecondsTz
            | Std::Iso8601ColonSecondsTz
    ) {
        if matches!(std, Std::NumColonSecondsTz | Std::Iso8601ColonSecondsTz) {
            out.push(':');
        }
        append_int(out, (absoffset % 60) as i64, 2);
    }
}

impl Time {
    /// Go's `Time.Format`.
    pub fn format(&self, layout: &str) -> String {
        let (year, month, day) = self.date();
        let (hour, min, sec) = self.clock();
        let (zone_name, zone_offset) = self.zone();
        let weekday = self.weekday();

        let mut out = String::with_capacity(layout.len() + 10);
        let mut rest = layout;
        loop {
            let (prefix, std, suffix) = next_std_chunk(rest);
            out.push_str(prefix);
            let Some(std) = std else { break };
            rest = suffix;
            match std {
                Std::Year => {
                    let y = if year < 0 { -year } else { year };
                    append_int(&mut out, y % 100, 2);
                }
                Std::LongYear => append_int(&mut out, year, 4),
                Std::Month => out.push_str(SHORT_MONTH_NAMES[(month - 1) as usize]),
                Std::LongMonth => out.push_str(LONG_MONTH_NAMES[(month - 1) as usize]),
                Std::NumMonth => append_int(&mut out, month, 0),
                Std::ZeroMonth => append_int(&mut out, month, 2),
                Std::WeekDay => out.push_str(SHORT_DAY_NAMES[weekday as usize]),
                Std::LongWeekDay => out.push_str(LONG_DAY_NAMES[weekday as usize]),
                Std::Day => append_int(&mut out, day, 0),
                Std::UnderDay => {
                    if day < 10 {
                        out.push(' ');
                    }
                    append_int(&mut out, day, 0);
                }
                Std::ZeroDay => append_int(&mut out, day, 2),
                Std::UnderYearDay => {
                    let yday = self.year_day();
                    if yday < 100 {
                        out.push(' ');
                        if yday < 10 {
                            out.push(' ');
                        }
                    }
                    append_int(&mut out, yday, 0);
                }
                Std::ZeroYearDay => append_int(&mut out, self.year_day(), 3),
                Std::Hour => append_int(&mut out, hour, 2),
                Std::Hour12 => {
                    let mut h = hour % 12;
                    if h == 0 {
                        h = 12;
                    }
                    append_int(&mut out, h, 0);
                }
                Std::ZeroHour12 => {
                    let mut h = hour % 12;
                    if h == 0 {
                        h = 12;
                    }
                    append_int(&mut out, h, 2);
                }
                Std::Minute => append_int(&mut out, min, 0),
                Std::ZeroMinute => append_int(&mut out, min, 2),
                Std::Second => append_int(&mut out, sec, 0),
                Std::ZeroSecond => append_int(&mut out, sec, 2),
                Std::Pm => out.push_str(if hour >= 12 { "PM" } else { "AM" }),
                Std::LowerPm => out.push_str(if hour >= 12 { "pm" } else { "am" }),
                Std::Tz => {
                    if !zone_name.is_empty() {
                        out.push_str(&zone_name);
                    } else {
                        // No zone name known, but one must be printed: Go falls
                        // back to the `-0700` form.
                        let mut zone = zone_offset / 60;
                        if zone < 0 {
                            out.push('-');
                            zone = -zone;
                        } else {
                            out.push('+');
                        }
                        append_int(&mut out, (zone / 60) as i64, 2);
                        append_int(&mut out, (zone % 60) as i64, 2);
                    }
                }
                std if std.is_numeric_zone() => {
                    append_zone_offset(&mut out, std, zone_offset)
                }
                Std::FracSecond0 { digits, comma } => {
                    append_nano(&mut out, self.nanosecond(), digits, comma, false)
                }
                Std::FracSecond9 { digits, comma } => {
                    append_nano(&mut out, self.nanosecond(), digits, comma, true)
                }
                _ => unreachable!("every layout element is handled"),
            }
        }
        out
    }
}
