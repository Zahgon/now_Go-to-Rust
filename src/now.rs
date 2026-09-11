use std::fmt;
use std::sync::OnceLock;

use regex::Regex;

use crate::deps::gotime::location::Location;
use crate::deps::gotime::parse::parse_in_location;
use crate::deps::gotime::{Time, Weekday, MINUTE, NANOSECOND, SECOND};
use crate::time_list::format_time_to_list;
use crate::{new, Now};

/// The error `Parse` reports when no layout matches.
///
/// The Go original returns `(time.Time, error)` and, when a later string in a
/// multi-string call fails, hands back both the time parsed so far *and* the
/// error. Rust's `Result` holds one or the other, so the partially folded time
/// travels on the error and is reachable through [`ParseError::partial`].
#[derive(Clone, Debug)]
pub struct ParseError {
    message: String,
    partial: Time,
}

impl ParseError {
    /// The message, `"Can't parse string as time: <input>"`.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The time folded so far, which is what the Go original returns alongside
    /// the error. It is the zero time when nothing parsed.
    pub fn partial(&self) -> &Time {
        &self.partial
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ParseError {}

// Match 15:04:05, 15:04:05.000, 15:04:05.000000 15, 2017-01-01 15:04,
// 2021-07-20T00:59:10Z, 2021-07-20T00:59:10+08:00, 2021-07-20T00:00:10-07:00 etc
fn has_time_regexp() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(\s+|^\s*|T)\d{1,2}((:\d{1,2})*|((:\d{1,2}){2}\.(\d{3}|\d{6}|\d{9})))(\s*$|[Z+-])")
            .expect("hasTimeRegexp is a valid pattern")
    })
}

// Match 15:04:05, 15, 15:04:05.000, 15:04:05.000000, etc
fn only_time_regexp() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^\s*\d{1,2}((:\d{1,2})*|((:\d{1,2}){2}\.(\d{3}|\d{6}|\d{9})))\s*$")
            .expect("onlyTimeRegexp is a valid pattern")
    })
}

impl Now {
    /// Beginning of minute.
    pub fn beginning_of_minute(&self) -> Time {
        self.time.truncate(MINUTE)
    }

    /// Beginning of hour.
    pub fn beginning_of_hour(&self) -> Time {
        let (y, m, d) = self.time.date();
        Time::new(y, m, d, self.time.hour(), 0, 0, 0, self.time.location())
    }

    /// Beginning of day.
    pub fn beginning_of_day(&self) -> Time {
        let (y, m, d) = self.time.date();
        Time::new(y, m, d, 0, 0, 0, 0, self.time.location())
    }

    /// Beginning of week.
    pub fn beginning_of_week(&self) -> Time {
        let t = self.beginning_of_day();
        let mut weekday = t.weekday().as_i64();

        if self.config.week_start_day != Weekday::Sunday {
            let week_start_day = self.config.week_start_day.as_i64();

            if weekday < week_start_day {
                weekday = weekday + 7 - week_start_day;
            } else {
                weekday -= week_start_day;
            }
        }
        t.add_date(0, 0, -weekday)
    }

    /// Beginning of month.
    pub fn beginning_of_month(&self) -> Time {
        let (y, m, _) = self.time.date();
        Time::new(y, m, 1, 0, 0, 0, 0, self.time.location())
    }

    /// Beginning of quarter.
    pub fn beginning_of_quarter(&self) -> Time {
        let month = self.beginning_of_month();
        let offset = (month.month() - 1) % 3;
        month.add_date(0, -offset, 0)
    }

    /// Beginning of half year.
    pub fn beginning_of_half(&self) -> Time {
        let month = self.beginning_of_month();
        let offset = (month.month() - 1) % 6;
        month.add_date(0, -offset, 0)
    }

    /// Beginning of year.
    pub fn beginning_of_year(&self) -> Time {
        let (y, _, _) = self.time.date();
        Time::new(y, 1, 1, 0, 0, 0, 0, self.time.location())
    }

    /// End of minute.
    pub fn end_of_minute(&self) -> Time {
        self.beginning_of_minute().add(MINUTE - NANOSECOND)
    }

    /// End of hour.
    pub fn end_of_hour(&self) -> Time {
        self.beginning_of_hour().add(crate::HOUR - NANOSECOND)
    }

    /// End of day.
    pub fn end_of_day(&self) -> Time {
        let (y, m, d) = self.time.date();
        Time::new(
            y,
            m,
            d,
            23,
            59,
            59,
            SECOND - NANOSECOND,
            self.time.location(),
        )
    }

    /// End of week.
    pub fn end_of_week(&self) -> Time {
        self.beginning_of_week().add_date(0, 0, 7).add(-NANOSECOND)
    }

    /// End of month.
    pub fn end_of_month(&self) -> Time {
        self.beginning_of_month().add_date(0, 1, 0).add(-NANOSECOND)
    }

