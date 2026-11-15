//! Budgets: their plan, their actual transactions, and how the two compare.

use jiff::civil::Date;

use crate::id::{LineId, TransactionId};
use crate::line::{Kind, Line, View};
use crate::money::{Currency, Money};
use crate::month::Month;
use crate::transaction::Transaction;

/// A budget (GLOSSARY.md: Budget): its lines (the plan) and its
/// transactions (the actual).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Budget {
    pub name: String,
    // Private: fixed when the budget is created (doc/scope.md).
    currency: Currency,
    // Private: `add_line` and `add_transaction` check what goes in.
    lines: Vec<Line>,
    transactions: Vec<Transaction>,
}

/// What comes in and what goes out over a month, both counted positive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonthTotals {
    pub incomes: Money,
    pub costs: Money,
}

impl MonthTotals {
    fn zero(currency: Currency) -> MonthTotals {
        MonthTotals {
            incomes: Money::zero(currency),
            costs: Money::zero(currency),
        }
    }

    /// Adds `amount` to the incomes or to the costs, depending on `kind`.
    fn add(&mut self, kind: Kind, amount: Money) {
        match kind {
            Kind::Income => self.incomes += amount,
            Kind::Cost => self.costs += amount,
        }
    }

    /// Incomes minus costs: a surplus if positive, a deficit if negative.
    /// For the smoothed plan, it is the budget's monthly balance
    /// (GLOSSARY.md: Monthly balance).
    pub fn balance(&self) -> Money {
        self.incomes - self.costs
    }
}

/// Planned and actual amounts of a line or a category over a month.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Comparison {
    pub planned: Money,
    pub actual: Money,
}

/// How a cost compares with its plan: the colour shown (doc/scope.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Green: under 90% of the plan.
    Under,
    /// Yellow or orange: from 90% to 100% of the plan.
    Close,
    /// Red: more than planned.
    Over,
}

impl Comparison {
    /// Actual minus planned (GLOSSARY.md: Variance). For a cost, positive
    /// means overspent.
    pub fn variance(&self) -> Money {
        self.actual - self.planned
    }

    /// The colour of a cost compared with its plan.
    pub fn status(&self) -> Status {
        // In 128 bits, so multiplying by 10 can't overflow.
        let planned = i128::from(self.planned.minor_units());
        let actual = i128::from(self.actual.minor_units());
        if actual > planned {
            Status::Over
        } else if planned > 0 && actual * 10 >= planned * 9 {
            Status::Close
        } else {
            Status::Under
        }
    }
}

/// Why a transaction was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetError {
    /// The amount isn't in the budget's currency.
    WrongCurrency,
    /// The transaction points to a line this budget doesn't have.
    UnknownLine,
    /// An income transaction on a cost line, or the reverse.
    KindMismatch,
}

impl Budget {
    /// An empty budget in `currency`.
    pub fn new(name: impl Into<String>, currency: Currency) -> Budget {
        Budget {
            name: name.into(),
            currency,
            lines: Vec::new(),
            transactions: Vec::new(),
        }
    }

    pub fn currency(&self) -> Currency {
        self.currency
    }

    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    pub fn transactions(&self) -> &[Transaction] {
        &self.transactions
    }

    /// The line with this identifier, if the budget has it.
    pub fn line(&self, id: LineId) -> Option<&Line> {
        self.lines.iter().find(|line| line.id == id)
    }

    /// Adds a line and returns its identifier. Panics if its amounts aren't
    /// in the budget's currency: callers build amounts from `currency()`,
    /// so that would be a bug.
    pub fn add_line(&mut self, line: Line) -> LineId {
        assert!(
            line.revisions()
                .iter()
                .all(|revision| revision.amount.currency() == self.currency),
            "line in another currency than its budget"
        );
        let id = line.id;
        // `push` takes the line by value: it is *moved* into the Vec, and
        // the caller can no longer use it (the compiler enforces it).
        self.lines.push(line);
        id
    }

