use std::sync::{Mutex, MutexGuard, OnceLock};

use now::deps::gotime::Time;
use now::{Config, Location, Weekday};

const FORMAT: &str = "2006-01-02 15:04:05.999999999";

fn location_caracas() -> &'static Location {
    static LOC: OnceLock<Location> = OnceLock::new();
    LOC.get_or_init(|| Location::load("America/Caracas").expect("America/Caracas"))
}

fn location_berlin() -> &'static Location {
    static LOC: OnceLock<Location> = OnceLock::new();
    LOC.get_or_init(|| Location::load("Europe/Berlin").expect("Europe/Berlin"))
}

fn time_caracas() -> Time {
    Time::new(2016, 1, 1, 12, 10, 0, 0, location_caracas())
}

/// The Go suite mutates the package globals `WeekStartDay` and `TimeFormats`
/// and relies on `go test` running a package's tests sequentially in source
/// order — `TestMondayAndSunday` asserts a Sunday week start without setting
/// one, because the preceding test left it there. Rust runs tests in parallel,
/// so each test takes this guard, which serialises them and restores the
/// package defaults. No assertion changes: the defaults restored here are
/// exactly the values in effect at that point in the Go run.
fn setup() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    let guard = LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    now::set_week_start_day(Weekday::Sunday);
    now::reset_time_formats();
    now::set_default_config(None);
    guard
}

#[track_caller]
fn assert_time(actual: &Time, expected: &str, msg: &str) {
    let actual_str = actual.format(FORMAT);
    assert_eq!(
        actual_str, expected,
        "Failed {msg}: actual: {actual_str}, expected: {expected}"
    );
}

