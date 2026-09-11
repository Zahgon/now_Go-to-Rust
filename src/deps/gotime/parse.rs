//! Parsing a value against a Go reference layout — a transcription of Go's
//! `time.Parse` / `time.ParseInLocation`.
//!
//! The details that matter to `now` and are easy to get wrong:
//!
//! * a non-padded numeric element consumes two digits when two are available
//!   and one otherwise, which is what lets `"2006-1-2"` parse `"2002-10-12"`;
//! * the layout must consume the whole value, which is what makes the ordered
//!   `TimeFormats` list resolve unambiguously;
//! * a value may carry a fractional second even when the layout has no
//!   fractional element, provided the layout has a seconds element.

use super::civil::{days_in, is_leap};
use super::layout::{next_std_chunk, Std};
use super::location::Location;
use super::{Time, LONG_DAY_NAMES, LONG_MONTH_NAMES, SHORT_DAY_NAMES, SHORT_MONTH_NAMES};
use std::fmt;

/// Go's `*time.ParseError`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub layout: String,
    pub value: String,
    pub layout_elem: String,
    pub value_elem: String,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.message.is_empty() {
            write!(
                f,
                "parsing time \"{}\" as \"{}\": cannot parse \"{}\" as \"{}\"",
                self.value, self.layout, self.value_elem, self.layout_elem
            )
        } else {
            write!(
                f,
                "parsing time \"{}\"{}",
                self.value, self.message
            )
        }
    }
}

impl std::error::Error for ParseError {}

/// Days before the first of each month in a non-leap year, 1-based on month.
const DAYS_BEFORE: [i64; 13] = [
    0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334, 365,
];

type Bad = ();

/// Go's `match`: byte-wise comparison that is case-insensitive for ASCII letters.
fn eq_fold(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).all(|(&c1, &c2)| {
        if c1 == c2 {
            return true;
        }
        let l1 = c1 | (b'a' - b'A');
        let l2 = c2 | (b'a' - b'A');
        l1 == l2 && l1.is_ascii_lowercase()
    })
}

/// Go's `lookup`: matches `val` against a table of names, case-insensitively,
/// returning the 1-based index and the unconsumed remainder.
fn lookup<'a>(tab: &[&str], val: &'a str) -> Result<(usize, &'a str), Bad> {
    for (i, v) in tab.iter().enumerate() {
        if val.len() >= v.len() && eq_fold(&val.as_bytes()[..v.len()], v.as_bytes()) {
            return Ok((i + 1, &val[v.len()..]));
        }
    }
    Err(())
}

fn is_digit(s: &str, i: usize) -> bool {
    s.as_bytes().get(i).is_some_and(u8::is_ascii_digit)
}

/// Go's `getnum`: one or two digits, or exactly two when `fixed`.
fn getnum(s: &str, fixed: bool) -> Result<(i64, &str), Bad> {
    if !is_digit(s, 0) {
        return Err(());
    }
    if !is_digit(s, 1) {
        if fixed {
            return Err(());
        }
        return Ok(((s.as_bytes()[0] - b'0') as i64, &s[1..]));
    }
    let b = s.as_bytes();
    Ok((
        ((b[0] - b'0') as i64) * 10 + (b[1] - b'0') as i64,
        &s[2..],
    ))
}

/// Go's `getnum3`: one to three digits, or exactly three when `fixed`.
fn getnum3(s: &str, fixed: bool) -> Result<(i64, &str), Bad> {
    let mut n = 0i64;
    let mut i = 0;
    while i < 3 && is_digit(s, i) {
        n = n * 10 + (s.as_bytes()[i] - b'0') as i64;
        i += 1;
    }
    if i == 0 || (fixed && i != 3) {
        return Err(());
    }
    Ok((n, &s[i..]))
}

