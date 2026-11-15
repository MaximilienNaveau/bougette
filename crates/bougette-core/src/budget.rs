//! Budgets and their monthly plan.

use crate::line::{Kind, Line, View};
use crate::money::{Currency, Money};
use crate::month::Month;

/// A budget (GLOSSARY.md: Budget). For now, only its plan: its lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Budget {
    pub name: String,
    // Private: fixed when the budget is created (doc/scope.md).
    currency: Currency,
    // Private: `add_line` checks each line's currency.
    lines: Vec<Line>,
}

/// A month of a budget's plan: what comes in and what goes out, both
/// positive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonthPlan {
    pub incomes: Money,
    pub costs: Money,
}

impl MonthPlan {
    /// Incomes minus costs: a surplus if positive, a deficit if negative.
    /// With `View::Smoothed`, it is the budget's monthly balance
    /// (GLOSSARY.md: Monthly balance).
    pub fn balance(&self) -> Money {
        self.incomes - self.costs
    }
}

impl Budget {
    /// An empty budget in `currency`.
    pub fn new(name: impl Into<String>, currency: Currency) -> Budget {
        Budget {
            name: name.into(),
            currency,
            lines: Vec::new(),
        }
    }

    pub fn currency(&self) -> Currency {
        self.currency
    }

    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    /// Adds a line. Panics if its amounts aren't in the budget's currency:
    /// callers build amounts from `currency()`, so that would be a bug.
    pub fn add_line(&mut self, line: Line) {
        // `.all(...)` over the revisions' currencies.
        assert!(
            line.revisions()
                .iter()
                .all(|revision| revision.amount.currency() == self.currency),
            "line in another currency than its budget"
        );
        // `push` takes the line by value: it is *moved* into the Vec, and
        // the caller can no longer use it (the compiler enforces it).
        self.lines.push(line);
    }

    /// The plan of `month`: total incomes and total costs of the lines.
    pub fn plan(&self, month: Month, view: View) -> MonthPlan {
        // `mut`: Rust variables are immutable (const) unless declared `mut`.
        let mut plan = MonthPlan {
            incomes: Money::zero(self.currency),
            costs: Money::zero(self.currency),
        };
        // `for x in &v` iterates by reference, like `for (const auto& x : v)`.
        for line in &self.lines {
            let amount = line.amount_in(month, view);
            match line.kind {
                // `+=` isn't defined for Money (we only implemented `+`),
                // so write it out.
                Kind::Income => plan.incomes = plan.incomes + amount,
                Kind::Cost => plan.costs = plan.costs + amount,
            }
        }
        plan
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schedule::Schedule;
    use jiff::civil::date;
    use std::num::NonZeroU8;

    fn eur(cents: i64) -> Money {
        Money::new(cents, Currency::EUR)
    }

    fn every(n: u8, first: jiff::civil::Date) -> Schedule {
        Schedule::Months {
            every: NonZeroU8::new(n).unwrap(),
            first,
        }
    }

    /// The household of GLOSSARY.md: €2,500 of contributions a month, rent,
    /// electricity every two months and a yearly insurance.
    fn household() -> Budget {
        let start = date(2026, 1, 1);
        let mut budget = Budget::new("Household", Currency::EUR);
        // A small helper closure: build a line or fail the test.
        let line = |name, kind, cents, schedule| {
            Line::new(name, "Housing", kind, start, eur(cents), schedule).unwrap()
        };
        budget.add_line(line(
            "Contributions",
            Kind::Income,
            250000,
            every(1, date(2026, 1, 1)),
        ));
        budget.add_line(line("Rent", Kind::Cost, 90000, every(1, date(2026, 1, 5))));
        budget.add_line(line(
            "Electricity",
            Kind::Cost,
            12000,
            every(2, date(2026, 1, 15)),
        ));
        budget.add_line(line(
            "Home insurance",
            Kind::Cost,
            48000,
            every(12, date(2026, 3, 10)),
        ));
        budget
    }

    #[test]
    fn due_view_follows_the_calendar() {
        let march = household().plan(Month::new(2026, 3).unwrap(), View::Due);
        // Rent + electricity + insurance, all due in March.
        assert_eq!(march.costs, eur(90000 + 12000 + 48000));
        assert_eq!(march.balance(), eur(250000 - 150000));

        let april = household().plan(Month::new(2026, 4).unwrap(), View::Due);
        assert_eq!(april.costs, eur(90000));
    }

    #[test]
    fn smoothed_view_is_the_same_every_month() {
        let budget = household();
        // Rent 900 + electricity 120 / 2 + insurance 480 / 12 = 1,000.
        for m in 1..=12 {
            let plan = budget.plan(Month::new(2026, m).unwrap(), View::Smoothed);
            assert_eq!(plan.costs, eur(100000));
            assert_eq!(plan.balance(), eur(150000));
        }
    }

    #[test]
    #[should_panic(expected = "another currency")]
    fn refuses_lines_in_another_currency() {
        let chf = Currency::from_code("CHF").unwrap();
        let mut budget = Budget::new("Household", Currency::EUR);
        budget.add_line(
            Line::new(
                "Rent",
                "Housing",
                Kind::Cost,
                date(2026, 1, 1),
                Money::new(100, chf),
                every(1, date(2026, 1, 5)),
            )
            .unwrap(),
        );
    }
}