#[test]
fn test_beginning_of() {
    let _guard = setup();

    let n = Time::new(2013, 11, 18, 17, 51, 49, 123456789, &Location::utc());

    assert_time(
        &now::with(n.clone()).beginning_of_minute(),
        "2013-11-18 17:51:00",
        "BeginningOfMinute",
    );

    now::set_week_start_day(Weekday::Monday);
    assert_time(
        &now::with(n.clone()).beginning_of_week(),
        "2013-11-18 00:00:00",
        "BeginningOfWeek, FirstDayMonday",
    );

    now::set_week_start_day(Weekday::Tuesday);
    assert_time(
        &now::with(n.clone()).beginning_of_week(),
        "2013-11-12 00:00:00",
        "BeginningOfWeek, FirstDayTuesday",
    );

    now::set_week_start_day(Weekday::Wednesday);
    assert_time(
        &now::with(n.clone()).beginning_of_week(),
        "2013-11-13 00:00:00",
        "BeginningOfWeek, FirstDayWednesday",
    );

    now::set_week_start_day(Weekday::Thursday);
    assert_time(
        &now::with(n.clone()).beginning_of_week(),
        "2013-11-14 00:00:00",
        "BeginningOfWeek, FirstDayThursday",
    );

    now::set_week_start_day(Weekday::Friday);
    assert_time(
        &now::with(n.clone()).beginning_of_week(),
        "2013-11-15 00:00:00",
        "BeginningOfWeek, FirstDayFriday",
    );

    now::set_week_start_day(Weekday::Saturday);
    assert_time(
        &now::with(n.clone()).beginning_of_week(),
        "2013-11-16 00:00:00",
        "BeginningOfWeek, FirstDaySaturday",
    );

    now::set_week_start_day(Weekday::Sunday);
    assert_time(
        &now::with(n.clone()).beginning_of_week(),
        "2013-11-17 00:00:00",
        "BeginningOfWeek, FirstDaySunday",
    );

    assert_time(
        &now::with(n.clone()).beginning_of_hour(),
        "2013-11-18 17:00:00",
        "BeginningOfHour",
    );

    // Truncate with hour bug
    assert_time(
        &now::with(time_caracas()).beginning_of_hour(),
        "2016-01-01 12:00:00",
        "BeginningOfHour Caracas",
    );

    assert_time(
        &now::with(n.clone()).beginning_of_day(),
        "2013-11-18 00:00:00",
        "BeginningOfDay",
    );

    let location = Location::load("Japan").expect("Error loading location");
    let beginning_of_day = Time::new(2015, 5, 1, 0, 0, 0, 0, &location);
    assert_time(
        &now::with(beginning_of_day).beginning_of_day(),
        "2015-05-01 00:00:00",
        "BeginningOfDay",
    );

    // DST
    let dst_beginning_of_day = Time::new(2017, 10, 29, 10, 0, 0, 0, location_berlin());
    assert_time(
        &now::with(dst_beginning_of_day).beginning_of_day(),
        "2017-10-29 00:00:00",
        "BeginningOfDay DST",
    );

    assert_time(
        &now::with(n.clone()).beginning_of_week(),
        "2013-11-17 00:00:00",
        "BeginningOfWeek",
    );

    let dst_beginning_of_week = Time::new(2017, 10, 30, 12, 0, 0, 0, location_berlin());
    assert_time(
        &now::with(dst_beginning_of_week).beginning_of_week(),
        "2017-10-29 00:00:00",
        "BeginningOfWeek",
    );

    let dst_beginning_of_week = Time::new(2017, 10, 29, 12, 0, 0, 0, location_berlin());
    assert_time(
        &now::with(dst_beginning_of_week).beginning_of_week(),
        "2017-10-29 00:00:00",
        "BeginningOfWeek",
    );

    now::set_week_start_day(Weekday::Monday);
    assert_time(
        &now::with(n.clone()).beginning_of_week(),
        "2013-11-18 00:00:00",
        "BeginningOfWeek, FirstDayMonday",
    );
    let dst_beginning_of_week = Time::new(2017, 10, 24, 12, 0, 0, 0, location_berlin());
    assert_time(
        &now::with(dst_beginning_of_week).beginning_of_week(),
        "2017-10-23 00:00:00",
        "BeginningOfWeek, FirstDayMonday",
    );

    let dst_beginning_of_week = Time::new(2017, 10, 29, 12, 0, 0, 0, location_berlin());
    assert_time(
        &now::with(dst_beginning_of_week).beginning_of_week(),
        "2017-10-23 00:00:00",
        "BeginningOfWeek, FirstDayMonday",
    );

    now::set_week_start_day(Weekday::Sunday);

    assert_time(
        &now::with(n.clone()).beginning_of_month(),
        "2013-11-01 00:00:00",
        "BeginningOfMonth",
    );

    // DST
    let dst_beginning_of_month = Time::new(2017, 10, 31, 0, 0, 0, 0, location_berlin());
    assert_time(
        &now::with(dst_beginning_of_month.clone()).beginning_of_month(),
        "2017-10-01 00:00:00",
        "BeginningOfMonth DST",
    );

    assert_time(
        &now::with(n.clone()).beginning_of_quarter(),
        "2013-10-01 00:00:00",
        "BeginningOfQuarter",
    );

    // DST
    assert_time(
        &now::with(dst_beginning_of_month).beginning_of_quarter(),
        "2017-10-01 00:00:00",
        "BeginningOfQuarter DST",
    );
    let dst_beginning_of_quarter = Time::new(2017, 11, 24, 0, 0, 0, 0, location_berlin());
    assert_time(
        &now::with(dst_beginning_of_quarter.clone()).beginning_of_quarter(),
        "2017-10-01 00:00:00",
        "BeginningOfQuarter DST",
    );

    assert_time(
        &now::with(dst_beginning_of_quarter.clone()).beginning_of_half(),
        "2017-07-01 00:00:00",
        "BeginningOfHalf DST",
    );

    assert_time(
        &now::with(n.add_date(0, -1, 0)).beginning_of_quarter(),
        "2013-10-01 00:00:00",
        "BeginningOfQuarter",
    );

    assert_time(
        &now::with(n.add_date(0, 1, 0)).beginning_of_quarter(),
        "2013-10-01 00:00:00",
        "BeginningOfQuarter",
    );

    assert_time(
        &now::with(n.add_date(0, 1, 0)).beginning_of_half(),
        "2013-07-01 00:00:00",
        "BeginningOfHalf",
    );

    // DST
    assert_time(
        &now::with(dst_beginning_of_quarter).beginning_of_year(),
        "2017-01-01 00:00:00",
        "BeginningOfYear DST",
    );

    assert_time(
        &now::with(time_caracas()).beginning_of_year(),
        "2016-01-01 00:00:00",
        "BeginningOfYear Caracas",
    );
}

