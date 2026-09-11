//! A narrow reimplementation of the Go `time` behaviour that `now` exposes.
//!
//! Only what the library's public contract depends on is reproduced: an
//! instant-plus-location `Time`, Go's `time.Date` normalisation and DST
//! disambiguation, calendar `AddDate` versus absolute `Add`/`Truncate`, and the
//! reference-layout format and parse engine.

pub mod civil;
pub mod format;
pub mod layout;
pub mod location;
pub mod parse;

use civil::{civil_from_days, days_from_civil, norm};
pub use location::Location;

/// Seconds between 0001-01-01 and 1970-01-01, Go's `unixToInternal`.
pub const UNIX_TO_INTERNAL: i64 = 62_135_596_800;

const SECONDS_PER_DAY: i64 = 86_400;

/// A span of time in nanoseconds, mirroring Go's `time.Duration`.
pub type Duration = i64;

/// Go's `time.Nanosecond`.
pub const NANOSECOND: Duration = 1;
/// Go's `time.Second`.
pub const SECOND: Duration = 1_000_000_000;
/// Go's `time.Minute`.
pub const MINUTE: Duration = 60 * SECOND;
/// Go's `time.Hour`.
pub const HOUR: Duration = 60 * MINUTE;

/// Day of the week, numbered as Go numbers `time.Weekday` (Sunday is zero).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Weekday {
    Sunday = 0,
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
}

impl Weekday {
    /// The weekday with the given Go numbering, Sunday being zero.
    pub fn from_i64(n: i64) -> Weekday {
        match n.rem_euclid(7) {
            0 => Weekday::Sunday,
            1 => Weekday::Monday,
            2 => Weekday::Tuesday,
            3 => Weekday::Wednesday,
            4 => Weekday::Thursday,
            5 => Weekday::Friday,
            _ => Weekday::Saturday,
        }
    }

    /// Go's numbering of this weekday.
    pub fn as_i64(self) -> i64 {
        self as i64
    }

    /// Go's `Weekday.String()`.
    pub fn name(self) -> &'static str {
        LONG_DAY_NAMES[self as usize]
    }
}

pub(crate) const LONG_DAY_NAMES: [&str; 7] = [
    "Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday",
];
pub(crate) const SHORT_DAY_NAMES: [&str; 7] =
    ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
pub(crate) const LONG_MONTH_NAMES: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August",
    "September", "October", "November", "December",
];
pub(crate) const SHORT_MONTH_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// An instant in time together with the location used to render it, mirroring
/// Go's `time.Time`.
///
/// Like Go's, the value is an absolute instant; the location affects only how
/// the wall clock reads. Two `Time`s at the same instant in different locations
/// are equal under [`Time::equal`] but format differently.
#[derive(Clone, Debug)]
pub struct Time {
    sec: i64,
    nsec: u32,
    loc: Location,
}

