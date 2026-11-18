//! Schedules: when a line's amount is due, and its smoothed amount.

// `NonZeroU8` is a u8 the type system guarantees is never 0: a schedule
// "every 0 months" cannot even be built, so no code has to check for it.
use std::num::NonZeroU8;

// `ToSpan` adds methods like `3.months()` or `1.week()` to integers; a
// `Span` is a duration in calendar units (months, weeks, days…).
use jiff::ToSpan;
use jiff::civil::Date;

// `crate::` names a module of this crate from its root, like a fully
// qualified C++ name.
use crate::money::Money;
use crate::month::Month;

/// When a line's amount is due (GLOSSARY.md: Schedule).
///
/// A Rust `enum` is a tagged union, like `std::variant`: a value is exactly
/// one of the variants below, each with its own fields. Unlike a C++ enum,
/// variants carry data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schedule {
    /// Every `every` months on the day of `first`, starting with `first`.
    /// Covers monthly (every 1), every N months, and yearly (every 12).
    /// A 31st falls on the last day of shorter months.
    Months { every: NonZeroU8, first: Date },
    /// Every week on the weekday of `first`, starting with `first`.
    Weekly { first: Date },
    /// Once, on the given date (a one-off line).
    Once { on: Date },
}

/// A period to spread amounts over (GLOSSARY.md: Smoothed amount).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    Week,
    Month,
    Year,
}

impl Period {
    /// How many such periods make a year, counting 52 weeks a year.
    fn per_year(self) -> i64 {
        // `match` is a `switch` that must cover every variant: add a variant
        // to Period and every `match` on it stops compiling until updated.
        // Each arm is an expression, and the `match` returns its value.
        match self {
            Period::Week => 52,
            Period::Month => 12,
            Period::Year => 1,
        }
    }
}

impl Schedule {
    /// The dates in `month` when the amount is due, in order.
    // `Vec<T>` is Rust's `std::vector<T>`.
    pub fn due_dates_in(&self, month: Month) -> Vec<Date> {
        // `match` with fields: each arm *destructures* the variant, binding
        // its fields to local names (like a structured binding).
        // `self` is `&Schedule`, so the bound fields are references too.
        match self {
            Schedule::Months { every, first } => {
                // How many months after the first payment is `month`?
                let offset = Month::of(*first).months_until(month);
                // `.get()` reads the u8 out of the NonZeroU8.
                let every = i32::from(every.get());
                if offset >= 0 && offset % every == 0 {
                    // Add the months to `first` itself, not to the previous
                    // due date, so a 31st clamped to February 28th goes back
                    // to the 31st in March. jiff clamps the day to the end
                    // of the month. `checked_add` fails only far beyond any
                    // real date (year 9999), so we treat that as a bug.
                    let due = first.checked_add(offset.months()).expect("date in range");
                    // `vec![...]` builds a Vec from a list, like an
                    // initializer list.
                    vec![due]
                } else {
                    // `Vec::new()` is an empty vector; it doesn't allocate.
                    Vec::new()
                }
            }
            Schedule::Weekly { first } => first
                // `series` is a lazy, endless iterator: first, first + 1
                // week, first + 2 weeks… Nothing is computed until
                // something asks for the next element, like a C++20 range
                // view.
                .series(1.week())
                // Skip the weeks before the month, then keep the ones in it.
                .skip_while(|date| *date < month.first_day())
                .take_while(|date| *date <= month.last_day())
                // Run the iterator and gather the dates into a Vec.
                .collect(),
            Schedule::Once { on } => {
                if month.contains(*on) {
                    vec![*on]
                } else {
                    Vec::new()
                }
            }
        }
    }

    /// The first due date: the one the others follow from.
    pub fn first(&self) -> Date {
        // `|` joins patterns binding the same name: either variant's date.
        match *self {
            Schedule::Months { first, .. } | Schedule::Weekly { first } => first,
            Schedule::Once { on } => on,
        }
    }

    /// The same repetition, following on from `first` instead.
    pub fn starting(self, first: Date) -> Schedule {
        match self {
            Schedule::Months { every, .. } => Schedule::Months { every, first },
            Schedule::Weekly { .. } => Schedule::Weekly { first },
            Schedule::Once { .. } => Schedule::Once { on: first },
        }
    }

