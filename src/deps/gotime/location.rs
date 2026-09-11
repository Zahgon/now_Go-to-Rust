//! Time zones, reproducing the parts of Go's `*time.Location` that `now`'s
//! behaviour depends on.
//!
//! Go's `Location.lookup` returns not just the offset in effect at an instant
//! but the half-open interval `[start, end)` over which that offset holds.
//! `time.Date` needs those bounds to disambiguate a local wall clock that falls
//! in a DST gap or repeat (see [`super::Time::date`]). `chrono-tz` exposes the
//! offset but not the interval, so the bounds are recovered by an exponential
//! probe followed by a binary search over the offset function, which is exact
//! because zone offsets are piecewise constant in time.

use chrono::{DateTime, Offset, TimeZone};
use chrono_tz::{OffsetName, Tz};
use std::fmt;
use std::sync::OnceLock;

/// The zone in effect over a half-open interval of absolute time.
///
/// Mirrors the tuple returned by Go's `(*Location).lookup`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZoneInfo {
    /// Zone abbreviation, e.g. `CET`, `PST`, `-0430`.
    pub name: String,
    /// Seconds east of UTC.
    pub offset: i32,
    /// First second of the interval, `i64::MIN` when unbounded.
    pub start: i64,
    /// One past the last second of the interval, `i64::MAX` when unbounded.
    pub end: i64,
}

/// A time zone.
///
/// The variants mirror the ways Go can produce a `*time.Location`: the `UTC`
/// singleton, `Local`, `FixedZone`, and `LoadLocation` of an IANA name.
#[derive(Clone, Debug)]
pub enum Location {
    Utc,
    Local(Tz),
    Fixed { name: String, offset: i32 },
    Named(Tz),
}

/// Go compares locations by pointer identity, so `LoadLocation("UTC")` is
/// `time.UTC`. Structural equality reproduces that: the `UTC` singleton is a
/// variant of its own and IANA zones compare by identity of the loaded zone.
impl PartialEq for Location {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Location::Utc, Location::Utc) => true,
            (Location::Local(_), Location::Local(_)) => true,
            (Location::Named(a), Location::Named(b)) => a == b,
            (
                Location::Fixed { name: n1, offset: o1 },
                Location::Fixed { name: n2, offset: o2 },
            ) => n1 == n2 && o1 == o2,
            _ => false,
        }
    }
}
impl Eq for Location {}

/// Go's `(*Location).String()`.
impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Location::Utc => f.write_str("UTC"),
            Location::Local(_) => f.write_str("Local"),
            Location::Fixed { name, .. } => f.write_str(name),
            Location::Named(tz) => f.write_str(tz.name()),
        }
    }
}

/// How far the interval probe will search before declaring the interval
/// unbounded. Zone tables in the IANA database do not extend past this.
const MAX_SEARCH_SECONDS: i64 = 400 * 365 * 86_400;

static LOCAL: OnceLock<Location> = OnceLock::new();

impl Location {
    /// Go's `time.UTC`.
    pub fn utc() -> Location {
        Location::Utc
    }

    /// Go's `time.Local`: the system zone, resolved once.
    ///
    /// Falls back to UTC when the platform cannot report a zone name, which is
    /// what Go also does when it cannot read the zone database.
    pub fn local() -> Location {
        LOCAL
            .get_or_init(|| match iana_time_zone::get_timezone() {
                Ok(name) => match name.parse::<Tz>() {
                    Ok(tz) => Location::Local(tz),
                    Err(_) => Location::Utc,
                },
                Err(_) => Location::Utc,
            })
            .clone()
    }

    /// Go's `time.LoadLocation`.
    ///
    /// `"UTC"` and `""` yield the UTC singleton and `"Local"` the system zone,
    /// exactly as Go does; anything else is looked up in the IANA database.
    pub fn load(name: &str) -> Result<Location, String> {
        match name {
            "" | "UTC" => Ok(Location::Utc),
            "Local" => Ok(Location::local()),
            _ => name
                .parse::<Tz>()
                .map(Location::Named)
                .map_err(|_| format!("unknown time zone {name}")),
        }
    }

    /// Go's `time.FixedZone`.
    pub fn fixed(name: &str, offset: i32) -> Location {
        Location::Fixed { name: name.to_string(), offset }
    }

    fn tz(&self) -> Option<Tz> {
        match self {
            Location::Named(tz) | Location::Local(tz) => Some(*tz),
            _ => None,
        }
    }

    /// Seconds east of UTC at the given absolute instant.
    fn raw_offset(&self, unix: i64) -> i32 {
        match self {
            Location::Utc => 0,
            Location::Fixed { offset, .. } => *offset,
            Location::Named(tz) | Location::Local(tz) => {
                let dt = DateTime::from_timestamp(unix, 0).unwrap_or_else(|| {
                    DateTime::from_timestamp(unix.clamp(-62_135_596_800, 253_402_300_799), 0)
                        .expect("clamped timestamp is representable")
                });
                tz.offset_from_utc_datetime(&dt.naive_utc()).fix().local_minus_utc()
            }
        }
    }

