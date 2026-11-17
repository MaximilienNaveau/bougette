//! Lines: the planned, recurring incomes and costs of a budget.

use jiff::civil::Date;

use crate::id::{LineId, MemberId, PotId};
use crate::money::Money;
use crate::month::Month;
use crate::schedule::{Period, Schedule};

/// Whether a line, or a transaction, brings money in or takes it out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Income,
    Cost,
}

/// How to count a line in a month: what is actually due that month, or
/// its smoothed amount (GLOSSARY.md: Smoothed amount).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// The amounts due on the dates of the month: €480 in March, €0 in
    /// April.
    Due,
    /// The smoothed amount: €40 every month. One-off lines aren't smoothed
    /// and count in their month only.
    Smoothed,
}

/// A line's amount and schedule from a given date on, until the next
/// revision (GLOSSARY.md: Revision).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revision {
    // `pub` fields can be read and written from outside, like a C++
    // struct's public members.
    pub from: Date,
    /// Always positive: `Kind` says which way the money goes.
    pub amount: Money,
    pub schedule: Schedule,
}

/// A planned, recurring income or cost (GLOSSARY.md: Line).
///
/// `Clone` without `Copy`: a Line owns heap memory (its Strings and Vec),
/// so copying it is an explicit, possibly costly `.clone()`, like a deep
/// copy constructor, and `let b = a;` moves it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub id: LineId,
    // `String` is an owned, growable UTF-8 string, like `std::string`.
    pub name: String,
    pub category: String,
    pub kind: Kind,
    /// For an allocation, the pot it feeds (GLOSSARY.md: Allocation).
    pub pot: Option<PotId>,
    /// The member who pays it with their own money, as for a transaction
    /// (GLOSSARY.md: Payer): an income paid by a member is their planned
    /// contribution, shown as a cost line in their personal budget (see
    /// `shared::reflections`). `None`: the budget's own money.
    pub payer: Option<MemberId>,
    // Private: `change_from` keeps them sorted by date, and there is always
    // at least one.
    revisions: Vec<Revision>,
    // The last day the line applies; `None` while it goes on.
    ends_on: Option<Date>,
}

/// Why a change to a line was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineError {
    /// The date is before the line starts.
    BeforeStart,
    /// The amount is zero or negative.
    NotPositive,
}

impl Line {
    /// A new line, valid from `from` with no end date.
    ///
    /// Returns `Err(LineError::NotPositive)` if `amount` isn't positive.
    // `impl Into<String>` accepts anything convertible to a String (a
    // `&str` literal or a String): a generic parameter, like a C++ template
    // constrained by a concept.
    pub fn new(
        name: impl Into<String>,
        category: impl Into<String>,
        kind: Kind,
        from: Date,
        amount: Money,
        schedule: Schedule,
    ) -> Result<Line, LineError> {
        // `Result<T, E>` is either `Ok(value)` or `Err(error)`: the error
        // is part of the return type instead of being thrown.
        if amount.minor_units() <= 0 {
            return Err(LineError::NotPositive);
        }
        Ok(Line {
            id: LineId::new(),
            name: name.into(),
            category: category.into(),
            kind,
            pot: None,
            payer: None,
            revisions: vec![Revision {
                from,
                amount,
                schedule,
            }],
            ends_on: None,
        })
    }

    /// The first day the line applies.
    pub fn starts_on(&self) -> Date {
        // `[0]` panics if the Vec is empty; `new` guarantees it isn't.
        self.revisions[0].from
    }

    /// The last day the line applies, if it ends.
    pub fn ends_on(&self) -> Option<Date> {
        self.ends_on
    }

    /// Its revisions, oldest first.
    // `&[Revision]` is a "slice": a borrowed view of contiguous elements,
    // like `std::span<const Revision>`.
    pub fn revisions(&self) -> &[Revision] {
        &self.revisions
    }