    /// Adds a transaction after checking its currency and its line.
    pub fn add_transaction(
        &mut self,
        transaction: Transaction,
    ) -> Result<TransactionId, BudgetError> {
        if transaction.amount.currency() != self.currency {
            return Err(BudgetError::WrongCurrency);
        }
        if let Some(line_id) = transaction.line {
            // `ok_or` turns an Option into a Result: `None` becomes the
            // given error. The `?` operator then returns that error from
            // the function, or unwraps the `Ok` value. It replaces the
            // "check and return early" boilerplate that exceptions hide.
            let line = self.line(line_id).ok_or(BudgetError::UnknownLine)?;
            if line.kind != transaction.kind {
                return Err(BudgetError::KindMismatch);
            }
        }
        let id = transaction.id;
        self.transactions.push(transaction);
        Ok(id)
    }

    /// The plan of `month`: total incomes and total costs of the lines.
    pub fn plan(&self, month: Month, view: View) -> MonthTotals {
        // `mut`: Rust variables are immutable (const) unless declared `mut`.
        let mut totals = MonthTotals::zero(self.currency);
        // `for x in &v` iterates by reference, like `for (const auto& x : v)`.
        for line in &self.lines {
            totals.add(line.kind, line.amount_in(month, view));
        }
        totals
    }

    /// The actual of `month`: total incomes and costs of its transactions.
    pub fn actual(&self, month: Month) -> MonthTotals {
        let mut totals = MonthTotals::zero(self.currency);
        for transaction in self.transactions_in(month) {
            totals.add(transaction.kind, transaction.amount);
        }
        totals
    }

    /// A line's plan and actual over `month`, or `None` for an unknown line.
    pub fn compare_line(&self, id: LineId, month: Month) -> Option<Comparison> {
        // `?` works on Options too: return `None` if there is no such line.
        let line = self.line(id)?;
        Some(Comparison {
            planned: line.amount_in(month, View::Due),
            actual: self.line_actual(id, month),
        })
    }

    /// The plan and actual of a category over `month`, for one kind. The
    /// actual includes the unplanned transactions, which have no line.
    pub fn compare_category(&self, kind: Kind, category: &str, month: Month) -> Comparison {
        let mut comparison = Comparison {
            planned: Money::zero(self.currency),
            actual: Money::zero(self.currency),
        };
        for line in &self.lines {
            if line.kind == kind && line.category == category {
                comparison.planned += line.amount_in(month, View::Due);
            }
        }
        for transaction in self.transactions_in(month) {
            if transaction.kind == kind && transaction.category == category {
                comparison.actual += transaction.amount;
            }
        }
        comparison
    }

    /// The expected totals of `month`, seen from `today` (GLOSSARY.md:
    /// Forecast):
    /// - a past month is its actual;
    /// - a future month is its plan;
    /// - the current month counts, for each line, the larger of what was
    ///   planned and what already happened, plus the unplanned
    ///   transactions.
    pub fn forecast(&self, month: Month, today: Date) -> MonthTotals {
        let current = Month::of(today);
        if month < current {
            return self.actual(month);
        }
        if month > current {
            return self.plan(month, View::Due);
        }
        let mut totals = MonthTotals::zero(self.currency);
        for line in &self.lines {
            let planned = line.amount_in(month, View::Due);
            let actual = self.line_actual(line.id, month);
            // Groceries: €250 spent of €600 planned still forecasts €600;
            // €650 spent forecasts €650.
            let expected = if actual.minor_units() > planned.minor_units() {
                actual
            } else {
                planned
            };
            totals.add(line.kind, expected);
        }
        for transaction in self.transactions_in(month) {
            if transaction.line.is_none() {
                totals.add(transaction.kind, transaction.amount);
            }
        }
        totals
    }