#[test]
fn test_end_of() {
    let _guard = setup();

    let n = Time::new(2013, 11, 18, 17, 51, 49, 123456789, &Location::utc());

    assert_time(
        &now::with(n.clone()).end_of_minute(),
        "2013-11-18 17:51:59.999999999",
        "EndOfMinute",
    );

    assert_time(
        &now::with(n.clone()).end_of_hour(),
        "2013-11-18 17:59:59.999999999",
        "EndOfHour",
    );

    assert_time(
        &now::with(time_caracas()).end_of_hour(),
        "2016-01-01 12:59:59.999999999",
        "EndOfHour Caracas",
    );

    assert_time(
        &now::with(n.clone()).end_of_day(),
        "2013-11-18 23:59:59.999999999",
        "EndOfDay",
    );

    let dst_end_of_day = Time::new(2017, 10, 29, 1, 0, 0, 0, location_berlin());
    assert_time(
        &now::with(dst_end_of_day).end_of_day(),
        "2017-10-29 23:59:59.999999999",
        "EndOfDay DST",
    );

    now::set_week_start_day(Weekday::Tuesday);
    assert_time(
        &now::with(n.clone()).end_of_week(),
        "2013-11-18 23:59:59.999999999",
        "EndOfWeek, FirstDayTuesday",
    );

    now::set_week_start_day(Weekday::Wednesday);
    assert_time(
        &now::with(n.clone()).end_of_week(),
        "2013-11-19 23:59:59.999999999",
        "EndOfWeek, FirstDayWednesday",
    );

    now::set_week_start_day(Weekday::Thursday);
    assert_time(
        &now::with(n.clone()).end_of_week(),
        "2013-11-20 23:59:59.999999999",
        "EndOfWeek, FirstDayThursday",
    );

    now::set_week_start_day(Weekday::Friday);
    assert_time(
        &now::with(n.clone()).end_of_week(),
        "2013-11-21 23:59:59.999999999",
        "EndOfWeek, FirstDayFriday",
    );

    now::set_week_start_day(Weekday::Saturday);
    assert_time(
        &now::with(n.clone()).end_of_week(),
        "2013-11-22 23:59:59.999999999",
        "EndOfWeek, FirstDaySaturday",
    );

    now::set_week_start_day(Weekday::Sunday);
    assert_time(
        &now::with(n.clone()).end_of_week(),
        "2013-11-23 23:59:59.999999999",
        "EndOfWeek, FirstDaySunday",
    );

    now::set_week_start_day(Weekday::Monday);
    assert_time(
        &now::with(n.clone()).end_of_week(),
        "2013-11-24 23:59:59.999999999",
        "EndOfWeek, FirstDayMonday",
    );

    let dst_end_of_week = Time::new(2017, 10, 24, 12, 0, 0, 0, location_berlin());
    assert_time(
        &now::with(dst_end_of_week).end_of_week(),
        "2017-10-29 23:59:59.999999999",
        "EndOfWeek, FirstDayMonday",
    );

    let dst_end_of_week = Time::new(2017, 10, 29, 12, 0, 0, 0, location_berlin());
    assert_time(
        &now::with(dst_end_of_week).end_of_week(),
        "2017-10-29 23:59:59.999999999",
        "EndOfWeek, FirstDayMonday",
    );

    now::set_week_start_day(Weekday::Sunday);
    assert_time(
        &now::with(n.clone()).end_of_week(),
        "2013-11-23 23:59:59.999999999",
        "EndOfWeek",
    );

    let dst_end_of_week = Time::new(2017, 10, 29, 0, 0, 0, 0, location_berlin());
    assert_time(
        &now::with(dst_end_of_week).end_of_week(),
        "2017-11-04 23:59:59.999999999",
        "EndOfWeek",
    );

    let dst_end_of_week = Time::new(2017, 10, 29, 12, 0, 0, 0, location_berlin());
    assert_time(
        &now::with(dst_end_of_week).end_of_week(),
        "2017-11-04 23:59:59.999999999",
        "EndOfWeek",
    );

    assert_time(
        &now::with(n.clone()).end_of_month(),
        "2013-11-30 23:59:59.999999999",
        "EndOfMonth",
    );

    assert_time(
        &now::with(n.clone()).end_of_quarter(),
        "2013-12-31 23:59:59.999999999",
        "EndOfQuarter",
    );

    assert_time(
        &now::with(n.clone()).end_of_half(),
        "2013-12-31 23:59:59.999999999",
        "EndOfHalf",
    );

    assert_time(
        &now::with(n.add_date(0, -1, 0)).end_of_quarter(),
        "2013-12-31 23:59:59.999999999",
        "EndOfQuarter",
    );

    assert_time(
        &now::with(n.add_date(0, 1, 0)).end_of_quarter(),
        "2013-12-31 23:59:59.999999999",
        "EndOfQuarter",
    );

    assert_time(
        &now::with(n.add_date(0, 1, 0)).end_of_half(),
        "2013-12-31 23:59:59.999999999",
        "EndOfHalf",
    );

    assert_time(
        &now::with(n.clone()).end_of_year(),
        "2013-12-31 23:59:59.999999999",
        "EndOfYear",
    );

    let n1 = Time::new(2013, 2, 18, 17, 51, 49, 123456789, &Location::utc());
    assert_time(
        &now::with(n1).end_of_month(),
        "2013-02-28 23:59:59.999999999",
        "EndOfMonth for 2013/02",
    );

    let n2 = Time::new(1900, 2, 18, 17, 51, 49, 123456789, &Location::utc());
    assert_time(
        &now::with(n2).end_of_month(),
        "1900-02-28 23:59:59.999999999",
        "EndOfMonth",
    );
}

