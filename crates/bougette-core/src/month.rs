//! A calendar month, from the 1st to its last day: the unit of every
//! budget view (doc/scope.md: a month always starts on the 1st).

// `jiff::civil::Date` is a calendar date with no time and no time zone.
use jiff::civil::Date;

/// A calendar month, e.g. March 2026.
///
/// `PartialOrd, Ord` give `<`, `>` and sorting (like `operator<=>`);
/// months compare by year, then by month, because that's the field order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Month {
    // The month is stored as its first day: the field is private, so the
    // day is always the 1st.
    first_day: Date,
}

impl Month {
    /// The month containing `date`.
    pub fn of(date: Date) -> Month {
        Month {
            first_day: date.first_of_month(),
        }
    }

    /// The month `month` (1 to 12) of `year`, or `None` if it doesn't exist.
    // `i16` and `i8` are the integer types jiff uses for years and months.
    pub fn new(year: i16, month: i8) -> Option<Month> {
        // `Date::new` returns a `Result`: either `Ok(date)` or `Err(error)`.
        // Rust has no exceptions: an operation that can fail returns a
        // `Result`, and the caller must deal with it. `.ok()` turns it into
        // an Option, dropping the error, and `.map(...)` wraps the date in a
        // Month when there is one.
        Date::new(year, month, 1).ok().map(Month::of)
    }

    /// Its first day, the 1st.
    pub fn first_day(&self) -> Date {
        self.first_day
    }

    /// Its last day, e.g. February 28th or 29th.
    pub fn last_day(&self) -> Date {
        self.first_day.last_of_month()
    }

    /// Whether `date` falls in this month.
    pub fn contains(&self, date: Date) -> bool {
        Month::of(date) == *self // `*self` dereferences `&Month` to compare values
    }

    /// The number of months from `self` to `later`, negative if `later` is
    /// before `self`. From January to March 2026 is 2.
    pub fn months_until(&self, later: Month) -> i32 {
        // Count months since year 0 for both and subtract. `i32::from`
        // widens losslessly (an implicit conversion in C++, explicit here).
        let index = |m: &Month| i32::from(m.first_day.year()) * 12 + i32::from(m.first_day.month());
        index(&later) - index(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // `date(y, m, d)` builds a Date and panics on an invalid one: handy in
    // tests, where an invalid literal is a bug in the test itself.
    use jiff::civil::date;

    #[test]
    fn spans_the_first_to_the_last_day() {
        let february = Month::new(2028, 2).unwrap(); // a leap year
        assert_eq!(february.first_day(), date(2028, 2, 1));
        assert_eq!(february.last_day(), date(2028, 2, 29));
        assert!(february.contains(date(2028, 2, 15)));
        assert!(!february.contains(date(2028, 3, 1)));
        assert_eq!(Month::new(2026, 13), None);
    }

    #[test]
    fn counts_months_across_years() {
        let november = Month::new(2026, 11).unwrap();
        assert_eq!(november.months_until(Month::new(2027, 2).unwrap()), 3);
        assert_eq!(november.months_until(Month::new(2026, 8).unwrap()), -3);
    }
}