    /// The transactions dated in `month`.
    // Returns "some iterator over &Transaction" without naming its exact
    // (long, compiler-generated) type, like returning `auto` in C++. The
    // `'_` says the iterator borrows from `self`: it can't outlive the
    // budget. That's a *lifetime*, a compile-time check that references
    // never dangle; C++ has no equivalent.
    fn transactions_in(&self, month: Month) -> impl Iterator<Item = &Transaction> + '_ {
        self.transactions
            .iter()
            .filter(move |transaction| month.contains(transaction.date))
    }

    /// The total of the transactions realising line `id` in `month`.
    fn line_actual(&self, id: LineId, month: Month) -> Money {
        self.transactions_in(month)
            .filter(|transaction| transaction.line == Some(id))
            .fold(Money::zero(self.currency), |total, transaction| {
                total + transaction.amount
            })
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

    fn month(year: i16, month: i8) -> Month {
        Month::new(year, month).unwrap()
    }

    fn every(n: u8, first: Date) -> Schedule {
        Schedule::Months {
            every: NonZeroU8::new(n).unwrap(),
            first,
        }
    }

    /// The household of GLOSSARY.md: €2,500 of contributions a month, rent,
    /// electricity every two months, a yearly insurance and groceries. The
    /// identifiers of the rent and groceries lines come back with it, in a
    /// tuple.
    fn household() -> (Budget, LineId, LineId) {
        let start = date(2026, 1, 1);
        let mut budget = Budget::new("Household", Currency::EUR);
        // A small helper closure: build a line or fail the test.
        let line = |name, category, kind, cents, schedule| {
            Line::new(name, category, kind, start, eur(cents), schedule).unwrap()
        };
        budget.add_line(line(
            "Contributions",
            "Contributions",
            Kind::Income,
            250000,
            every(1, date(2026, 1, 1)),
        ));
        let rent = budget.add_line(line(
            "Rent",
            "Housing",
            Kind::Cost,
            90000,
            every(1, date(2026, 1, 5)),
        ));
        budget.add_line(line(
            "Electricity",
            "Energy",
            Kind::Cost,
            12000,
            every(2, date(2026, 1, 15)),
        ));
        budget.add_line(line(
            "Home insurance",
            "Housing",
            Kind::Cost,
            48000,
            every(12, date(2026, 3, 10)),
        ));
        let groceries = budget.add_line(line(
            "Groceries",
            "Food",
            Kind::Cost,
            60000,
            every(1, date(2026, 1, 1)),
        ));
        (budget, rent, groceries)
    }

    fn cost(day: Date, cents: i64, category: &str, line: Option<LineId>) -> Transaction {
        Transaction::new(day, Kind::Cost, eur(cents), category, line)
    }

    #[test]
    fn due_view_follows_the_calendar() {
        // `let (a, ..) = tuple` keeps the first element and ignores the rest.
        let (budget, ..) = household();
        let march = budget.plan(month(2026, 3), View::Due);
        // Rent + electricity + insurance + groceries, all due in March.
        assert_eq!(march.costs, eur(90000 + 12000 + 48000 + 60000));
        assert_eq!(march.balance(), eur(250000 - 210000));

        let april = budget.plan(month(2026, 4), View::Due);
        assert_eq!(april.costs, eur(90000 + 60000));
    }

    #[test]
    fn smoothed_view_is_the_same_every_month() {
        let (budget, ..) = household();
        // Rent 900 + electricity 120 / 2 + insurance 480 / 12 + groceries
        // 600 = 1,600.
        for m in 1..=12 {
            let plan = budget.plan(month(2026, m), View::Smoothed);
            assert_eq!(plan.costs, eur(160000));
            assert_eq!(plan.balance(), eur(90000));
        }
    }

    #[test]
    fn compares_a_line_with_its_transactions() {
        let (mut budget, _, groceries) = household();
        budget
            .add_transaction(cost(date(2026, 4, 3), 25000, "Food", Some(groceries)))
            .unwrap();
        budget
            .add_transaction(cost(date(2026, 4, 17), 30000, "Food", Some(groceries)))
            .unwrap();
        let april = budget.compare_line(groceries, month(2026, 4)).unwrap();
        assert_eq!(april.actual, eur(55000));
        assert_eq!(april.variance(), eur(-5000));
        assert_eq!(april.status(), Status::Close); // 92% of the plan
    }

    #[test]
    fn colours_follow_the_ninety_percent_threshold() {
        let compare = |planned, actual| Comparison {
            planned: eur(planned),
            actual: eur(actual),
        };
        assert_eq!(compare(60000, 53999).status(), Status::Under);
        assert_eq!(compare(60000, 54000).status(), Status::Close);
        assert_eq!(compare(60000, 60000).status(), Status::Close);
        assert_eq!(compare(60000, 60001).status(), Status::Over);
        // Nothing planned: green until something is spent.
        assert_eq!(compare(0, 0).status(), Status::Under);
        assert_eq!(compare(0, 1).status(), Status::Over);
    }

    #[test]
    fn unplanned_spending_shows_in_its_category() {
        let (mut budget, _, groceries) = household();
        budget
            .add_transaction(cost(date(2026, 4, 3), 50000, "Food", Some(groceries)))
            .unwrap();
        // A restaurant: in the Food category, but on no line.
        budget
            .add_transaction(cost(date(2026, 4, 10), 15000, "Food", None))
            .unwrap();
        let food = budget.compare_category(Kind::Cost, "Food", month(2026, 4));
        assert_eq!(food.planned, eur(60000));
        assert_eq!(food.actual, eur(65000));
        assert_eq!(food.status(), Status::Over);
    }

    #[test]
    fn a_refund_lowers_what_was_spent() {
        let (mut budget, _, groceries) = household();
        budget
            .add_transaction(cost(date(2026, 4, 3), 20000, "Food", Some(groceries)))
            .unwrap();
        budget
            .add_transaction(cost(date(2026, 4, 9), -5000, "Food", Some(groceries)))
            .unwrap();
        let april = budget.compare_line(groceries, month(2026, 4)).unwrap();
        assert_eq!(april.actual, eur(15000));
    }

    #[test]
    fn forecast_of_the_current_month() {
        let (mut budget, rent, groceries) = household();
        let today = date(2026, 4, 15);
        // Rent paid, €250 of groceries so far, and an unplanned €80.
        budget
            .add_transaction(cost(date(2026, 4, 5), 90000, "Housing", Some(rent)))
            .unwrap();
        budget
            .add_transaction(cost(date(2026, 4, 12), 25000, "Food", Some(groceries)))
            .unwrap();
        budget
            .add_transaction(cost(date(2026, 4, 13), 8000, "Leisure", None))
            .unwrap();
        let forecast = budget.forecast(month(2026, 4), today);
        // Rent 900 + groceries max(250, 600) + unplanned 80.
        assert_eq!(forecast.costs, eur(90000 + 60000 + 8000));
        // The contributions haven't arrived yet: the plan counts them.
        assert_eq!(forecast.incomes, eur(250000));
        // Past months are their actual, future months their plan.
        assert_eq!(budget.forecast(month(2026, 3), today).costs, eur(0));
        assert_eq!(
            budget.forecast(month(2026, 5), today),
            budget.plan(month(2026, 5), View::Due)
        );
    }

    #[test]
    fn refuses_inconsistent_transactions() {
        let (mut budget, rent, _) = household();
        let chf = Currency::from_code("CHF").unwrap();
        let in_chf = Transaction::new(
            date(2026, 4, 5),
            Kind::Cost,
            Money::new(100, chf),
            "Housing",
            None,
        );
        assert_eq!(
            budget.add_transaction(in_chf),
            Err(BudgetError::WrongCurrency)
        );

        let unknown = cost(date(2026, 4, 5), 100, "Housing", Some(LineId::new()));
        assert_eq!(
            budget.add_transaction(unknown),
            Err(BudgetError::UnknownLine)
        );

        let income_on_rent = Transaction::new(
            date(2026, 4, 5),
            Kind::Income,
            eur(100),
            "Housing",
            Some(rent),
        );
        assert_eq!(
            budget.add_transaction(income_on_rent),
            Err(BudgetError::KindMismatch)
        );
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