#[test]
fn test_monday_and_sunday() {
    let _guard = setup();

    let n = Time::new(2013, 11, 19, 17, 51, 49, 123456789, &Location::utc());
    let n2 = Time::new(2013, 11, 24, 17, 51, 49, 123456789, &Location::utc());
    let n_dst = Time::new(2017, 10, 29, 10, 0, 0, 0, location_berlin());

    assert_time(
        &now::with(n.clone()).monday(&[]),
        "2013-11-18 00:00:00",
        "Monday",
    );

    assert_time(
        &now::with(n2.clone()).monday(&[]),
        "2013-11-18 00:00:00",
        "Monday",
    );

    assert_time(
        &now::with(time_caracas()).monday(&[]),
        "2015-12-28 00:00:00",
        "Monday Caracas",
    );

    assert_time(
        &now::with(n_dst.clone()).monday(&[]),
        "2017-10-23 00:00:00",
        "Monday DST",
    );

    assert_time(
        &now::with(n.clone()).monday(&["17:51:49"]),
        "2013-11-18 17:51:49",
        "Monday",
    );

    assert_time(
        &now::with(n.clone()).monday(&["17:51"]),
        "2013-11-18 17:51:00",
        "Monday",
    );

    assert_time(
        &now::with(n.clone()).sunday(&[]),
        "2013-11-24 00:00:00",
        "Sunday",
    );

    assert_time(
        &now::with(n.clone()).sunday(&["18:19:20"]),
        "2013-11-24 18:19:20",
        "Sunday",
    );

    assert_time(
        &now::with(n.clone()).sunday(&["18:19"]),
        "2013-11-24 18:19:00",
        "Sunday",
    );

    assert_time(
        &now::with(n2).sunday(&[]),
        "2013-11-24 00:00:00",
        "Sunday",
    );

    assert_time(
        &now::with(time_caracas()).sunday(&[]),
        "2016-01-03 00:00:00",
        "Sunday Caracas",
    );

    assert_time(
        &now::with(n_dst.clone()).sunday(&[]),
        "2017-10-29 00:00:00",
        "Sunday DST",
    );

    assert_time(
        &now::with(n.clone()).end_of_sunday(),
        "2013-11-24 23:59:59.999999999",
        "EndOfSunday",
    );

    assert_time(
        &now::with(time_caracas()).end_of_sunday(),
        "2016-01-03 23:59:59.999999999",
        "EndOfSunday Caracas",
    );

    assert_time(
        &now::with(n_dst).end_of_sunday(),
        "2017-10-29 23:59:59.999999999",
        "EndOfSunday DST",
    );

    assert_time(
        &now::with(n.clone()).beginning_of_week(),
        "2013-11-17 00:00:00",
        "BeginningOfWeek, FirstDayMonday",
    );

    now::set_week_start_day(Weekday::Monday);
    assert_time(
        &now::with(n).beginning_of_week(),
        "2013-11-18 00:00:00",
        "BeginningOfWeek, FirstDayMonday",
    );
}

