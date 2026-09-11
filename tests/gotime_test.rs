//! Tests for the behaviour the Go original delegated to its standard library.
//!
//! Upstream, all of this was covered by the Go `time` and `regexp` test suites.
//! After the port it is `now`'s own code, so it needs `now`'s own tests: the
//! reference-layout format and parse engine, `time.Date` normalisation and DST
//! disambiguation, calendar `AddDate` versus absolute `Add`/`Truncate`, and
//! zone resolution on parse.

use now::deps::gotime::parse::parse_in_location;
use now::deps::gotime::{Time, Weekday, HOUR, MINUTE, NANOSECOND, SECOND};
use now::Location;

const FULL: &str = "2006-01-02 15:04:05.999999999";

fn utc(y: i64, m: i64, d: i64, h: i64, mi: i64, s: i64, ns: i64) -> Time {
    Time::new(y, m, d, h, mi, s, ns, &Location::utc())
}

fn berlin() -> Location {
    Location::load("Europe/Berlin").expect("Europe/Berlin")
}

// --- time.Date normalisation -------------------------------------------------

#[test]
fn date_normalises_out_of_range_components() {
    // Month 0 is December of the previous year; month 13 is January of the next.
    assert_eq!(utc(2013, 0, 1, 0, 0, 0, 0).format(FULL), "2012-12-01 00:00:00");
    assert_eq!(utc(2013, 13, 1, 0, 0, 0, 0).format(FULL), "2014-01-01 00:00:00");

    // Day 0 is the last day of the previous month.
    assert_eq!(utc(2013, 3, 0, 0, 0, 0, 0).format(FULL), "2013-02-28 00:00:00");
    assert_eq!(utc(2012, 3, 0, 0, 0, 0, 0).format(FULL), "2012-02-29 00:00:00");

    // Day 32 of a 31-day month is the 1st of the next.
    assert_eq!(utc(2013, 1, 32, 0, 0, 0, 0).format(FULL), "2013-02-01 00:00:00");

    // Fields normalise from the smallest unit upward.
    assert_eq!(
        utc(2013, 1, 1, 23, 59, 59, 1_000_000_000).format(FULL),
        "2013-01-02 00:00:00"
    );
    assert_eq!(utc(2013, 1, 1, 25, 0, 0, 0).format(FULL), "2013-01-02 01:00:00");
    assert_eq!(utc(2013, 1, 1, 0, -1, 0, 0).format(FULL), "2012-12-31 23:59:00");
}

#[test]
fn leap_years_follow_the_proleptic_gregorian_calendar() {
    // 1900 is divisible by 100 but not 400, so it is not a leap year.
    assert_eq!(utc(1900, 3, 0, 0, 0, 0, 0).format(FULL), "1900-02-28 00:00:00");
    assert_eq!(utc(2000, 3, 0, 0, 0, 0, 0).format(FULL), "2000-02-29 00:00:00");
    assert_eq!(utc(2024, 3, 0, 0, 0, 0, 0).format(FULL), "2024-02-29 00:00:00");
}

#[test]
fn weekday_and_year_day() {
    assert_eq!(utc(1970, 1, 1, 0, 0, 0, 0).weekday(), Weekday::Thursday);
    assert_eq!(utc(2013, 11, 18, 0, 0, 0, 0).weekday(), Weekday::Monday);
    assert_eq!(utc(2013, 11, 17, 0, 0, 0, 0).weekday(), Weekday::Sunday);
    // Pre-epoch instants must not fall a day out through truncating division.
    assert_eq!(utc(1969, 12, 31, 23, 0, 0, 0).weekday(), Weekday::Wednesday);

    assert_eq!(utc(2013, 1, 1, 0, 0, 0, 0).year_day(), 1);
    assert_eq!(utc(2013, 12, 31, 0, 0, 0, 0).year_day(), 365);
    assert_eq!(utc(2012, 12, 31, 0, 0, 0, 0).year_day(), 366);
}

// --- DST disambiguation ------------------------------------------------------

#[test]
fn date_resolves_a_dst_gap_the_way_go_does() {
    // Europe/Berlin springs forward 2017-03-26 02:00 -> 03:00, so 02:30 does
    // not exist. Go's rule re-looks-up at the interval boundary and lands on
    // 03:30 CEST.
    let t = Time::new(2017, 3, 26, 2, 30, 0, 0, &berlin());
    assert_eq!(t.format("2006-01-02 15:04:05 -0700 MST"), "2017-03-26 03:30:00 +0200 CEST");
}