    /// From `from` on, the line has a new amount and schedule; months
    /// before keep the old ones (doc/scope.md). Revisions from `from` or
    /// later are replaced.
    // `&mut self`: the method changes the line, like a non-const member
    // function. The borrow checker guarantees nobody else is reading the
    // line while it changes, so no iterator over `revisions` can be
    // invalidated behind our back.
    pub fn change_from(
        &mut self,
        from: Date,
        amount: Money,
        schedule: Schedule,
    ) -> Result<(), LineError> {
        if from < self.starts_on() {
            return Err(LineError::BeforeStart);
        }
        if amount.minor_units() <= 0 {
            return Err(LineError::NotPositive);
        }
        // Keep only the revisions that started before `from`. `retain` is
        // the erase-remove idiom in one call.
        self.revisions.retain(|revision| revision.from < from);
        self.revisions.push(Revision {
            from,
            amount,
            schedule,
        });
        // `Ok(())`: success with nothing to return (`()` is like `void`).
        Ok(())
    }

    /// The line stops after `last_day`, or goes on again with `None`.
    pub fn end_on(&mut self, last_day: Option<Date>) -> Result<(), LineError> {
        // `if let` runs the block only when the pattern matches: here, when
        // `last_day` is `Some(day)`, binding `day`. `&&` then adds a
        // condition on `day` (a "let chain", new in edition 2024).
        if let Some(day) = last_day
            && day < self.starts_on()
        {
            return Err(LineError::BeforeStart);
        }
        self.ends_on = last_day;
        Ok(())
    }

    /// The revision in force on `date`, or `None` outside the line's dates.
    pub fn revision_on(&self, date: Date) -> Option<&Revision> {
        // `is_some_and` is true if there is an end date and the test holds.
        if date < self.starts_on() || self.ends_on.is_some_and(|end| date > end) {
            return None;
        }
        // The last revision starting on or before `date`. `.rev()` walks the
        // Vec backwards and `.find(...)` stops at the first match, giving
        // an `Option<&Revision>`, like `std::find_if` on reverse iterators.
        self.revisions
            .iter()
            .rev()
            .find(|revision| revision.from <= date)
    }

    /// What the line amounts to in `month`, always positive or zero (see
    /// `signed_amount_in` for the sign).
    pub fn amount_in(&self, month: Month, view: View) -> Money {
        let currency = self.revisions[0].amount.currency();
        match view {
            View::Due => self
                .revisions
                .iter()
                // For each revision, the due dates of its schedule in the
                // month, kept only where that revision is the one in force.
                // `flat_map` concatenates the per-revision lists.
                .flat_map(|revision| {
                    revision
                        .schedule
                        .due_dates_in(month)
                        .into_iter()
                        // `move` makes the closure take its own copy of
                        // `revision` (a reference), so it can outlive this
                        // iteration step.
                        // `std::ptr::eq` compares addresses, not contents:
                        // is the revision in force this very one?
                        .filter(move |date| {
                            self.revision_on(*date)
                                .is_some_and(|in_force| std::ptr::eq(in_force, revision))
                        })
                        .map(move |_| revision.amount)
                })
                // Sum the amounts, starting from zero. `fold` is
                // `std::accumulate`.
                .fold(Money::zero(currency), |total, amount| total + amount),
            View::Smoothed => {
                // The line counts from the later of its start and the 1st.
                let day = self.starts_on().max(month.first_day());
                if day > month.last_day() {
                    return Money::zero(currency); // it starts after this month
                }
                // `let ... else`: bind the revision, or leave the function
                // with zero when there is none (the line has ended).
                let Some(revision) = self.revision_on(day) else {
                    return Money::zero(currency);
                };
                // A one-off line has no smoothed amount: count what's due.
                // `unwrap_or_else` runs the closure only when there is None.
                revision
                    .schedule
                    .smoothed(revision.amount, Period::Month)
                    .unwrap_or_else(|| self.amount_in(month, View::Due))
            }
        }
    }