#[test]
fn test_parse() {
    let _guard = setup();

    let n = Time::new(2013, 11, 18, 17, 51, 49, 123456789, &Location::utc());
    let at = |strs: &[&str]| now::with(n.clone()).must_parse(strs);

    assert_time(&at(&["2002"]), "2002-01-01 00:00:00", "Parse 2002");

    assert_time(&at(&["2002-10"]), "2002-10-01 00:00:00", "Parse 2002-10");

    assert_time(
        &at(&["2002-10-12"]),
        "2002-10-12 00:00:00",
        "Parse 2002-10-12",
    );

    assert_time(
        &at(&["2002-10-12 22"]),
        "2002-10-12 22:00:00",
        "Parse 2002-10-12 22",
    );

    assert_time(
        &at(&["2002-10-12 22:14"]),
        "2002-10-12 22:14:00",
        "Parse 2002-10-12 22:14",
    );

    assert_time(
        &at(&["2002-10-12 2:4"]),
        "2002-10-12 02:04:00",
        "Parse 2002-10-12 2:4",
    );

    assert_time(
        &at(&["2002-10-12 02:04"]),
        "2002-10-12 02:04:00",
        "Parse 2002-10-12 02:04",
    );

    assert_time(
        &at(&["2002-10-12 22:14:56"]),
        "2002-10-12 22:14:56",
        "Parse 2002-10-12 22:14:56",
    );

    assert_time(
        &at(&["2002-10-12 00:14:56"]),
        "2002-10-12 00:14:56",
        "Parse 2002-10-12 00:14:56",
    );

    assert_time(
        &at(&["2013-12-19 23:28:09.999999999 +0800 CST"]),
        "2013-12-19 23:28:09.999999999",
        "Parse two strings 2013-12-19 23:28:09.999999999 +0800 CST",
    );

    assert_time(&at(&["10-12"]), "2013-10-12 00:00:00", "Parse 10-12");

    assert_time(&at(&["18"]), "2013-11-18 18:00:00", "Parse 18 as hour");

    assert_time(&at(&["18:20"]), "2013-11-18 18:20:00", "Parse 18:20");

    assert_time(&at(&["00:01"]), "2013-11-18 00:01:00", "Parse 00:01");

    assert_time(&at(&["00:00:00"]), "2013-11-18 00:00:00", "Parse 00:00:00");

    assert_time(&at(&["18:20:39"]), "2013-11-18 18:20:39", "Parse 18:20:39");

    assert_time(
        &at(&["18:20:39", "2011-01-01"]),
        "2011-01-01 18:20:39",
        "Parse two strings 18:20:39, 2011-01-01",
    );

    assert_time(
        &at(&["2011-1-1", "18:20:39"]),
        "2011-01-01 18:20:39",
        "Parse two strings 2011-01-01, 18:20:39",
    );

    assert_time(
        &at(&["2011-01-01", "18"]),
        "2011-01-01 18:00:00",
        "Parse two strings 2011-01-01, 18",
    );

    assert_time(
        &at(&["2002-10-12T00:14:56Z"]),
        "2002-10-12 00:14:56",
        "Parse 2002-10-12T00:14:56Z",
    );
    assert_time(
        &at(&["2002-10-12T00:00:56Z"]),
        "2002-10-12 00:00:56",
        "Parse 2002-10-12T00:00:56Z",
    );
    assert_time(
        &at(&["2002-10-12T00:00:00.999Z"]),
        "2002-10-12 00:00:00.999",
        "Parse 2002-10-12T00:00:00.999Z",
    );
    assert_time(
        &at(&["2002-10-12T00:14:56.999999Z"]),
        "2002-10-12 00:14:56.999999",
        "Parse 2002-10-12T00:14:56.999999Z",
    );
    assert_time(
        &at(&["2002-10-12T00:00:56.999999999Z"]),
        "2002-10-12 00:00:56.999999999",
        "Parse 2002-10-12T00:00:56.999999999Z",
    );

    assert_time(
        &at(&["2002-10-12T00:14:56+08:00"]),
        "2002-10-12 00:14:56",
        "Parse 2002-10-12T00:14:56+08:00",
    );
    let (_, off) = at(&["2002-10-12T00:14:56+08:00"]).zone();
    assert_eq!(
        off, 28800,
        "Parse 2002-10-12T00:14:56+08:00 shouldn't lose time zone offset"
    );
    assert_time(
        &at(&["2002-10-12T00:00:56-07:00"]),
        "2002-10-12 00:00:56",
        "Parse 2002-10-12T00:00:56-07:00",
    );
    let (_, off2) = at(&["2002-10-12T00:00:56-07:00"]).zone();
    assert_eq!(
        off2, -25200,
        "Parse 2002-10-12T00:00:56-07:00 shouldn't lose time zone offset"
    );
    assert_time(
        &at(&["2002-10-12T00:01:12.333+0200"]),
        "2002-10-12 00:01:12.333",
        "Parse 2002-10-12T00:01:12.333+0200",
    );
    let (_, off3) = at(&["2002-10-12T00:01:12.333+0200"]).zone();
    assert_eq!(
        off3, 7200,
        "Parse 2002-10-12T00:01:12.333+0200 shouldn't lose time zone offset"
    );
    assert_time(
        &at(&["2002-10-12T00:00:56.999999999+08:00"]),
        "2002-10-12 00:00:56.999999999",
        "Parse 2002-10-12T00:00:56.999999999+08:00",
    );
    let (_, off4) = at(&["2002-10-12T00:14:56.999999999+08:00"]).zone();
    assert_eq!(
        off4, 28800,
        "Parse 2002-10-12T00:14:56.999999999+08:00 shouldn't lose time zone offset"
    );
    assert_time(
        &at(&["2002-10-12T00:00:56.666666-07:00"]),
        "2002-10-12 00:00:56.666666",
        "Parse 2002-10-12T00:00:56.666666-07:00",
    );
    let (_, off5) = at(&["2002-10-12T00:00:56.666666-07:00"]).zone();
    assert_eq!(
        off5, -25200,
        "Parse 2002-10-12T00:00:56.666666-07:00 shouldn't lose time zone offset"
    );
    assert_time(
        &at(&["2002-10-12T00:01:12.999999999-06"]),
        "2002-10-12 00:01:12.999999999",
        "Parse 2002-10-12T00:01:12.999999999-06",
    );
    let (_, off6) = at(&["2002-10-12T00:01:12.999999999-06"]).zone();
    assert_eq!(
        off6, -21600,
        "Parse 2002-10-12T00:01:12.999999999-06 shouldn't lose time zone offset"
    );

    now::append_time_format("02 Jan 15:04");
    assert_time(
        &at(&["04 Feb 12:09"]),
        "2013-02-04 12:09:00",
        "Parse 04 Feb 12:09 with specified format",
    );

    assert_time(
        &at(&["23:28:9 Dec 19, 2013 PST"]),
        "2013-12-19 23:28:09",
        "Parse 23:28:9 Dec 19, 2013 PST",
    );

    assert_eq!(
        at(&["23:28:9 Dec 19, 2013 PST"]).location().to_string(),
        "PST",
        "Parse 23:28:9 Dec 19, 2013 PST shouldn't lose time zone"
    );

    let n2 = at(&["23:28:9 Dec 19, 2013 PST"]);
    assert_eq!(
        now::with(n2).must_parse(&["10:20"]).location().to_string(),
        "PST",
        "Parse 10:20 shouldn't change time zone"
    );

    now::append_time_format("2006-01-02T15:04:05.0");
    assert_eq!(
        now::must_parse_in_location(&Location::utc(), &["2018-02-13T15:17:06.0"]).to_string(),
        "2018-02-13 15:17:06 +0000 UTC",
        "ParseInLocation 2018-02-13T15:17:06.0"
    );

    now::append_time_format("2006-01-02 15:04:05.000");
    assert_time(
        &at(&["2018-04-20 21:22:23.473"]),
        "2018-04-20 21:22:23.473",
        "Parse 2018/04/20 21:22:23.473",
    );

    now::append_time_format("15:04:05.000");
    assert_time(
        &at(&["13:00:01.365"]),
        "2013-11-18 13:00:01.365",
        "Parse 13:00:01.365",
    );

    now::append_time_format("2006-01-02 15:04:05.000000");
    assert_time(
        &at(&["2010-01-01 07:24:23.131384"]),
        "2010-01-01 07:24:23.131384",
        "Parse 2010-01-01 07:24:23.131384",
    );
    assert_time(
        &at(&["00:00:00.182736"]),
        "2013-11-18 00:00:00.182736",
        "Parse 00:00:00.182736",
    );

    let n3 = now::must_parse(&["2017-12-11T10:25:49Z"]);
    assert_eq!(
        *n3.location(),
        Location::utc(),
        "time location should be UTC, but got {}",
        n3.location()
    );
}

