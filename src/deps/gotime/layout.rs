//! The reference-layout scanner, a transcription of Go's `nextStdChunk`.
//!
//! Go layouts describe a format by writing out the reference time
//! `Mon Jan 2 15:04:05 MST 2006`. Scanning a layout means finding the next
//! recognised element and treating everything before it as a literal. The
//! element set and the order in which alternatives are tested are both
//! observable — `"2006-01-02T15:04:05Z0700"` and `"2006-01-02T15:04:05Z07:00"`
//! differ only in which `Z` alternative matches first — so this follows Go's
//! ordering exactly.

/// A recognised layout element.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Std {
    LongMonth,
    Month,
    NumMonth,
    ZeroMonth,
    LongWeekDay,
    WeekDay,
    Day,
    UnderDay,
    ZeroDay,
    UnderYearDay,
    ZeroYearDay,
    Hour,
    Hour12,
    ZeroHour12,
    Minute,
    ZeroMinute,
    Second,
    ZeroSecond,
    LongYear,
    Year,
    Pm,
    LowerPm,
    Tz,
    Iso8601Tz,
    Iso8601SecondsTz,
    Iso8601ShortTz,
    Iso8601ColonTz,
    Iso8601ColonSecondsTz,
    NumTz,
    NumSecondsTz,
    NumShortTz,
    NumColonTz,
    NumColonSecondsTz,
    /// `.000`-style fraction: fixed width, always emitted, mandatory on parse.
    FracSecond0 { digits: usize, comma: bool },
    /// `.999`-style fraction: trailing zeros dropped, omitted entirely when
    /// zero, optional on parse.
    FracSecond9 { digits: usize, comma: bool },
}

impl Std {
    /// `true` for the elements that carry a numeric zone offset.
    pub fn is_numeric_zone(self) -> bool {
        matches!(
            self,
            Std::Iso8601Tz
                | Std::Iso8601SecondsTz
                | Std::Iso8601ShortTz
                | Std::Iso8601ColonTz
                | Std::Iso8601ColonSecondsTz
                | Std::NumTz
                | Std::NumSecondsTz
                | Std::NumShortTz
                | Std::NumColonTz
                | Std::NumColonSecondsTz
        )
    }
}

/// The elements `0x` selects, indexed by the digit that follows the `0`.
const STD_0X: [Std; 6] = [
    Std::ZeroMonth,
    Std::ZeroDay,
    Std::ZeroHour12,
    Std::ZeroMinute,
    Std::ZeroSecond,
    Std::Year,
];

fn starts_with_lower_case(s: &[u8]) -> bool {
    !s.is_empty() && s[0].is_ascii_lowercase()
}