#[test]
fn date_resolves_an_ambiguous_dst_hour_the_way_go_does() {
    // Europe/Berlin falls back 2017-10-29 03:00 -> 02:00, so 02:30 happens
    // twice. Go looks the offset up at the wall clock read as UTC, which is
    // already past the transition, so it picks the second occurrence, CET.
    // Verified against the Go original.
    let t = Time::new(2017, 10, 29, 2, 30, 0, 0, &berlin());
    assert_eq!(t.format("2006-01-02 15:04:05 -0700 MST"), "2017-10-29 02:30:00 +0100 CET");
}

#[test]
fn numeric_zone_names_follow_the_database_convention() {
    // The database trims trailing zero components, and Go returns its strings
    // verbatim: Caracas is -0430 before 2016 and -04 after it.
    let caracas = Location::load("America/Caracas").expect("America/Caracas");
    assert_eq!(Time::new(2016, 1, 1, 12, 0, 0, 0, &caracas).zone().0, "-0430");
    assert_eq!(Time::new(2017, 1, 1, 12, 0, 0, 0, &caracas).zone().0, "-04");

    // A zone that does have a name keeps it rather than going numeric.
    let kolkata = Location::load("Asia/Kolkata").expect("Asia/Kolkata");
    assert_eq!(Time::new(2017, 1, 1, 12, 0, 0, 0, &kolkata).zone(), ("IST".to_string(), 19800));

    let chatham = Location::load("Pacific/Chatham").expect("Pacific/Chatham");
    assert_eq!(Time::new(2017, 7, 1, 12, 0, 0, 0, &chatham).zone().0, "+1245");
}