#[test]
fn test_between() {
    let _guard = setup();

    let tm = Time::new(2015, 6, 30, 17, 51, 49, 123456789, &Location::local());
    assert!(
        now::with(tm.clone()).between("23:28:9 Dec 19, 2013 PST", "23:28:9 Dec 19, 2015 PST"),
        "Between"
    );

    assert!(
        now::with(tm).between("2015-05-12 12:20", "2015-06-30 17:51:50"),
        "Between"
    );
}

#[test]
fn test_config() {
    let _guard = setup();

    let location = Location::load("Asia/Shanghai")
        .expect("load location for Asia/Shanghai should returns no error");

    let my_config = Config {
        week_start_day: Weekday::Monday,
        time_location: Some(location),
        time_formats: vec!["2006-01-02 15:04:05".to_string()],
    };

    // 2013-11-18 17:51:49.123456789 Mon
    let n = Time::new(2013, 11, 18, 17, 51, 49, 123456789, &Location::local());
    assert_time(
        &my_config.with(n).beginning_of_week(),
        "2013-11-18 00:00:00",
        "BeginningOfWeek, FirstDayMonday",
    );

    let result = my_config
        .parse(&["2018-02-13 15:17:06"])
        .expect("ParseInLocation 2018-02-13T15:17:06.0");
    assert_eq!(
        result.to_string(),
        "2018-02-13 15:17:06 +0800 CST",
        "ParseInLocation 2018-02-13T15:17:06.0, got {result}"
    );

    let result = my_config.must_parse(&["2018-02-13 15:17:06"]);
    assert_eq!(
        result.to_string(),
        "2018-02-13 15:17:06 +0800 CST",
        "ParseInLocation 2018-02-13T15:17:06.0, got {result}"
    );
}

