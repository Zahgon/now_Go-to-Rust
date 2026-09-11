use crate::deps::gotime::Time;

/// Decomposes a time into the ordered list `Parse` folds over:
/// `[nanosecond, second, minute, hour, day, month, year]`.
///
/// The order is smallest unit first, which is what lets `Parse` treat "index
/// four and five" as the day and month.
pub(crate) fn format_time_to_list(t: &Time) -> [i64; 7] {
    let (hour, min, sec) = t.clock();
    let (year, month, day) = t.date();
    [t.nanosecond() as i64, sec, min, hour, day, month, year]
}