#[test]
fn parse_reports_the_element_it_failed_on_not_the_tail() {
    // Go snapshots the value before an element consumes any of it.
    let e = parse_in_location("2006", "10-12", &Location::utc()).expect_err("fails");
    assert_eq!(
        e.to_string(),
        r#"parsing time "10-12" as "2006": cannot parse "10-12" as "2006""#
    );
    let e = parse_in_location("2006", "2002-10", &Location::utc()).expect_err("fails");
    assert_eq!(e.to_string(), r#"parsing time "2002-10": extra text: "-10""#);
}

#[test]
fn parse_range_checks_a_numeric_zone_offset() {
    // A range error outranks the malformed-input error for the same element.
    let e = parse_in_location("-0700", "9999-12-31", &Location::utc()).expect_err("fails");
    assert_eq!(
        e.to_string(),
        r#"parsing time "9999-12-31": time zone offset hour out of range"#
    );
    // Offsets of exactly 24 hours are accepted: the tests use `>`, not `>=`.
    assert!(parse_in_location("-0700", "+2400", &Location::utc()).is_ok());
}

#[test]
fn zone_lookup_reports_the_offset_in_effect() {
    let summer = Time::new(2017, 7, 1, 12, 0, 0, 0, &berlin());
    assert_eq!(summer.zone(), ("CEST".to_string(), 7200));

    let winter = Time::new(2017, 12, 1, 12, 0, 0, 0, &berlin());
    assert_eq!(winter.zone(), ("CET".to_string(), 3600));

    // Caracas ran at -04:30 in 2016; the database names that zone numerically.
    let caracas = Location::load("America/Caracas").expect("America/Caracas");
    let t = Time::new(2016, 1, 1, 12, 10, 0, 0, &caracas);
    assert_eq!(t.zone(), ("-0430".to_string(), -16200));
}

// --- AddDate / Add / Truncate ------------------------------------------------

#[test]
fn add_date_is_calendar_arithmetic_and_does_not_clamp() {
    // One month after 31 January is 3 March, not 28 February.
    assert_eq!(
        utc(2013, 1, 31, 0, 0, 0, 0).add_date(0, 1, 0).format(FULL),
        "2013-03-03 00:00:00"
    );
    assert_eq!(
        utc(2012, 1, 31, 0, 0, 0, 0).add_date(0, 1, 0).format(FULL),
        "2012-03-02 00:00:00"
    );
    assert_eq!(
        utc(2013, 12, 31, 0, 0, 0, 0).add_date(0, 0, 1).format(FULL),
        "2014-01-01 00:00:00"
    );
}

#[test]
fn add_date_re_resolves_the_zone_but_add_does_not() {
    // 2017-10-23 00:00 CEST plus seven calendar days is 2017-10-30 00:00 CET:
    // the wall clock is preserved and the offset changes.
    let start = Time::new(2017, 10, 23, 0, 0, 0, 0, &berlin());
    let by_calendar = start.add_date(0, 0, 7);
    assert_eq!(
        by_calendar.format("2006-01-02 15:04:05 -0700"),
        "2017-10-30 00:00:00 +0100"
    );

    // Adding an absolute seven days instead keeps the instant and shifts the
    // wall clock by the hour the zone lost.
    let by_duration = start.add(7 * 24 * HOUR);
    assert_eq!(
        by_duration.format("2006-01-02 15:04:05 -0700"),
        "2017-10-29 23:00:00 +0100"
    );
}

#[test]
fn truncate_rounds_the_absolute_time_down() {
    let t = utc(2013, 11, 18, 17, 51, 49, 123456789);
    assert_eq!(t.truncate(MINUTE).format(FULL), "2013-11-18 17:51:00");
    assert_eq!(t.truncate(SECOND).format(FULL), "2013-11-18 17:51:49");
    assert_eq!(t.truncate(HOUR).format(FULL), "2013-11-18 17:00:00");
    // A non-positive duration is a no-op.
    assert_eq!(t.truncate(0).format(FULL), "2013-11-18 17:51:49.123456789");

    // Truncation is absolute, so in a zone offset by a half hour truncating to
    // the hour does not land on the local hour. This is exactly why
    // BeginningOfHour is built with Date rather than Truncate.
    let caracas = Location::load("America/Caracas").expect("America/Caracas");
    let c = Time::new(2016, 1, 1, 12, 10, 0, 0, &caracas);
    assert_eq!(c.truncate(HOUR).format(FULL), "2016-01-01 11:30:00");
}

#[test]
fn add_moves_the_instant_by_nanoseconds() {
    let t = utc(2013, 11, 18, 0, 0, 0, 0);
    assert_eq!(t.add(-NANOSECOND).format(FULL), "2013-11-17 23:59:59.999999999");
    assert_eq!(t.add(MINUTE - NANOSECOND).format(FULL), "2013-11-18 00:00:59.999999999");
}

// --- the reference-layout formatter -----------------------------------------

#[test]
fn format_renders_every_layout_element() {
    let t = utc(2013, 1, 2, 15, 4, 5, 123456789);
    assert_eq!(t.format("2006"), "2013");
    assert_eq!(t.format("06"), "13");
    assert_eq!(t.format("January"), "January");
    assert_eq!(t.format("Jan"), "Jan");
    assert_eq!(t.format("01"), "01");
    assert_eq!(t.format("1"), "1");
    assert_eq!(t.format("Monday"), "Wednesday");
    assert_eq!(t.format("Mon"), "Wed");
    assert_eq!(t.format("02"), "02");
    assert_eq!(t.format("2"), "2");
    assert_eq!(t.format("_2"), " 2");
    assert_eq!(t.format("15"), "15");
    assert_eq!(t.format("3"), "3");
    assert_eq!(t.format("03"), "03");
    assert_eq!(t.format("4"), "4");
    assert_eq!(t.format("04"), "04");
    assert_eq!(t.format("5"), "5");
    assert_eq!(t.format("05"), "05");
    assert_eq!(t.format("PM"), "PM");
    assert_eq!(t.format("pm"), "pm");
    assert_eq!(t.format("002"), "002");
    assert_eq!(t.format("__2"), "  2");
    assert_eq!(t.format("MST"), "UTC");
}

#[test]
fn format_renders_twelve_hour_clocks_and_meridiems() {
    assert_eq!(utc(2013, 1, 1, 0, 4, 0, 0).format("3:04PM"), "12:04AM");
    assert_eq!(utc(2013, 1, 1, 12, 4, 0, 0).format("3:04PM"), "12:04PM");
    assert_eq!(utc(2013, 1, 1, 13, 4, 0, 0).format("3:04PM"), "1:04PM");
    assert_eq!(utc(2013, 1, 1, 9, 4, 0, 0).format("03:04pm"), "09:04am");
}

#[test]
fn format_trims_the_nine_style_fraction_and_pads_the_zero_style() {
    let ns = |n: i64| utc(2013, 1, 1, 0, 0, 0, n);
    // `.9` drops trailing zeros, and vanishes with its separator when zero.
    assert_eq!(ns(0).format("05.999999999"), "00");
    assert_eq!(ns(999_000_000).format("05.999999999"), "00.999");
    assert_eq!(ns(123_456_789).format("05.999999999"), "00.123456789");
    assert_eq!(ns(123_456_789).format("05.999"), "00.123");
    // `.0` is fixed width and always emitted, truncating rather than rounding.
    assert_eq!(ns(0).format("05.000"), "00.000");
    assert_eq!(ns(473_000_000).format("05.000"), "00.473");
    assert_eq!(ns(999_999_999).format("05.000"), "00.999");
    // A comma separator is carried through.
    assert_eq!(ns(473_000_000).format("05,000"), "00,473");
}

#[test]
fn format_renders_numeric_zones() {
    let plus = parse_in_location("2006-01-02T15:04:05Z07:00", "2013-01-02T15:04:05+08:00", &Location::utc())
        .expect("parses");
    assert_eq!(plus.format("-0700"), "+0800");
    assert_eq!(plus.format("-07:00"), "+08:00");
    assert_eq!(plus.format("-07"), "+08");
    assert_eq!(plus.format("Z0700"), "+0800");
    assert_eq!(plus.format("Z07:00"), "+08:00");

    // The `Z` forms render a zero offset as a literal `Z`; the `-` forms do not.
    let zulu = utc(2013, 1, 2, 15, 4, 5, 0);
    assert_eq!(zulu.format("Z0700"), "Z");
    assert_eq!(zulu.format("Z07:00"), "Z");
    assert_eq!(zulu.format("-0700"), "+0000");

    // A half-hour offset keeps its minutes.
    let caracas = Location::load("America/Caracas").expect("America/Caracas");
    let c = Time::new(2016, 1, 1, 12, 10, 0, 0, &caracas);
    assert_eq!(c.format("-0700"), "-0430");
    assert_eq!(c.format("-07:00"), "-04:30");
}

#[test]
fn format_falls_back_to_a_numeric_zone_when_the_name_is_empty() {
    // A fixed zone with no name must still print something for MST.
    let t = utc(2013, 1, 2, 15, 4, 5, 0).in_location(&Location::fixed("", 28800));
    assert_eq!(t.format("MST"), "+0800");
    assert_eq!(t.to_string(), "2013-01-02 23:04:05 +0800 +0800");
}

#[test]
fn display_matches_gos_time_string() {
    assert_eq!(
        utc(2018, 2, 13, 15, 17, 6, 0).to_string(),
        "2018-02-13 15:17:06 +0000 UTC"
    );
    assert_eq!(
        utc(2013, 11, 18, 17, 51, 49, 123456789).to_string(),
        "2013-11-18 17:51:49.123456789 +0000 UTC"
    );
}

// --- the reference-layout parser ---------------------------------------------

fn p(layout: &str, value: &str) -> Result<Time, String> {
    parse_in_location(layout, value, &Location::utc()).map_err(|e| e.to_string())
}

#[test]
fn parse_requires_the_layout_to_consume_the_whole_value() {
    // This is what makes the ordered TimeFormats list resolve unambiguously.
    assert!(p("2006", "2002").is_ok());
    assert!(p("2006", "2002-10").is_err());
    assert!(p("2006-1", "2002-10").is_ok());
    assert!(p("15", "18").is_ok());
    assert!(p("15", "18:20").is_err());
}

#[test]
fn parse_reads_one_or_two_digits_for_unpadded_elements() {
    // "2006-1-2" must accept both one- and two-digit months and days.
    assert_eq!(p("2006-1-2", "2002-10-12").unwrap().format(FULL), "2002-10-12 00:00:00");
    assert_eq!(p("2006-1-2", "2002-1-2").unwrap().format(FULL), "2002-01-02 00:00:00");
    // A padded element requires exactly two digits.
    assert!(p("2006-01-02", "2002-1-2").is_err());
    assert_eq!(p("2006-01-02", "2002-01-02").unwrap().format(FULL), "2002-01-02 00:00:00");
}

#[test]
fn parse_absorbs_a_fraction_the_layout_does_not_declare() {
    // "2006-01-02T15:04:05Z07" has no fractional element, yet Go consumes one.
    let t = p("2006-01-02T15:04:05Z07", "2002-10-12T00:01:12.999999999-06").unwrap();
    assert_eq!(t.format(FULL), "2002-10-12 00:01:12.999999999");
    assert_eq!(t.zone().1, -21600);

    // But when the layout does declare one, the declared element consumes it.
    let t = p("2006-01-02 15:04:05.000", "2018-04-20 21:22:23.473").unwrap();
    assert_eq!(t.format(FULL), "2018-04-20 21:22:23.473");
}

#[test]
fn parse_handles_the_fixed_and_optional_fraction_forms() {
    // `.000` is mandatory and fixed width.
    assert!(p("15:04:05.000", "13:00:01.365").is_ok());
    assert!(p("15:04:05.000", "13:00:01.36").is_err());
    // `.999` is optional.
    assert_eq!(
        p("15:04:05.999999999", "13:00:01").unwrap().format(FULL),
        "0000-01-01 13:00:01"
    );
    assert_eq!(
        p("15:04:05.999999999", "13:00:01.5").unwrap().format(FULL),
        "0000-01-01 13:00:01.5"
    );
}

#[test]
fn parse_defaults_missing_components_to_zero() {
    // A time-only layout yields year 0, January 1 — the zeros `Parse` then
    // folds the receiver's own values onto.
    let t = p("15", "18").unwrap();
    assert_eq!((t.year(), t.month(), t.day()), (0, 1, 1));
    assert_eq!(t.hour(), 18);
}

#[test]
fn parse_reads_two_digit_years_the_way_go_pivots_them() {
    assert_eq!(p("06", "69").unwrap().year(), 1969);
    assert_eq!(p("06", "99").unwrap().year(), 1999);
    assert_eq!(p("06", "00").unwrap().year(), 2000);
    assert_eq!(p("06", "68").unwrap().year(), 2068);
}

#[test]
fn parse_matches_month_and_day_names_case_insensitively_and_ignores_the_weekday() {
    assert_eq!(p("Jan 2 2006", "Feb 4 2013").unwrap().month(), 2);
    assert_eq!(p("Jan 2 2006", "feb 4 2013").unwrap().month(), 2);
    assert_eq!(p("January 2 2006", "February 4 2013").unwrap().month(), 2);
    // Go parses the weekday and never checks it against the date.
    let t = p("Mon Jan 2 2006", "Sun Feb 4 2013").unwrap();
    assert_eq!(t.format(FULL), "2013-02-04 00:00:00");
    assert_eq!(t.weekday(), Weekday::Monday);
}

#[test]
fn parse_validates_component_ranges() {
    assert!(p("2006-01-02", "2013-13-01").is_err());
    assert!(p("2006-01-02", "2013-02-30").is_err());
    assert!(p("2006-01-02", "2012-02-30").is_err());
    assert!(p("2006-01-02", "2012-02-29").is_ok());
    assert!(p("15:04:05", "24:00:00").is_err());
    assert!(p("15:04:05", "23:60:00").is_err());
    assert!(p("15:04:05", "23:00:60").is_err());
}

#[test]
fn parse_reads_a_literal_z_as_utc() {
    let t = p("2006-01-02T15:04:05Z0700", "2002-10-12T00:14:56Z").unwrap();
    assert_eq!(*t.location(), Location::utc());
    assert_eq!(t.format(FULL), "2002-10-12 00:14:56");
}

#[test]
fn parse_keeps_a_numeric_offset_as_a_fixed_zone() {
    let t = p("2006-01-02T15:04:05Z07:00", "2002-10-12T00:14:56+08:00").unwrap();
    assert_eq!(t.zone(), (String::new(), 28800));
    assert_eq!(t.format(FULL), "2002-10-12 00:14:56");

    // With both an offset and a name, the name rides along on the fixed zone.
    let t = p(
        "2006-01-02 15:04:05.999999999 -0700 MST",
        "2013-12-19 23:28:09.999999999 +0800 CST",
    )
    .unwrap();
    assert_eq!(t.zone(), ("CST".to_string(), 28800));
}

#[test]
fn parse_keeps_the_location_when_the_offset_matches_it() {
    // Berlin was at +01:00 in December, so parsing +01:00 in Berlin keeps
    // Berlin rather than fabricating a fixed zone.
    let b = berlin();
    let t = parse_in_location("2006-01-02T15:04:05Z07:00", "2017-12-01T12:00:00+01:00", &b)
        .expect("parses");
    assert_eq!(*t.location(), b);
    assert_eq!(t.zone(), ("CET".to_string(), 3600));
}

#[test]
fn parse_turns_an_unknown_abbreviation_into_a_zero_offset_fixed_zone() {
    let t = p("15:4:5 Jan 2, 2006 MST", "23:28:9 Dec 19, 2013 PST").unwrap();
    assert_eq!(t.location().to_string(), "PST");
    assert_eq!(t.zone(), ("PST".to_string(), 0));
    assert_eq!(t.format(FULL), "2013-12-19 23:28:09");
}

#[test]
fn parse_resolves_an_abbreviation_that_the_location_actually_uses() {
    // CET is Berlin's winter abbreviation, so it resolves to +01:00 rather
    // than to a fabricated zero-offset zone.
    let t = parse_in_location("2006-01-02 15:04:05 MST", "2017-12-01 12:00:00 CET", &berlin())
        .expect("parses");
    assert_eq!(t.zone(), ("CET".to_string(), 3600));
}

#[test]
fn parse_reads_utc_and_gmt_offsets_by_name() {
    let t = p("2006-01-02 15:04:05 MST", "2013-01-02 15:04:05 UTC").unwrap();
    assert_eq!(*t.location(), Location::utc());

    let t = p("2006-01-02 15:04:05 MST", "2013-01-02 15:04:05 GMT+3").unwrap();
    assert_eq!(t.zone(), ("GMT+3".to_string(), 10800));
}

#[test]
fn parse_treats_a_space_in_the_layout_as_a_run_of_spaces() {
    assert!(p("Jan _2 15:04:05", "Jan  2 15:04:05").is_ok());
    assert!(p("Jan _2 15:04:05", "Jan 22 15:04:05").is_ok());
}

#[test]
fn parse_reads_the_layouts_the_default_format_list_carries() {
    let cases: [(&str, &str, &str); 10] = [
        ("Mon Jan _2 15:04:05 2006", "Mon Jan  2 15:04:05 2013", "2013-01-02 15:04:05"),
        ("Mon Jan _2 15:04:05 MST 2006", "Wed Jan  2 15:04:05 UTC 2013", "2013-01-02 15:04:05"),
        ("Mon Jan 02 15:04:05 -0700 2006", "Wed Jan 02 15:04:05 +0000 2013", "2013-01-02 15:04:05"),
        ("02 Jan 06 15:04 MST", "02 Jan 13 15:04 UTC", "2013-01-02 15:04:00"),
        ("02 Jan 06 15:04 -0700", "02 Jan 13 15:04 +0000", "2013-01-02 15:04:00"),
        ("Monday, 02-Jan-06 15:04:05 MST", "Wednesday, 02-Jan-13 15:04:05 UTC", "2013-01-02 15:04:05"),
        ("Mon, 02 Jan 2006 15:04:05 MST", "Wed, 02 Jan 2013 15:04:05 UTC", "2013-01-02 15:04:05"),
        ("3:04PM", "3:04PM", "0000-01-01 15:04:00"),
        ("Jan _2 15:04:05.000", "Jan  2 15:04:05.123", "0000-01-02 15:04:05.123"),
        ("20060102", "20130102", "2013-01-02 00:00:00"),
    ];
    for (layout, value, expected) in cases {
        let t = p(layout, value)
            .unwrap_or_else(|e| panic!("layout {layout:?} value {value:?}: {e}"));
        assert_eq!(t.format(FULL), expected, "layout {layout:?} value {value:?}");
    }
}

// --- locations ---------------------------------------------------------------

#[test]
fn location_display_and_equality_follow_gos_pointer_identity() {
    assert_eq!(Location::load("UTC").unwrap(), Location::utc());
    assert_eq!(Location::load("").unwrap(), Location::utc());
    assert_eq!(Location::utc().to_string(), "UTC");
    assert_eq!(berlin().to_string(), "Europe/Berlin");
    assert_eq!(Location::fixed("PST", 0).to_string(), "PST");
    assert_eq!(Location::local().to_string(), "Local");

    assert_ne!(berlin(), Location::utc());
    assert_ne!(Location::fixed("PST", 0), Location::utc());
    assert_eq!(Location::fixed("PST", 0), Location::fixed("PST", 0));
    assert_ne!(Location::fixed("PST", 0), Location::fixed("PST", 3600));

    assert!(Location::load("Not/AZone").is_err());
}

#[test]
fn in_location_keeps_the_instant_and_changes_the_wall_clock() {
    let t = utc(2013, 1, 2, 15, 4, 5, 0);
    let b = t.in_location(&berlin());
    assert_eq!(t.unix_sec(), b.unix_sec());
    assert_eq!(b.format(FULL), "2013-01-02 16:04:05");
}