    /// `amount`, due on this schedule, spread evenly over `period`: e.g.
    /// €480 every 12 months is €40 a month. `None` for a one-off line,
    /// which is never smoothed (doc/scope.md).
    pub fn smoothed(&self, amount: Money, period: Period) -> Option<Money> {
        // How often the amount is due in a year, as the fraction
        // `times / years`: every 3 months is 12 / 3 times a year.
        // A `match` can return a tuple, unpacked here into two variables.
        let (times, years) = match self {
            Schedule::Months { every, .. } => (12, i64::from(every.get())),
            Schedule::Weekly { .. } => (52, 1),
            // `return` leaves the function early, like in C++.
            Schedule::Once { .. } => return None,
        };
        // amount × times a year / periods a year, rounded once.
        Some(amount.mul_div_rounded(times, years * period.per_year()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::money::Currency;
    use jiff::civil::date;

    fn eur(cents: i64) -> Money {
        Money::new(cents, Currency::EUR)
    }

    fn month(year: i16, month: i8) -> Month {
        Month::new(year, month).unwrap()
    }

    // `NonZeroU8::new` returns None for 0; `unwrap` is fine in tests.
    fn every(n: u8) -> NonZeroU8 {
        NonZeroU8::new(n).unwrap()
    }

    #[test]
    fn monthly_on_the_31st_falls_on_the_last_day() {
        let rent = Schedule::Months {
            every: every(1),
            first: date(2026, 1, 31),
        };
        assert_eq!(rent.due_dates_in(month(2026, 2)), vec![date(2026, 2, 28)]);
        assert_eq!(rent.due_dates_in(month(2026, 3)), vec![date(2026, 3, 31)]);
        // Nothing before the first payment.
        assert_eq!(rent.due_dates_in(month(2025, 12)), vec![]);
    }

    #[test]
    fn every_two_months_skips_the_months_between() {
        let electricity = Schedule::Months {
            every: every(2),
            first: date(2026, 1, 5),
        };
        assert_eq!(
            electricity.due_dates_in(month(2026, 3)),
            vec![date(2026, 3, 5)]
        );
        assert_eq!(electricity.due_dates_in(month(2026, 4)), vec![]);
    }

    #[test]
    fn yearly_is_every_twelve_months() {
        let insurance = Schedule::Months {
            every: every(12),
            first: date(2026, 3, 10),
        };
        assert_eq!(
            insurance.due_dates_in(month(2027, 3)),
            vec![date(2027, 3, 10)]
        );
        assert_eq!(insurance.due_dates_in(month(2027, 4)), vec![]);
    }

    #[test]
    fn weekly_lists_every_week_in_the_month() {
        // 2026-11-02 is a Monday.
        let market = Schedule::Weekly {
            first: date(2026, 11, 2),
        };
        assert_eq!(
            market.due_dates_in(month(2026, 12)),
            vec![
                date(2026, 12, 7),
                date(2026, 12, 14),
                date(2026, 12, 21),
                date(2026, 12, 28)
            ]
        );
    }

    #[test]
    fn once_is_due_in_its_month_only() {
        let repair = Schedule::Once {
            on: date(2026, 6, 15),
        };
        assert_eq!(repair.due_dates_in(month(2026, 6)), vec![date(2026, 6, 15)]);
        assert_eq!(repair.due_dates_in(month(2026, 7)), vec![]);
    }

    #[test]
    fn smooths_over_any_period() {
        let insurance = Schedule::Months {
            every: every(12),
            first: date(2026, 3, 10),
        };
        assert_eq!(
            insurance.smoothed(eur(48000), Period::Month),
            Some(eur(4000))
        );
        assert_eq!(
            insurance.smoothed(eur(48000), Period::Year),
            Some(eur(48000))
        );

        let electricity = Schedule::Months {
            every: every(2),
            first: date(2026, 1, 5),
        };
        assert_eq!(
            electricity.smoothed(eur(12000), Period::Month),
            Some(eur(6000))
        );

        // €10 a week is 52 × 10 / 12 = €43.33 a month.
        let market = Schedule::Weekly {
            first: date(2026, 11, 2),
        };
        assert_eq!(market.smoothed(eur(1000), Period::Month), Some(eur(4333)));
        assert_eq!(market.smoothed(eur(1000), Period::Week), Some(eur(1000)));
    }

    #[test]
    fn one_off_lines_are_not_smoothed() {
        let repair = Schedule::Once {
            on: date(2026, 6, 15),
        };
        assert_eq!(repair.smoothed(eur(120000), Period::Month), None);
    }
}