    /// Zone abbreviation at the given absolute instant.
    ///
    /// The IANA database stores numeric abbreviations such as `-0430` verbatim
    /// and Go returns them as-is. `chrono-tz` reports those as absent, so they
    /// are rendered back into Go's form here.
    fn raw_name(&self, unix: i64) -> String {
        match self {
            Location::Utc => "UTC".to_string(),
            Location::Fixed { name, .. } => name.clone(),
            Location::Named(tz) | Location::Local(tz) => {
                let dt = DateTime::from_timestamp(unix, 0).unwrap_or_else(|| {
                    DateTime::from_timestamp(unix.clamp(-62_135_596_800, 253_402_300_799), 0)
                        .expect("clamped timestamp is representable")
                });
                let off = tz.offset_from_utc_datetime(&dt.naive_utc());
                match off.abbreviation() {
                    Some(abbrev) => abbrev.to_string(),
                    None => numeric_zone_name(off.fix().local_minus_utc()),
                }
            }
        }
    }

    /// Go's `(*Location).lookup`: the zone in effect at `unix`, together with
    /// the interval over which that zone holds.
    pub fn lookup(&self, unix: i64) -> ZoneInfo {
        let offset = self.raw_offset(unix);
        let name = self.raw_name(unix);
        let (start, end) = match self.tz() {
            None => (i64::MIN, i64::MAX),
            Some(_) => (self.interval_start(unix, offset), self.interval_end(unix, offset)),
        };
        ZoneInfo { name, offset, start, end }
    }

    /// First second of the offset interval containing `unix`.
    fn interval_start(&self, unix: i64, offset: i32) -> i64 {
        let mut step: i64 = 1;
        let mut lo;
        loop {
            let probe = unix.saturating_sub(step);
            if self.raw_offset(probe) != offset {
                lo = probe;
                break;
            }
            if step > MAX_SEARCH_SECONDS {
                return i64::MIN;
            }
            step = step.saturating_mul(2);
        }
        // Smallest x in (lo, unix] whose offset still equals `offset`.
        let mut hi = unix;
        while hi - lo > 1 {
            let mid = lo + (hi - lo) / 2;
            if self.raw_offset(mid) == offset {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        hi
    }

    /// One past the last second of the offset interval containing `unix`.
    fn interval_end(&self, unix: i64, offset: i32) -> i64 {
        let mut step: i64 = 1;
        let mut hi;
        loop {
            let probe = unix.saturating_add(step);
            if self.raw_offset(probe) != offset {
                hi = probe;
                break;
            }
            if step > MAX_SEARCH_SECONDS {
                return i64::MAX;
            }
            step = step.saturating_mul(2);
        }
        // Smallest x in (unix, hi] whose offset differs from `offset`.
        let mut lo = unix;
        while hi - lo > 1 {
            let mid = lo + (hi - lo) / 2;
            if self.raw_offset(mid) == offset {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        hi
    }

    /// Go's `(*Location).lookupName`: the offset of the zone called `name`,
    /// preferring one that was actually in effect at `unix`.
    ///
    /// Go scans the location's zone table. `chrono-tz` does not expose one, so
    /// the candidate zones are gathered by probing across the surrounding year,
    /// which reaches both the standard and the daylight zone of any location
    /// that has them.
    pub fn lookup_name(&self, name: &str, unix: i64) -> Option<i32> {
        const PROBE_STEP: i64 = 61 * 86_400;
        let mut fallback: Option<i32> = None;
        for k in -6..=6i64 {
            let probe = unix.saturating_add(k * PROBE_STEP);
            let zone = self.lookup(probe);
            if zone.name != name {
                continue;
            }
            // Go verifies the zone really was in effect at the instant the wall
            // clock implies, which is `unix - offset`.
            let confirmed = self.lookup(unix.saturating_sub(zone.offset as i64));
            if confirmed.name == name {
                return Some(confirmed.offset);
            }
            fallback.get_or_insert(zone.offset);
        }
        fallback
    }
}

/// Renders an offset the way the IANA database names an unabbreviated zone.
///
/// The database trims trailing zero components, so `America/Caracas` is `-04`
/// after 2016 but `-0430` before it, and `Asia/Kolkata` is `+0530`. Go returns
/// those strings verbatim, so anything else would drift from it in `MST` and in
/// `Time::String`.
fn numeric_zone_name(offset: i32) -> String {
    let sign = if offset < 0 { '-' } else { '+' };
    let abs = offset.abs();
    let (h, m, s) = (abs / 3600, (abs / 60) % 60, abs % 60);
    if s != 0 {
        format!("{sign}{h:02}{m:02}{s:02}")
    } else if m != 0 {
        format!("{sign}{h:02}{m:02}")
    } else {
        format!("{sign}{h:02}")
    }
}