fn atoi(s: &str) -> Result<i64, Bad> {
    let (neg, body) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    if body.is_empty() || !body.bytes().all(|c| c.is_ascii_digit()) {
        return Err(());
    }
    let v: i64 = body.parse().map_err(|_| ())?;
    Ok(if neg { -v } else { v })
}

fn cutspace(s: &str) -> &str {
    s.trim_start_matches(' ')
}

/// Go's `skip`: consumes a literal prefix, where a space in the layout matches
/// a run of one or more spaces in the value.
fn skip<'a>(value: &'a str, prefix: &str) -> Result<&'a str, Bad> {
    let mut value = value;
    let mut prefix = prefix;
    while !prefix.is_empty() {
        if prefix.starts_with(' ') {
            if !value.is_empty() && !value.starts_with(' ') {
                return Err(());
            }
            prefix = cutspace(prefix);
            value = cutspace(value);
            continue;
        }
        let pc = prefix.chars().next().expect("prefix is non-empty");
        if !value.starts_with(pc) {
            return Err(());
        }
        prefix = &prefix[pc.len_utf8()..];
        value = &value[pc.len_utf8()..];
    }
    Ok(value)
}

/// Go's `parseNanoseconds`: `value[..nbytes]` is a separator followed by digits.
fn parse_nanoseconds(value: &str, nbytes: usize) -> Result<i64, Bad> {
    if !matches!(value.as_bytes().first(), Some(b'.') | Some(b',')) {
        return Err(());
    }
    let (value, nbytes) = if nbytes > 10 { (&value[..10], 10) } else { (value, nbytes) };
    let mut ns = atoi(&value[1..nbytes])?;
    if ns < 0 {
        return Err(());
    }
    for _ in 0..(10 - nbytes) {
        ns *= 10;
    }
    Ok(ns)
}

/// Go's `leadingInt`: the longest leading run of digits.
fn leading_int(s: &str) -> Result<(i64, &str), Bad> {
    let mut x: i64 = 0;
    let mut i = 0;
    while is_digit(s, i) {
        x = x
            .checked_mul(10)
            .and_then(|v| v.checked_add((s.as_bytes()[i] - b'0') as i64))
            .ok_or(())?;
        i += 1;
    }
    Ok((x, &s[i..]))
}

/// Go's `parseSignedOffset`: the length of a leading `±h` / `±hh`.
fn parse_signed_offset(value: &str) -> usize {
    let Some(sign) = value.as_bytes().first() else {
        return 0;
    };
    if *sign != b'-' && *sign != b'+' {
        return 0;
    }
    let Ok((x, rem)) = leading_int(&value[1..]) else {
        return 0;
    };
    if rem.len() == value.len() - 1 || x > 23 {
        return 0;
    }
    value.len() - rem.len()
}

/// Go's `parseGMT`.
fn parse_gmt(value: &str) -> usize {
    let rest = &value[3..];
    if rest.is_empty() {
        return 3;
    }
    3 + parse_signed_offset(rest)
}

/// Go's `parseTimeZone`: the length of a leading zone abbreviation, if any.
fn parse_time_zone(value: &str) -> Option<usize> {
    if value.len() < 3 {
        return None;
    }
    // ChST and MeST are the only zones with a lower-case letter.
    if value.len() >= 4 && (&value[..4] == "ChST" || &value[..4] == "MeST") {
        return Some(4);
    }
    // GMT may carry an hour offset.
    if &value[..3] == "GMT" {
        return Some(parse_gmt(value));
    }
    // Some zones are unnamed and appear as ±hh.
    let b = value.as_bytes();
    if b[0] == b'+' || b[0] == b'-' {
        let n = parse_signed_offset(value);
        return if n > 0 { Some(n) } else { None };
    }
    // Otherwise: three to five upper-case letters, with the shape Go accepts.
    let mut n_upper = 0;
    while n_upper < 6 {
        match b.get(n_upper) {
            Some(c) if c.is_ascii_uppercase() => n_upper += 1,
            _ => break,
        }
    }
    match n_upper {
        5 if b[4] == b'T' => Some(5),
        4 if b[3] == b'T' || &value[..4] == "WITA" => Some(4),
        3 => Some(3),
        _ => None,
    }
}

