//! `now` is a time toolkit for rust.
//!
//! More details README here: <https://github.com/jinzhu/now>
//!
//! ```
//! now::beginning_of_minute(); // 2013-11-18 17:51:00 Mon
//! now::beginning_of_day();    // 2013-11-18 00:00:00 Mon
//! now::end_of_day();          // 2013-11-18 23:59:59.999999999 Mon
//! ```

pub mod deps;
mod now;
mod time_list;

use std::sync::RwLock;

pub use deps::gotime::location::Location;
pub use deps::gotime::{Duration, Time, Weekday, HOUR, MINUTE, NANOSECOND, SECOND};

/// Layouts from Go's `time` package, spelled with the reference time
/// `Mon Jan 2 15:04:05 MST 2006`. They are part of the default
/// [`time_formats`] list, so their exact text is contract.
pub mod layouts {
    pub const ANSIC: &str = "Mon Jan _2 15:04:05 2006";
    pub const UNIX_DATE: &str = "Mon Jan _2 15:04:05 MST 2006";
    pub const RUBY_DATE: &str = "Mon Jan 02 15:04:05 -0700 2006";
    pub const RFC822: &str = "02 Jan 06 15:04 MST";
    pub const RFC822Z: &str = "02 Jan 06 15:04 -0700";
    pub const RFC850: &str = "Monday, 02-Jan-06 15:04:05 MST";
    pub const RFC1123: &str = "Mon, 02 Jan 2006 15:04:05 MST";
    pub const RFC1123Z: &str = "Mon, 02 Jan 2006 15:04:05 -0700";
    pub const RFC3339: &str = "2006-01-02T15:04:05Z07:00";
    pub const RFC3339_NANO: &str = "2006-01-02T15:04:05.999999999Z07:00";
    pub const KITCHEN: &str = "3:04PM";
    pub const STAMP: &str = "Jan _2 15:04:05";
    pub const STAMP_MILLI: &str = "Jan _2 15:04:05.000";
    pub const STAMP_MICRO: &str = "Jan _2 15:04:05.000000";
    pub const STAMP_NANO: &str = "Jan _2 15:04:05.000000000";
}