    /// Like `amount_in`, but negative for a cost: what it does to the
    /// budget's balance.
    pub fn signed_amount_in(&self, month: Month, view: View) -> Money {
        let amount = self.amount_in(month, view);
        match self.kind {
            Kind::Income => amount,
            Kind::Cost => -amount,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::money::Currency;
    use jiff::civil::date;
    use std::num::NonZeroU8;

    fn eur(cents: i64) -> Money {
        Money::new(cents, Currency::EUR)
    }

    fn month(year: i16, month: i8) -> Month {
        Month::new(year, month).unwrap()
    }

    fn monthly(first: Date) -> Schedule {
        Schedule::Months {
            every: NonZeroU8::MIN, // the smallest NonZeroU8: 1
            first,
        }
    }

    fn rent() -> Line {
        Line::new(
            "Rent",
            "Housing",
            Kind::Cost,
            date(2026, 1, 1),
            eur(90000),
            monthly(date(2026, 1, 5)),
        )
        .unwrap()
    }

    #[test]
    fn refuses_amounts_that_are_not_positive() {
        let line = Line::new(
            "Rent",
            "Housing",
            Kind::Cost,
            date(2026, 1, 1),
            eur(0),
            monthly(date(2026, 1, 5)),
        );
        assert_eq!(line, Err(LineError::NotPositive));
    }

    #[test]
    fn a_change_applies_from_its_date_only() {
        let mut line = rent();
        line.change_from(date(2026, 7, 1), eur(95000), monthly(date(2026, 7, 5)))
            .unwrap();
        assert_eq!(line.amount_in(month(2026, 6), View::Due), eur(90000));
        assert_eq!(line.amount_in(month(2026, 7), View::Due), eur(95000));
        assert_eq!(line.amount_in(month(2026, 7), View::Smoothed), eur(95000));
        // A change before the line starts makes no sense.
        assert_eq!(
            line.change_from(date(2025, 12, 1), eur(1), monthly(date(2026, 1, 5))),
            Err(LineError::BeforeStart)
        );
    }

    #[test]
    fn a_later_change_from_an_earlier_date_replaces_it() {
        let mut line = rent();
        line.change_from(date(2026, 7, 1), eur(95000), monthly(date(2026, 7, 5)))
            .unwrap();
        line.change_from(date(2026, 4, 1), eur(92000), monthly(date(2026, 4, 5)))
            .unwrap();
        assert_eq!(line.revisions().len(), 2);
        assert_eq!(line.amount_in(month(2026, 8), View::Due), eur(92000));
    }

    #[test]
    fn counts_nothing_outside_its_dates() {
        let mut line = rent();
        line.end_on(Some(date(2026, 3, 31))).unwrap();
        assert_eq!(line.amount_in(month(2025, 12), View::Due), eur(0));
        assert_eq!(line.amount_in(month(2025, 12), View::Smoothed), eur(0));
        assert_eq!(line.amount_in(month(2026, 3), View::Smoothed), eur(90000));
        assert_eq!(line.amount_in(month(2026, 4), View::Due), eur(0));
        assert_eq!(line.amount_in(month(2026, 4), View::Smoothed), eur(0));
    }

    #[test]
    fn yearly_cost_is_due_once_but_smoothed_every_month() {
        let insurance = Line::new(
            "Home insurance",
            "Housing",
            Kind::Cost,
            date(2026, 1, 1),
            eur(48000),
            Schedule::Months {
                every: NonZeroU8::new(12).unwrap(),
                first: date(2026, 3, 10),
            },
        )
        .unwrap();
        assert_eq!(insurance.amount_in(month(2026, 3), View::Due), eur(48000));
        assert_eq!(insurance.amount_in(month(2026, 4), View::Due), eur(0));
        assert_eq!(
            insurance.amount_in(month(2026, 4), View::Smoothed),
            eur(4000)
        );
        assert_eq!(
            insurance.signed_amount_in(month(2026, 4), View::Smoothed),
            eur(-4000)
        );
    }

    #[test]
    fn one_off_counts_in_its_month_even_when_smoothed() {
        let repair = Line::new(
            "Car repair",
            "Transport",
            Kind::Cost,
            date(2026, 6, 1),
            eur(120000),
            Schedule::Once {
                on: date(2026, 6, 15),
            },
        )
        .unwrap();
        assert_eq!(
            repair.amount_in(month(2026, 6), View::Smoothed),
            eur(120000)
        );
        assert_eq!(repair.amount_in(month(2026, 7), View::Smoothed), eur(0));
    }
}