#[test]
fn test_quarter() {
    let _guard = setup();

    struct TestCase {
        given_date: Time,
        expected_quarter: u32,
    }

    let tests = [
        TestCase {
            given_date: Time::new(2021, 6, 18, 0, 0, 0, 0, &Location::utc()),
            expected_quarter: 2,
        },
        TestCase {
            given_date: Time::new(2021, 7, 18, 0, 0, 0, 0, &Location::utc()),
            expected_quarter: 3,
        },
    ];

    for tc in tests {
        let got = now::with(tc.given_date).quarter();
        assert_eq!(
            got, tc.expected_quarter,
            "Quarter {} expected, got {got}",
            tc.expected_quarter
        );
    }
}

/// The counterpart of the Go original's `Example()`: it asserts nothing, and
/// exists to keep every package-level entry point exercised and compiling.
#[test]
fn example() {
    let _guard = setup();

    Time::now(); // 2013-11-18 17:51:49.123456789 Mon

    now::beginning_of_minute(); // 2013-11-18 17:51:00 Mon
    now::beginning_of_hour(); // 2013-11-18 17:00:00 Mon
    now::beginning_of_day(); // 2013-11-18 00:00:00 Mon
    now::beginning_of_week(); // 2013-11-17 00:00:00 Sun

    now::set_week_start_day(Weekday::Monday); // Set Monday as first day
    now::beginning_of_week(); // 2013-11-18 00:00:00 Mon
    now::beginning_of_month(); // 2013-11-01 00:00:00 Fri
    now::beginning_of_quarter(); // 2013-10-01 00:00:00 Tue
    now::beginning_of_year(); // 2013-01-01 00:00:00 Tue

    now::end_of_minute(); // 2013-11-18 17:51:59.999999999 Mon
    now::end_of_hour(); // 2013-11-18 17:59:59.999999999 Mon
    now::end_of_day(); // 2013-11-18 23:59:59.999999999 Mon
    now::end_of_week(); // 2013-11-23 23:59:59.999999999 Sat

    now::set_week_start_day(Weekday::Monday); // Set Monday as first day
    now::end_of_week(); // 2013-11-24 23:59:59.999999999 Sun
    now::end_of_month(); // 2013-11-30 23:59:59.999999999 Sat
    now::end_of_quarter(); // 2013-12-31 23:59:59.999999999 Tue
    now::end_of_year(); // 2013-12-31 23:59:59.999999999 Tue

    // Use another time
    let t = Time::new(2013, 2, 18, 17, 51, 49, 123456789, &Location::utc());
    now::with(t).end_of_month(); // 2013-02-28 23:59:59.999999999 Thu

    now::monday(&[]); // 2013-11-18 00:00:00 Mon
    now::monday(&["17:44"]); // 2013-11-18 17:44:00 Mon
    now::sunday(&[]); // 2013-11-24 00:00:00 Sun
    now::sunday(&["17:44"]); // 2013-11-24 17:44:00 Sun
    now::end_of_sunday(); // 2013-11-24 23:59:59.999999999 Sun
}
