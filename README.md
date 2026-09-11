## Now

Now is a time toolkit for rust

[![crates.io](https://img.shields.io/crates/v/now.svg "crates.io")](https://crates.io/crates/now)
[![test status](https://github.com/jinzhu/now/workflows/tests/badge.svg?branch=master "test status")](https://github.com/jinzhu/now/actions)
[![MIT license](https://img.shields.io/badge/license-MIT-brightgreen.svg)](https://opensource.org/licenses/MIT)

## Install

```
cargo add now
```

## Usage

Calculating time based on current time

```rust
use now::Weekday;

now::Time::now(); // 2013-11-18 17:51:49.123456789 Mon

now::beginning_of_minute();        // 2013-11-18 17:51:00 Mon
now::beginning_of_hour();          // 2013-11-18 17:00:00 Mon
now::beginning_of_day();           // 2013-11-18 00:00:00 Mon
now::beginning_of_week();          // 2013-11-17 00:00:00 Sun
now::beginning_of_month();         // 2013-11-01 00:00:00 Fri
now::beginning_of_quarter();       // 2013-10-01 00:00:00 Tue
now::beginning_of_year();          // 2013-01-01 00:00:00 Tue

now::end_of_minute();              // 2013-11-18 17:51:59.999999999 Mon
now::end_of_hour();                // 2013-11-18 17:59:59.999999999 Mon
now::end_of_day();                 // 2013-11-18 23:59:59.999999999 Mon
now::end_of_week();                // 2013-11-23 23:59:59.999999999 Sat
now::end_of_month();               // 2013-11-30 23:59:59.999999999 Sat
now::end_of_quarter();             // 2013-12-31 23:59:59.999999999 Tue
now::end_of_year();                // 2013-12-31 23:59:59.999999999 Tue

now::set_week_start_day(Weekday::Monday); // Set Monday as first day, default is Sunday
now::end_of_week();                       // 2013-11-24 23:59:59.999999999 Sun
```

Calculating time based on another time

```rust
use now::{Location, Time};

let t = Time::new(2013, 2, 18, 17, 51, 49, 123456789, &Location::local());
now::with(t).end_of_month();   // 2013-02-28 23:59:59.999999999 Thu
```

Calculating time based on configuration

```rust
use now::{Config, Location, Time, Weekday};

let location = Location::load("Asia/Shanghai").unwrap();

let my_config = Config {
    week_start_day: Weekday::Monday,
    time_location: Some(location),
    time_formats: vec!["2006-01-02 15:04:05".to_string()],
};

// 2013-11-18 17:51:49.123456789 Mon
let t = Time::new(2013, 11, 18, 17, 51, 49, 123456789, &Location::local());
my_config.with(t).beginning_of_week();       // 2013-11-18 00:00:00 Mon

my_config.parse(&["2002-10-12 22:14:01"]);   // 2002-10-12 22:14:01
my_config.parse(&["2002-10-12 22:14"]);      // returns error 'Can't parse string as time: 2002-10-12 22:14'
```

### Monday/Sunday

Don't be bothered with the week start day setting, you can use `monday`, `sunday`

```rust
now::monday(&[]);            // 2013-11-18 00:00:00 Mon
now::monday(&["17:44"]);     // 2013-11-18 17:44:00 Mon
now::sunday(&[]);            // 2013-11-24 00:00:00 Sun (Next Sunday)
now::sunday(&["18:19:24"]);  // 2013-11-24 18:19:24 Sun (Next Sunday)
now::end_of_sunday();        // 2013-11-24 23:59:59.999999999 Sun (End of next Sunday)

// 2013-11-24 17:51:49.123456789 Sun
let t = Time::new(2013, 11, 24, 17, 51, 49, 123456789, &Location::local());
now::with(t.clone()).monday(&[]);           // 2013-11-18 00:00:00 Mon (Last Monday if today is Sunday)
now::with(t.clone()).monday(&["17:44"]);    // 2013-11-18 17:44:00 Mon (Last Monday if today is Sunday)
now::with(t.clone()).sunday(&[]);           // 2013-11-24 00:00:00 Sun (Beginning Of Today if today is Sunday)
now::with(t.clone()).sunday(&["18:19:24"]); // 2013-11-24 18:19:24 Sun (Beginning Of Today if today is Sunday)
now::with(t).end_of_sunday();               // 2013-11-24 23:59:59.999999999 Sun (End of Today if today is Sunday)
```

### Parse String to Time

Time formats are [Go reference layouts](https://pkg.go.dev/time#pkg-constants),
written out with the reference time `Mon Jan 2 15:04:05 MST 2006`.

```rust
now::Time::now(); // 2013-11-18 17:51:49.123456789 Mon

// parse(&[&str]) -> Result<Time, ParseError>
now::parse(&["2017"]);                // Ok(2017-01-01 00:00:00)
now::parse(&["2017-10"]);             // Ok(2017-10-01 00:00:00)
now::parse(&["2017-10-13"]);          // Ok(2017-10-13 00:00:00)
now::parse(&["1999-12-12 12"]);       // Ok(1999-12-12 12:00:00)
now::parse(&["1999-12-12 12:20"]);    // Ok(1999-12-12 12:20:00)
now::parse(&["1999-12-12 12:20:21"]); // Ok(1999-12-12 12:20:21)
now::parse(&["10-13"]);               // Ok(2013-10-13 00:00:00)
now::parse(&["12:20"]);               // Ok(2013-11-18 12:20:00)
now::parse(&["12:20:13"]);            // Ok(2013-11-18 12:20:13)
now::parse(&["14"]);                  // Ok(2013-11-18 14:00:00)
now::parse(&["99:99"]);               // Err(Can't parse string as time: 99:99)

// must_parse must parse string to time or it will panic
now::must_parse(&["2013-01-13"]);       // 2013-01-13 00:00:00
now::must_parse(&["02-17"]);            // 2013-02-17 00:00:00
now::must_parse(&["2-17"]);             // 2013-02-17 00:00:00
now::must_parse(&["8"]);                // 2013-11-18 08:00:00
now::must_parse(&["2002-10-12 22:14"]); // 2002-10-12 22:14:00
now::must_parse(&["99:99"]);            // panic: Can't parse string as time: 99:99
```

Extend `now` to support more formats is quite easy, just update the time formats
with other time layouts, e.g:

```rust
now::append_time_format("02 Jan 2006 15:04");
```

Please send me pull requests if you want a format to be supported officially

## Contributing

You can help to make the project better, check out [http://gorm.io/contribute.html](http://gorm.io/contribute.html) for things you can do.

# Author

**jinzhu**

* <http://github.com/jinzhu>
* <wosmvp@gmail.com>
* <http://twitter.com/zhangjinzhu>

## License

Released under the [MIT License](http://www.opensource.org/licenses/MIT).