impl Time {
    /// Go's `time.Date`.
    ///
    /// Out-of-range components carry into the next larger unit (month `0` is
    /// December of the previous year, day `0` is the last day of the previous
    /// month), and the wall clock is resolved to an instant using Go's DST
    /// disambiguation: look up the offset at the wall clock read as if it were
    /// UTC, then, if the implied instant falls outside the interval that offset
    /// came from, look up again at the interval boundary.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        year: i64,
        month: i64,
        day: i64,
        hour: i64,
        min: i64,
        sec: i64,
        nsec: i64,
        loc: &Location,
    ) -> Time {
        // Normalise from the smallest unit upward, exactly as Go does.
        let (sec, nsec) = norm(sec, nsec, 1_000_000_000);
        let (min, sec) = norm(min, sec, 60);
        let (hour, min) = norm(hour, min, 60);
        let (day, hour) = norm(day, hour, 24);
        let (year, m) = norm(year, month - 1, 12);
        let month = m + 1;

        let days = days_from_civil(year, month, 1) + (day - 1);
        let unix = days * SECONDS_PER_DAY + hour * 3600 + min * 60 + sec;

        // `unix` currently holds the wall clock read as if it were UTC.
        let zone = loc.lookup(unix);
        let mut offset = zone.offset;
        let unix = if offset != 0 {
            let utc = unix - offset as i64;
            if utc < zone.start {
                offset = loc.lookup(zone.start - 1).offset;
            } else if utc >= zone.end {
                offset = loc.lookup(zone.end).offset;
            }
            unix - offset as i64
        } else {
            unix
        };

        Time { sec: unix, nsec: nsec as u32, loc: loc.clone() }
    }

    /// Go's zero `time.Time`: January 1, year 1, 00:00:00 UTC.
    pub fn zero() -> Time {
        Time { sec: -UNIX_TO_INTERNAL, nsec: 0, loc: Location::Utc }
    }

    /// Builds a `Time` directly from an absolute instant.
    pub fn from_unix(sec: i64, nsec: u32, loc: &Location) -> Time {
        Time { sec, nsec, loc: loc.clone() }
    }

    /// Go's `time.Now`.
    pub fn now() -> Time {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        Time {
            sec: now.as_secs() as i64,
            nsec: now.subsec_nanos(),
            loc: Location::local(),
        }
    }

    /// Seconds since the Unix epoch.
    pub fn unix_sec(&self) -> i64 {
        self.sec
    }

    /// Go's `Time.Nanosecond`.
    pub fn nanosecond(&self) -> u32 {
        self.nsec
    }

    /// Go's `Time.Location`.
    pub fn location(&self) -> &Location {
        &self.loc
    }

    /// Go's `Time.In`: the same instant rendered in another location.
    pub fn in_location(&self, loc: &Location) -> Time {
        Time { sec: self.sec, nsec: self.nsec, loc: loc.clone() }
    }

    /// Go's `Time.Zone`: the abbreviation and offset in effect at this instant.
    pub fn zone(&self) -> (String, i32) {
        let zone = self.loc.lookup(self.sec);
        (zone.name, zone.offset)
    }

    /// Seconds east of UTC in effect at this instant.
    pub fn offset(&self) -> i32 {
        self.loc.lookup(self.sec).offset
    }

    /// The wall clock as seconds since the Unix epoch in the local zone.
    fn local_sec(&self) -> i64 {
        self.sec + self.offset() as i64
    }

    /// Go's `Time.Date`: year, month (1-based) and day of the month.
    pub fn date(&self) -> (i64, i64, i64) {
        civil_from_days(self.local_sec().div_euclid(SECONDS_PER_DAY))
    }

    /// Go's `Time.Clock`: hour, minute and second.
    pub fn clock(&self) -> (i64, i64, i64) {
        let secs = self.local_sec().rem_euclid(SECONDS_PER_DAY);
        (secs / 3600, (secs / 60) % 60, secs % 60)
    }

    /// Go's `Time.Year`.
    pub fn year(&self) -> i64 {
        self.date().0
    }

    /// Go's `Time.Month`, 1-based.
    pub fn month(&self) -> i64 {
        self.date().1
    }

    /// Go's `Time.Day`.
    pub fn day(&self) -> i64 {
        self.date().2
    }

    /// Go's `Time.Hour`.
    pub fn hour(&self) -> i64 {
        self.clock().0
    }

    /// Go's `Time.Minute`.
    pub fn minute(&self) -> i64 {
        self.clock().1
    }

    /// Go's `Time.Second`.
    pub fn second(&self) -> i64 {
        self.clock().2
    }

    /// Go's `Time.YearDay`, 1-based.
    pub fn year_day(&self) -> i64 {
        let (y, m, d) = self.date();
        days_from_civil(y, m, d) - days_from_civil(y, 1, 1) + 1
    }

    /// Go's `Time.Weekday`.
    ///
    /// 1970-01-01 was a Thursday, which fixes the phase.
    pub fn weekday(&self) -> Weekday {
        Weekday::from_i64(self.local_sec().div_euclid(SECONDS_PER_DAY) + 4)
    }

    /// Go's `Time.Add`: absolute arithmetic on the instant, with no zone
    /// re-resolution.
    pub fn add(&self, d: Duration) -> Time {
        let total = self.sec as i128 * 1_000_000_000 + self.nsec as i128 + d as i128;
        Time {
            sec: total.div_euclid(1_000_000_000) as i64,
            nsec: total.rem_euclid(1_000_000_000) as u32,
            loc: self.loc.clone(),
        }
    }

    /// Go's `Time.AddDate`: calendar arithmetic on the broken-down date, then a
    /// fresh resolution through [`Time::new`]. It does not clamp — one month
    /// after 31 January is 3 March.
    pub fn add_date(&self, years: i64, months: i64, days: i64) -> Time {
        let (year, month, day) = self.date();
        let (hour, min, sec) = self.clock();
        Time::new(
            year + years,
            month + months,
            day + days,
            hour,
            min,
            sec,
            self.nsec as i64,
            &self.loc,
        )
    }

    /// Go's `Time.Truncate`: rounds the *absolute* time down toward the zero
    /// time (0001-01-01 UTC), not the local wall clock.
    pub fn truncate(&self, d: Duration) -> Time {
        if d <= 0 {
            return self.clone();
        }
        let since_year_one =
            (self.sec as i128 + UNIX_TO_INTERNAL as i128) * 1_000_000_000 + self.nsec as i128;
        let r = since_year_one.rem_euclid(d as i128);
        self.add(-(r as i64))
    }

    /// Go's `Time.After`.
    pub fn after(&self, other: &Time) -> bool {
        (self.sec, self.nsec) > (other.sec, other.nsec)
    }

    /// Go's `Time.Before`.
    pub fn before(&self, other: &Time) -> bool {
        (self.sec, self.nsec) < (other.sec, other.nsec)
    }

    /// Go's `Time.Equal`: same instant, regardless of location.
    pub fn equal(&self, other: &Time) -> bool {
        (self.sec, self.nsec) == (other.sec, other.nsec)
    }
}

/// Go's `Time.String`, which formats with the layout
/// `"2006-01-02 15:04:05.999999999 -0700 MST"`.
impl std::fmt::Display for Time {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.format("2006-01-02 15:04:05.999999999 -0700 MST"))
    }
}