/// The default time formats, parsed in this order.
///
/// `Parse` returns the first layout that matches the whole input, so the order
/// is observable: reordering this list changes results.
fn default_time_formats() -> Vec<String> {
    [
        "2006",
        "2006-1",
        "2006-1-2",
        "2006-1-2 15",
        "2006-1-2 15:4",
        "2006-1-2 15:4:5",
        "1-2",
        "15:4:5",
        "15:4",
        "15",
        "15:4:5 Jan 2, 2006 MST",
        "2006-01-02 15:04:05.999999999 -0700 MST",
        "2006-01-02T15:04:05Z0700",
        "2006-01-02T15:04:05Z07",
        "2006.1.2",
        "2006.1.2 15:04:05",
        "2006.01.02",
        "2006.01.02 15:04:05",
        "2006.01.02 15:04:05.999999999",
        "1/2/2006",
        "1/2/2006 15:4:5",
        "2006/01/02",
        "20060102",
        "2006/01/02 15:04:05",
        layouts::ANSIC,
        layouts::UNIX_DATE,
        layouts::RUBY_DATE,
        layouts::RFC822,
        layouts::RFC822Z,
        layouts::RFC850,
        layouts::RFC1123,
        layouts::RFC1123Z,
        layouts::RFC3339,
        layouts::RFC3339_NANO,
        layouts::KITCHEN,
        layouts::STAMP,
        layouts::STAMP_MILLI,
        layouts::STAMP_MICRO,
        layouts::STAMP_NANO,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

static WEEK_START_DAY: RwLock<Weekday> = RwLock::new(Weekday::Sunday);
static TIME_FORMATS: RwLock<Option<Vec<String>>> = RwLock::new(None);
static DEFAULT_CONFIG: RwLock<Option<Config>> = RwLock::new(None);

/// The day a week starts on. Default is Sunday.
pub fn week_start_day() -> Weekday {
    *WEEK_START_DAY.read().expect("week start day lock")
}

/// Sets the day a week starts on.
pub fn set_week_start_day(day: Weekday) {
    *WEEK_START_DAY.write().expect("week start day lock") = day;
}

/// The default time formats strings will be parsed as.
pub fn time_formats() -> Vec<String> {
    TIME_FORMATS
        .read()
        .expect("time formats lock")
        .clone()
        .unwrap_or_else(default_time_formats)
}

/// Replaces the default time formats.
pub fn set_time_formats(formats: Vec<String>) {
    *TIME_FORMATS.write().expect("time formats lock") = Some(formats);
}

/// Appends a layout to the default time formats.
pub fn append_time_format(format: impl Into<String>) {
    let mut formats = time_formats();
    formats.push(format.into());
    set_time_formats(formats);
}

/// Restores the built-in default time formats.
pub fn reset_time_formats() {
    *TIME_FORMATS.write().expect("time formats lock") = None;
}

/// The configuration [`with`] uses when one has been installed.
pub fn default_config() -> Option<Config> {
    DEFAULT_CONFIG.read().expect("default config lock").clone()
}

/// Installs, or clears, the configuration [`with`] uses.
pub fn set_default_config(config: Option<Config>) {
    *DEFAULT_CONFIG.write().expect("default config lock") = config;
}

/// Configuration for the now package.
#[derive(Clone, Debug)]
pub struct Config {
    pub week_start_day: Weekday,
    pub time_location: Option<Location>,
    pub time_formats: Vec<String>,
}

impl Config {
    /// Initializes [`Now`] based on this configuration.
    pub fn with(&self, t: Time) -> Now {
        Now { time: t, config: self.clone() }
    }

    /// Parses strings to time based on this configuration.
    pub fn parse(&self, strs: &[&str]) -> Result<Time, ParseError> {
        match &self.time_location {
            None => self.with(Time::now()).parse(strs),
            Some(loc) => self.with(Time::now().in_location(loc)).parse(strs),
        }
    }

    /// Must parse strings to time or it will panic.
    pub fn must_parse(&self, strs: &[&str]) -> Time {
        match &self.time_location {
            None => self.with(Time::now()).must_parse(strs),
            Some(loc) => self.with(Time::now().in_location(loc)).must_parse(strs),
        }
    }
}

/// A time together with the configuration used to interpret it.
///
/// The Go original embeds `time.Time`, so every `time.Time` method is callable
/// on a `*Now`. [`Deref`](std::ops::Deref) to [`Time`] is the Rust equivalent.
#[derive(Clone, Debug)]
pub struct Now {
    pub time: Time,
    pub config: Config,
}

impl std::ops::Deref for Now {
    type Target = Time;
    fn deref(&self) -> &Time {
        &self.time
    }
}

/// Initializes [`Now`] with a time.
///
/// Uses [`default_config`] when one is installed, otherwise builds a
/// configuration from the current [`week_start_day`] and [`time_formats`].
pub fn with(t: Time) -> Now {
    let config = default_config().unwrap_or_else(|| Config {
        week_start_day: week_start_day(),
        time_location: None,
        time_formats: time_formats(),
    });
    Now { time: t, config }
}

/// Initializes [`Now`] with a time.
pub fn new(t: Time) -> Now {
    with(t)
}

pub use now::ParseError;

/// Generates the package-level form of a `Now` method: it operates on the
/// current time, exactly as the Go original's free functions do.
macro_rules! package_level {
    ($(#[$m:meta])* $name:ident -> $ret:ty) => {
        $(#[$m])*
        pub fn $name() -> $ret {
            with(Time::now()).$name()
        }
    };
}

package_level!(
    /// Beginning of minute.
    beginning_of_minute -> Time);
package_level!(
    /// Beginning of hour.
    beginning_of_hour -> Time);
package_level!(
    /// Beginning of day.
    beginning_of_day -> Time);
package_level!(
    /// Beginning of week.
    beginning_of_week -> Time);
package_level!(
    /// Beginning of month.
    beginning_of_month -> Time);
package_level!(
    /// Beginning of quarter.
    beginning_of_quarter -> Time);
package_level!(
    /// Beginning of year.
    beginning_of_year -> Time);
package_level!(
    /// End of minute.
    end_of_minute -> Time);
package_level!(
    /// End of hour.
    end_of_hour -> Time);
package_level!(
    /// End of day.
    end_of_day -> Time);
package_level!(
    /// End of week.
    end_of_week -> Time);
package_level!(
    /// End of month.
    end_of_month -> Time);
package_level!(
    /// End of quarter.
    end_of_quarter -> Time);
package_level!(
    /// End of year.
    end_of_year -> Time);
package_level!(
    /// End of sunday.
    end_of_sunday -> Time);
package_level!(
    /// The yearly quarter.
    quarter -> u32);

/// Returns the [`Time`] value of Monday.
pub fn monday(strs: &[&str]) -> Time {
    with(Time::now()).monday(strs)
}

/// Returns the [`Time`] value of Sunday.
pub fn sunday(strs: &[&str]) -> Time {
    with(Time::now()).sunday(strs)
}

/// Parses strings to time.
pub fn parse(strs: &[&str]) -> Result<Time, ParseError> {
    with(Time::now()).parse(strs)
}

/// Parses strings to time in a location.
pub fn parse_in_location(loc: &Location, strs: &[&str]) -> Result<Time, ParseError> {
    with(Time::now().in_location(loc)).parse(strs)
}

/// Must parse strings to time or it will panic.
pub fn must_parse(strs: &[&str]) -> Time {
    with(Time::now()).must_parse(strs)
}

/// Must parse strings to time in a location or it will panic.
pub fn must_parse_in_location(loc: &Location, strs: &[&str]) -> Time {
    with(Time::now().in_location(loc)).must_parse(strs)
}

/// Checks whether now is between the begin and end time.
pub fn between(time1: &str, time2: &str) -> bool {
    with(Time::now()).between(time1, time2)
}