/// Splits `layout` at the next recognised element.
///
/// Returns the literal prefix, the element (absent when the rest of the layout
/// is all literal), and the remaining layout after the element.
pub fn next_std_chunk(layout: &str) -> (&str, Option<Std>, &str) {
    let b = layout.as_bytes();
    let n = b.len();
    let mut i = 0;
    while i < n {
        match b[i] {
            b'J' => {
                if n >= i + 3 && &b[i..i + 3] == b"Jan" {
                    if n >= i + 7 && &b[i..i + 7] == b"January" {
                        return (&layout[..i], Some(Std::LongMonth), &layout[i + 7..]);
                    }
                    if !starts_with_lower_case(&b[i + 3..]) {
                        return (&layout[..i], Some(Std::Month), &layout[i + 3..]);
                    }
                }
            }
            b'M' => {
                if n >= i + 3 {
                    if &b[i..i + 3] == b"Mon" {
                        if n >= i + 6 && &b[i..i + 6] == b"Monday" {
                            return (&layout[..i], Some(Std::LongWeekDay), &layout[i + 6..]);
                        }
                        if !starts_with_lower_case(&b[i + 3..]) {
                            return (&layout[..i], Some(Std::WeekDay), &layout[i + 3..]);
                        }
                    }
                    if &b[i..i + 3] == b"MST" {
                        return (&layout[..i], Some(Std::Tz), &layout[i + 3..]);
                    }
                }
            }
            b'0' => {
                if n >= i + 2 && (b'1'..=b'6').contains(&b[i + 1]) {
                    let std = STD_0X[(b[i + 1] - b'1') as usize];
                    return (&layout[..i], Some(std), &layout[i + 2..]);
                }
                if n >= i + 3 && b[i + 1] == b'0' && b[i + 2] == b'2' {
                    return (&layout[..i], Some(Std::ZeroYearDay), &layout[i + 3..]);
                }
            }
            b'1' => {
                if n >= i + 2 && b[i + 1] == b'5' {
                    return (&layout[..i], Some(Std::Hour), &layout[i + 2..]);
                }
                return (&layout[..i], Some(Std::NumMonth), &layout[i + 1..]);
            }
            b'2' => {
                if n >= i + 4 && &b[i..i + 4] == b"2006" {
                    return (&layout[..i], Some(Std::LongYear), &layout[i + 4..]);
                }
                return (&layout[..i], Some(Std::Day), &layout[i + 1..]);
            }
            b'_' => {
                if n > i + 1 && b[i + 1] == b'2' {
                    // `_2006` is a literal underscore followed by a long year.
                    if n >= i + 5 && &b[i + 1..i + 5] == b"2006" {
                        return (&layout[..i + 1], Some(Std::LongYear), &layout[i + 5..]);
                    }
                    return (&layout[..i], Some(Std::UnderDay), &layout[i + 2..]);
                }
                if n > i + 2 && b[i + 1] == b'_' && b[i + 2] == b'2' {
                    return (&layout[..i], Some(Std::UnderYearDay), &layout[i + 3..]);
                }
            }
            b'3' => return (&layout[..i], Some(Std::Hour12), &layout[i + 1..]),
            b'4' => return (&layout[..i], Some(Std::Minute), &layout[i + 1..]),
            b'5' => return (&layout[..i], Some(Std::Second), &layout[i + 1..]),
            b'P' => {
                if n >= i + 2 && b[i + 1] == b'M' {
                    return (&layout[..i], Some(Std::Pm), &layout[i + 2..]);
                }
            }
            b'p' => {
                if n >= i + 2 && b[i + 1] == b'm' {
                    return (&layout[..i], Some(Std::LowerPm), &layout[i + 2..]);
                }
            }
            b'-' => {
                if n >= i + 7 && &b[i..i + 7] == b"-070000" {
                    return (&layout[..i], Some(Std::NumSecondsTz), &layout[i + 7..]);
                }
                if n >= i + 9 && &b[i..i + 9] == b"-07:00:00" {
                    return (&layout[..i], Some(Std::NumColonSecondsTz), &layout[i + 9..]);
                }
                if n >= i + 5 && &b[i..i + 5] == b"-0700" {
                    return (&layout[..i], Some(Std::NumTz), &layout[i + 5..]);
                }
                if n >= i + 6 && &b[i..i + 6] == b"-07:00" {
                    return (&layout[..i], Some(Std::NumColonTz), &layout[i + 6..]);
                }
                if n >= i + 3 && &b[i..i + 3] == b"-07" {
                    return (&layout[..i], Some(Std::NumShortTz), &layout[i + 3..]);
                }
            }
            b'Z' => {
                if n >= i + 7 && &b[i..i + 7] == b"Z070000" {
                    return (&layout[..i], Some(Std::Iso8601SecondsTz), &layout[i + 7..]);
                }
                if n >= i + 9 && &b[i..i + 9] == b"Z07:00:00" {
                    return (&layout[..i], Some(Std::Iso8601ColonSecondsTz), &layout[i + 9..]);
                }
                if n >= i + 5 && &b[i..i + 5] == b"Z0700" {
                    return (&layout[..i], Some(Std::Iso8601Tz), &layout[i + 5..]);
                }
                if n >= i + 6 && &b[i..i + 6] == b"Z07:00" {
                    return (&layout[..i], Some(Std::Iso8601ColonTz), &layout[i + 6..]);
                }
                if n >= i + 3 && &b[i..i + 3] == b"Z07" {
                    return (&layout[..i], Some(Std::Iso8601ShortTz), &layout[i + 3..]);
                }
            }
            c @ (b'.' | b',') if i + 1 < n && (b[i + 1] == b'0' || b[i + 1] == b'9') => {
                {
                    let ch = b[i + 1];
                    let mut j = i + 1;
                    while j < n && b[j] == ch {
                        j += 1;
                    }
                    // The run of digits must end here — only a fractional
                    // second is all digits.
                    if !(j < n && b[j].is_ascii_digit()) {
                        let digits = j - (i + 1);
                        let comma = c == b',';
                        let std = if ch == b'9' {
                            Std::FracSecond9 { digits, comma }
                        } else {
                            Std::FracSecond0 { digits, comma }
                        };
                        return (&layout[..i], Some(std), &layout[j..]);
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    (layout, None, "")
}