fn err_at(layout: &str, value: &str, layout_elem: &str, value_elem: &str) -> ParseError {
    ParseError {
        layout: layout.to_string(),
        value: value.to_string(),
        layout_elem: layout_elem.to_string(),
        value_elem: value_elem.to_string(),
        message: String::new(),
    }
}

fn err_msg(layout: &str, value: &str, message: &str) -> ParseError {
    ParseError {
        layout: layout.to_string(),
        value: value.to_string(),
        layout_elem: String::new(),
        value_elem: String::new(),
        message: message.to_string(),
    }
}

/// Go's `time.ParseInLocation`.
pub fn parse_in_location(layout: &str, value: &str, loc: &Location) -> Result<Time, ParseError> {
    parse_impl(layout, value, loc, loc)
}

#[allow(clippy::cognitive_complexity)]
fn parse_impl(
    layout: &str,
    value: &str,
    default_location: &Location,
    local: &Location,
) -> Result<Time, ParseError> {
    let alayout = layout;
    let avalue = value;

    let mut year: i64 = 0;
    let mut month: i64 = -1;
    let mut day: i64 = -1;
    let mut yday: i64 = -1;
    let mut hour: i64 = 0;
    let mut min: i64 = 0;
    let mut sec: i64 = 0;
    let mut nsec: i64 = 0;
    let mut z: Option<Location> = None;
    let mut zone_offset: i64 = -1;
    let mut zone_name = String::new();
    let mut pm_set = false;
    let mut am_set = false;

    let mut layout = layout;
    let mut value = value;

    loop {
        let (prefix, std, suffix) = next_std_chunk(layout);
        let stdstr = &layout[prefix.len()..layout.len() - suffix.len()];
        value = skip(value, prefix).map_err(|_| err_at(alayout, avalue, prefix, value))?;
        let Some(std) = std else {
            if !value.is_empty() {
                return Err(err_msg(alayout, avalue, &format!(": extra text: \"{value}\"")));
            }
            break;
        };
        layout = suffix;

        // Go snapshots the value before the element consumes any of it, so a
        // failure reports the whole element rather than the tail left over
        // after a partial read.
        let hold = value;

        macro_rules! bail {
            () => {
                return Err(err_at(alayout, avalue, stdstr, hold))
            };
        }

        match std {
            Std::Year => {
                if value.len() < 2 || !is_digit(value, 0) || !is_digit(value, 1) {
                    bail!();
                }
                let p = &value[..2];
                value = &value[2..];
                let Ok(y) = atoi(p) else { bail!() };
                year = if y >= 69 { y + 1900 } else { y + 2000 };
            }
            Std::LongYear => {
                if value.len() < 4 || !is_digit(value, 0) {
                    bail!();
                }
                let p = &value[..4];
                value = &value[4..];
                let Ok(y) = atoi(p) else { bail!() };
                year = y;
            }
            Std::Month => match lookup(&SHORT_MONTH_NAMES, value) {
                Ok((m, rest)) => {
                    month = m as i64;
                    value = rest;
                }
                Err(_) => bail!(),
            },
            Std::LongMonth => match lookup(&LONG_MONTH_NAMES, value) {
                Ok((m, rest)) => {
                    month = m as i64;
                    value = rest;
                }
                Err(_) => bail!(),
            },
            Std::NumMonth | Std::ZeroMonth => {
                match getnum(value, std == Std::ZeroMonth) {
                    Ok((m, rest)) => {
                        month = m;
                        value = rest;
                    }
                    Err(_) => bail!(),
                }
                if !(1..=12).contains(&month) {
                    return Err(err_msg(alayout, avalue, ": month out of range"));
                }
            }
            // Go parses the weekday and discards it; it is never validated
            // against the date.
            Std::WeekDay => match lookup(&SHORT_DAY_NAMES, value) {
                Ok((_, rest)) => value = rest,
                Err(_) => bail!(),
            },
            Std::LongWeekDay => match lookup(&LONG_DAY_NAMES, value) {
                Ok((_, rest)) => value = rest,
                Err(_) => bail!(),
            },
            Std::Day | Std::UnderDay | Std::ZeroDay => {
                if std == Std::UnderDay && value.starts_with(' ') {
                    value = &value[1..];
                }
                match getnum(value, std == Std::ZeroDay) {
                    Ok((d, rest)) => {
                        day = d;
                        value = rest;
                    }
                    Err(_) => bail!(),
                }
                // The range check happens once the year and month are known.
            }
            Std::UnderYearDay | Std::ZeroYearDay => {
                if std == Std::UnderYearDay {
                    for _ in 0..2 {
                        if value.starts_with(' ') {
                            value = &value[1..];
                        }
                    }
                }
                match getnum3(value, std == Std::ZeroYearDay) {
                    Ok((d, rest)) => {
                        yday = d;
                        value = rest;
                    }
                    Err(_) => bail!(),
                }
            }
            Std::Hour => {
                match getnum(value, false) {
                    Ok((h, rest)) => {
                        hour = h;
                        value = rest;
                    }
                    Err(_) => bail!(),
                }
                if !(0..24).contains(&hour) {
                    return Err(err_msg(alayout, avalue, ": hour out of range"));
                }
            }
            Std::Hour12 | Std::ZeroHour12 => {
                match getnum(value, std == Std::ZeroHour12) {
                    Ok((h, rest)) => {
                        hour = h;
                        value = rest;
                    }
                    Err(_) => bail!(),
                }
                if !(0..=12).contains(&hour) {
                    return Err(err_msg(alayout, avalue, ": hour out of range"));
                }
            }
            Std::Minute | Std::ZeroMinute => {
                match getnum(value, std == Std::ZeroMinute) {
                    Ok((m, rest)) => {
                        min = m;
                        value = rest;
                    }
                    Err(_) => bail!(),
                }
                if !(0..60).contains(&min) {
                    return Err(err_msg(alayout, avalue, ": minute out of range"));
                }
            }
            Std::Second | Std::ZeroSecond => {
                match getnum(value, std == Std::ZeroSecond) {
                    Ok((s, rest)) => {
                        sec = s;
                        value = rest;
                    }
                    Err(_) => bail!(),
                }
                if !(0..60).contains(&sec) {
                    return Err(err_msg(alayout, avalue, ": second out of range"));
                }
                // A fractional second in the value but not in the layout is
                // still consumed, provided the layout does not go on to declare
                // one itself.
                if value.len() >= 2
                    && matches!(value.as_bytes()[0], b'.' | b',')
                    && is_digit(value, 1)
                {
                    let (_, next_std, _) = next_std_chunk(layout);
                    if !matches!(
                        next_std,
                        Some(Std::FracSecond0 { .. }) | Some(Std::FracSecond9 { .. })
                    ) {
                        let mut n = 2;
                        while n < value.len() && is_digit(value, n) {
                            n += 1;
                        }
                        match parse_nanoseconds(value, n) {
                            Ok(ns) => {
                                nsec = ns;
                                value = &value[n..];
                            }
                            Err(_) => {
                                return Err(err_msg(alayout, avalue, ": fractional second out of range"))
                            }
                        }
                    }
                }
            }
            Std::Pm => {
                if value.len() < 2 {
                    bail!();
                }
                let p = &value[..2];
                value = &value[2..];
                match p {
                    "PM" => pm_set = true,
                    "AM" => am_set = true,
                    _ => bail!(),
                }
            }
            Std::LowerPm => {
                if value.len() < 2 {
                    bail!();
                }
                let p = &value[..2];
                value = &value[2..];
                match p {
                    "pm" => pm_set = true,
                    "am" => am_set = true,
                    _ => bail!(),
                }
            }
            std if std.is_numeric_zone() => {
                // The three plain `Z` forms accept a literal `Z` for UTC.
                if matches!(std, Std::Iso8601Tz | Std::Iso8601ShortTz | Std::Iso8601ColonTz)
                    && value.starts_with('Z')
                {
                    value = &value[1..];
                    z = Some(Location::utc());
                    continue;
                }
                let b = value.as_bytes();
                let (sign, hh, mm, ss, rest) = match std {
                    Std::Iso8601ColonTz | Std::NumColonTz => {
                        if value.len() < 6 || b[3] != b':' {
                            bail!();
                        }
                        (&value[..1], &value[1..3], &value[4..6], "00", &value[6..])
                    }
                    Std::NumShortTz | Std::Iso8601ShortTz => {
                        if value.len() < 3 {
                            bail!();
                        }
                        (&value[..1], &value[1..3], "00", "00", &value[3..])
                    }
                    Std::Iso8601ColonSecondsTz | Std::NumColonSecondsTz => {
                        if value.len() < 9 || b[3] != b':' || b[6] != b':' {
                            bail!();
                        }
                        (&value[..1], &value[1..3], &value[4..6], &value[7..9], &value[9..])
                    }
                    Std::Iso8601SecondsTz | Std::NumSecondsTz => {
                        if value.len() < 7 {
                            bail!();
                        }
                        (&value[..1], &value[1..3], &value[3..5], &value[5..7], &value[7..])
                    }
                    _ => {
                        if value.len() < 5 {
                            bail!();
                        }
                        (&value[..1], &value[1..3], &value[3..5], "00", &value[5..])
                    }
                };
                value = rest;
                // Go reads the three fields in order and stops at the first
                // that fails, but still range-checks what it did read.
                let mut bad = false;
                let mut read = |field: &str| -> i64 {
                    if bad {
                        return 0;
                    }
                    match getnum(field, true) {
                        Ok((v, _)) => v,
                        Err(_) => {
                            bad = true;
                            0
                        }
                    }
                };
                let (hr, mmv, ssv) = (read(hh), read(mm), read(ss));
                // The range tests use `>` rather than `>=`, as some people do
                // write offsets of 24 hours or 60 minutes or 60 seconds.
                let mut range_err: Option<&str> = None;
                if hr > 24 {
                    range_err = Some("time zone offset hour");
                }
                if mmv > 60 {
                    range_err = Some("time zone offset minute");
                }
                if ssv > 60 {
                    range_err = Some("time zone offset second");
                }
                zone_offset = (hr * 60 + mmv) * 60 + ssv;
                match sign.as_bytes()[0] {
                    b'+' => {}
                    b'-' => zone_offset = -zone_offset,
                    _ => bad = true,
                }
                // A range error outranks a malformed-input error.
                if let Some(r) = range_err {
                    return Err(err_msg(alayout, avalue, &format!(": {r} out of range")));
                }
                if bad {
                    bail!();
                }
            }
            Std::Tz => {
                if value.len() >= 3 && &value[..3] == "UTC" {
                    z = Some(Location::utc());
                    value = &value[3..];
                    continue;
                }
                match parse_time_zone(value) {
                    Some(n) => {
                        zone_name = value[..n].to_string();
                        value = &value[n..];
                    }
                    None => bail!(),
                }
            }
            Std::FracSecond0 { digits, .. } => {
                // The fixed-width form requires exactly this many digits.
                let ndigit = 1 + digits;
                if value.len() < ndigit {
                    bail!();
                }
                match parse_nanoseconds(value, ndigit) {
                    Ok(ns) => {
                        nsec = ns;
                        value = &value[ndigit..];
                    }
                    Err(_) => bail!(),
                }
            }
            Std::FracSecond9 { .. } => {
                if value.len() < 2
                    || !matches!(value.as_bytes()[0], b'.' | b',')
                    || !is_digit(value, 1)
                {
                    // Fractional second omitted.
                    continue;
                }
                // Take any number of digits, even more than asked for.
                let mut i = 1;
                while i < value.len() - 1 && is_digit(value, i + 1) {
                    i += 1;
                }
                match parse_nanoseconds(value, 1 + i) {
                    Ok(ns) => {
                        nsec = ns;
                        value = &value[1 + i..];
                    }
                    Err(_) => {
                        return Err(err_msg(alayout, avalue, ": fractional second out of range"))
                    }
                }
            }
            _ => unreachable!("every layout element is handled"),
        }
    }

    if pm_set && hour < 12 {
        hour += 12;
    } else if am_set && hour == 12 {
        hour = 0;
    }

    if yday >= 0 {
        let mut m = 0i64;
        let mut d = 0i64;
        let mut yd = yday;
        if is_leap(year) {
            if yd == 31 + 29 {
                m = 2;
                d = 29;
            } else if yd > 31 + 29 {
                yd -= 1;
            }
        }
        if !(1..=365).contains(&yd) {
            return Err(err_msg(alayout, avalue, ": day-of-year out of range"));
        }
        if m == 0 {
            m = (yd - 1) / 31 + 1;
            if DAYS_BEFORE[m as usize] < yd {
                m += 1;
            }
            d = yd - DAYS_BEFORE[(m - 1) as usize];
        }
        if month >= 0 && month != m {
            return Err(err_msg(alayout, avalue, ": day-of-year does not match month"));
        }
        month = m;
        if day >= 0 && day != d {
            return Err(err_msg(alayout, avalue, ": day-of-year does not match day"));
        }
        day = d;
    } else {
        if month < 0 {
            month = 1;
        }
        if day < 0 {
            day = 1;
        }
    }

    if day < 1 || day > days_in(month, year) {
        return Err(err_msg(alayout, avalue, ": day out of range"));
    }

    if let Some(z) = z {
        return Ok(Time::new(year, month, day, hour, min, sec, nsec, &z));
    }

    if zone_offset != -1 {
        let t = Time::new(year, month, day, hour, min, sec, nsec, &Location::utc());
        let t = Time::from_unix(
            t.unix_sec() - zone_offset,
            t.nanosecond(),
            &Location::utc(),
        );
        // If the zone in effect locally at that instant has the same offset
        // (and name, when one was parsed), keep the location rather than
        // fabricating a fixed zone.
        let zi = local.lookup(t.unix_sec());
        if zi.offset as i64 == zone_offset && (zone_name.is_empty() || zi.name == zone_name) {
            return Ok(t.in_location(local));
        }
        return Ok(t.in_location(&Location::fixed(&zone_name, zone_offset as i32)));
    }

    if !zone_name.is_empty() {
        let t = Time::new(year, month, day, hour, min, sec, nsec, &Location::utc());
        if let Some(offset) = local.lookup_name(&zone_name, t.unix_sec()) {
            return Ok(Time::from_unix(
                t.unix_sec() - offset as i64,
                t.nanosecond(),
                &Location::utc(),
            )
            .in_location(local));
        }
        // Otherwise fabricate a zone with an unknown offset, except that a
        // `GMT±h` name carries its own.
        let mut offset = 0i32;
        if zone_name.len() > 3 && &zone_name[..3] == "GMT" {
            if let Ok(v) = atoi(&zone_name[3..]) {
                offset = (v * 3600) as i32;
            }
        }
        return Ok(t.in_location(&Location::fixed(&zone_name, offset)));
    }

    Ok(Time::new(year, month, day, hour, min, sec, nsec, default_location))
}