    /// End of quarter.
    pub fn end_of_quarter(&self) -> Time {
        self.beginning_of_quarter().add_date(0, 3, 0).add(-NANOSECOND)
    }

    /// End of half year.
    pub fn end_of_half(&self) -> Time {
        self.beginning_of_half().add_date(0, 6, 0).add(-NANOSECOND)
    }

    /// End of year.
    pub fn end_of_year(&self) -> Time {
        self.beginning_of_year().add_date(1, 0, 0).add(-NANOSECOND)
    }

    /// Returns the Monday date of the week specified by this time.
    ///
    /// # Panics
    ///
    /// Panics when `strs` is non-empty and none of them parse, matching the Go
    /// original.
    pub fn monday(&self, strs: &[&str]) -> Time {
        let parse_time = if !strs.is_empty() {
            match self.parse(strs) {
                Ok(t) => t,
                Err(e) => panic!("{e}"),
            }
        } else {
            self.beginning_of_day()
        };
        let mut weekday = parse_time.weekday().as_i64();
        if weekday == 0 {
            weekday = 7;
        }
        parse_time.add_date(0, 0, -weekday + 1)
    }

    /// Returns the Sunday date of the week specified by this time.
    ///
    /// # Panics
    ///
    /// Panics when `strs` is non-empty and none of them parse, matching the Go
    /// original.
    pub fn sunday(&self, strs: &[&str]) -> Time {
        let parse_time = if !strs.is_empty() {
            match self.parse(strs) {
                Ok(t) => t,
                Err(e) => panic!("{e}"),
            }
        } else {
            self.beginning_of_day()
        };
        let mut weekday = parse_time.weekday().as_i64();
        if weekday == 0 {
            weekday = 7;
        }
        parse_time.add_date(0, 0, 7 - weekday)
    }

    /// End of sunday.
    pub fn end_of_sunday(&self) -> Time {
        new(self.sunday(&[])).end_of_day()
    }

    /// Returns the yearly quarter.
    pub fn quarter(&self) -> u32 {
        ((self.time.month() - 1) / 3 + 1) as u32
    }

    fn parse_with_format(&self, str: &str, location: &Location) -> Result<Time, String> {
        for format in &self.config.time_formats {
            if let Ok(t) = parse_in_location(format, str, location) {
                return Ok(t);
            }
        }
        Err(format!("Can't parse string as time: {str}"))
    }

    /// Parses strings to time.
    ///
    /// Each string is folded onto the receiver's time: components the string
    /// does not carry are taken from the time built so far.
    pub fn parse(&self, strs: &[&str]) -> Result<Time, ParseError> {
        let mut set_current_time = false;
        let current_location = self.time.location().clone();
        let mut only_time_in_str = true;
        let mut current_time = format_time_to_list(&self.time);

        let mut t = Time::zero();
        let mut err: Option<String> = None;

        for str in strs {
            let has_time_in_str = has_time_regexp().is_match(str);
            only_time_in_str =
                has_time_in_str && only_time_in_str && only_time_regexp().is_match(str);

            match self.parse_with_format(str, &current_location) {
                Ok(parsed) => {
                    err = None;
                    let location = parsed.location().clone();
                    let mut parse_time = format_time_to_list(&parsed);

                    for i in 0..parse_time.len() {
                        // Don't reset hour, minute, second if current time str including time
                        if has_time_in_str && i <= 3 {
                            continue;
                        }

                        // If value is zero, replace it with current time
                        if parse_time[i] == 0 {
                            if set_current_time {
                                parse_time[i] = current_time[i];
                            }
                        } else {
                            set_current_time = true;
                        }

                        // if current time only includes time, should change day, month to current time
                        if only_time_in_str && (i == 4 || i == 5) {
                            parse_time[i] = current_time[i];
                            continue;
                        }
                    }

                    t = Time::new(
                        parse_time[6],
                        parse_time[5],
                        parse_time[4],
                        parse_time[3],
                        parse_time[2],
                        parse_time[1],
                        parse_time[0],
                        &location,
                    );
                    current_time = format_time_to_list(&t);
                }
                Err(e) => err = Some(e),
            }
        }

        match err {
            Some(message) => Err(ParseError { message, partial: t }),
            None => Ok(t),
        }
    }

    /// Must parse strings to time or it will panic.
    ///
    /// # Panics
    ///
    /// Panics when no layout matches, matching the Go original.
    pub fn must_parse(&self, strs: &[&str]) -> Time {
        match self.parse(strs) {
            Ok(t) => t,
            Err(e) => panic!("{e}"),
        }
    }

    /// Checks whether this time is between the begin and end time.
    ///
    /// Both bounds are exclusive.
    ///
    /// # Panics
    ///
    /// Panics when either bound fails to parse, matching the Go original.
    pub fn between(&self, begin: &str, end: &str) -> bool {
        let begin_time = self.must_parse(&[begin]);
        let end_time = self.must_parse(&[end]);
        self.time.after(&begin_time) && self.time.before(&end_time)
    }
}
