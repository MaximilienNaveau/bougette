//! Budgets: their plan, their actual transactions, and how the two compare.

use jiff::civil::Date;

use crate::id::{BudgetId, LineId, PotId, TransactionId};
use crate::line::{Kind, Line, LineError, View};
use crate::money::{Currency, Money};
use crate::month::Month;
use crate::pot::Pot;
use crate::schedule::Schedule;
use crate::shared::Reflections;
use crate::transaction::{PotMove, Transaction};

/// A budget (GLOSSARY.md: Budget): its lines (the plan), its transactions
/// (the actual) and its pots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Budget {
    pub id: BudgetId,
    pub name: String,
    /// A member's personal budget: their contributions, advances and
    /// repayments in shared budgets show in it (see `with_reflections`).
    pub personal: bool,
    // Private: fixed when the budget is created (doc/scope.md).
    currency: Currency,
    // Private: `add_line`, `add_transaction` and `add_pot` check what goes
    // in.
    lines: Vec<Line>,
    transactions: Vec<Transaction>,
    pots: Vec<Pot>,
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

/// Why a line or a transaction was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetError {
    /// The amount isn't in the budget's currency.
    WrongCurrency,
    /// The transaction points to a line this budget doesn't have.
    UnknownLine,
    /// The line or transaction points to a pot this budget doesn't have.
    UnknownPot,
    /// An income transaction on a cost line, or the reverse.
    KindMismatch,
    /// Money moves into or out of a pot through costs only.
    PotNeedsACost,
    /// A transaction on an allocation line doesn't feed that line's pot.
    PotMismatch,
    /// No transaction of this budget has that identifier.
    UnknownTransaction,
    /// The line still has transactions: remove them first.
    LineInUse,
    /// The line refused the change.
    // A variant can carry data: this one wraps the line's own error.
    Line(LineError),
    /// Only a cost can be paid to a member, and not by another member.
    PayeeNeedsACost,
}

/// The reason, in words, as shown to people (`{}` in `format!`).
impl std::fmt::Display for BudgetError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let reason = match self {
            BudgetError::WrongCurrency => "the amount isn't in the budget's currency",
            BudgetError::UnknownLine => "no such line",
            BudgetError::UnknownPot => "no such pot",
            BudgetError::KindMismatch => "an income can't realise a cost line, nor the reverse",
            BudgetError::PotNeedsACost => "only costs move money into or out of a pot",
            BudgetError::PotMismatch => {
                "a transaction on an allocation line goes into that line's pot"
            }
            BudgetError::UnknownTransaction => "no such transaction",
            BudgetError::LineInUse => {
                "the line has transactions: remove them first, or end the line"
            }
            BudgetError::Line(LineError::BeforeStart) => {
                "the line would end before it starts: remove it instead"
            }
            BudgetError::Line(LineError::NotPositive) => "a line's amount must be positive",
            BudgetError::Line(LineError::NoSuchRevision) => "no change of the line on that date",
            BudgetError::Line(LineError::OnlyRevision) => {
                "a line keeps at least one amount: remove the line instead"
            }
            BudgetError::PayeeNeedsACost => {
                "only a cost can be paid to a member, and not by a member"
            }
        };
        // `write!` into the formatter, like `os << reason`.
        write!(f, "{reason}")
    }
}

// Lets `?` turn a LineError into a BudgetError (see bougette-store's
// StoreError for the same pattern).
impl From<LineError> for BudgetError {
    fn from(error: LineError) -> Self {
        BudgetError::Line(error)
    }
}

impl Budget {
    /// An empty budget in `currency`.
    pub fn new(name: impl Into<String>, currency: Currency) -> Budget {
        Budget {
            id: BudgetId::new(),
            name: name.into(),
            currency,
            lines: Vec::new(),
            transactions: Vec::new(),
            pots: Vec::new(),
            personal: false,
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

    pub fn pots(&self) -> &[Pot] {
        &self.pots
    }

    /// The pot with this identifier, if the budget has it.
    pub fn pot(&self, id: PotId) -> Option<&Pot> {
        self.pots.iter().find(|pot| pot.id == id)
    }

    /// Adds a pot and returns its identifier.
    pub fn add_pot(&mut self, pot: Pot) -> PotId {
        let id = pot.id;
        self.pots.push(pot);
        id
    }

    /// Adds a line and returns its identifier. An allocation (a line with a
    /// pot) must be a cost and feed a pot of this budget.
    ///
    /// Panics if the line's amounts aren't in the budget's currency:
    /// callers build amounts from `currency()`, so that would be a bug.
    pub fn add_line(&mut self, line: Line) -> Result<LineId, BudgetError> {
        assert!(
            line.revisions()
                .iter()
                .all(|revision| revision.amount.currency() == self.currency),
            "line in another currency than its budget"
        );
        if let Some(pot) = line.pot {
            if line.kind != Kind::Cost {
                return Err(BudgetError::PotNeedsACost);
            }
            // `ok_or` turns an Option into a Result: `None` becomes the
            // given error. The `?` operator then returns that error from
            // the function, or unwraps the `Ok` value. It replaces the
            // "check and return early" boilerplate that exceptions hide.
            self.pot(pot).ok_or(BudgetError::UnknownPot)?;
        }
        let id = line.id;
        // `push` takes the line by value: it is *moved* into the Vec, and
        // the caller can no longer use it (the compiler enforces it).
        self.lines.push(line);
        Ok(id)
    }

    /// Adds a transaction after checking its currency, its line and its pot.
    pub fn add_transaction(
        &mut self,
        transaction: Transaction,
    ) -> Result<TransactionId, BudgetError> {
        if transaction.amount.currency() != self.currency {
            return Err(BudgetError::WrongCurrency);
        }
        if let Some(line_id) = transaction.line {
            let line = self.line(line_id).ok_or(BudgetError::UnknownLine)?;
            if line.kind != transaction.kind {
                return Err(BudgetError::KindMismatch);
            }
            // A transaction on an allocation line puts money in its pot.
            if let Some(pot) = line.pot
                && transaction.pot != Some(PotMove::Into(pot))
            {
                return Err(BudgetError::PotMismatch);
            }
        }
        if transaction.payee.is_some()
            && (transaction.kind != Kind::Cost || transaction.payer.is_some())
        {
            return Err(BudgetError::PayeeNeedsACost);
        }
        if let Some(PotMove::Into(pot) | PotMove::PaidFrom(pot)) = transaction.pot {
            // `A(x) | B(x)` matches either variant and binds `x` in both.
            if transaction.kind != Kind::Cost {
                return Err(BudgetError::PotNeedsACost);
            }
            self.pot(pot).ok_or(BudgetError::UnknownPot)?;
        }
        let id = transaction.id;
        self.transactions.push(transaction);
        Ok(id)
    }

    /// From `from` on, line `id` has a new amount and schedule; months
    /// before keep the old ones (see `Line::change_from`).
    pub fn change_line(
        &mut self,
        id: LineId,
        from: Date,
        amount: Money,
        schedule: Schedule,
    ) -> Result<(), BudgetError> {
        if amount.currency() != self.currency {
            return Err(BudgetError::WrongCurrency);
        }
        // Two `?` in a row: the first fails on an unknown line, the second
        // converts the line's refusal with `From` above.
        self.line_mut(id)?.change_from(from, amount, schedule)?;
        Ok(())
    }

    /// Line `id` stops after `last_day`, or goes on again with `None`.
    pub fn end_line(&mut self, id: LineId, last_day: Option<Date>) -> Result<(), BudgetError> {
        self.line_mut(id)?.end_on(last_day)?;
        Ok(())
    }

    /// Sets one revision of line `id` (see `Line::set_revision`).
    pub fn set_revision(
        &mut self,
        id: LineId,
        replacing: Option<Date>,
        from: Date,
        amount: Money,
        schedule: Schedule,
    ) -> Result<(), BudgetError> {
        if amount.currency() != self.currency {
            return Err(BudgetError::WrongCurrency);
        }
        self.line_mut(id)?
            .set_revision(replacing, from, amount, schedule)?;
        Ok(())
    }

    /// Removes the revision of line `id` starting on `from`.
    pub fn remove_revision(&mut self, id: LineId, from: Date) -> Result<(), BudgetError> {
        self.line_mut(id)?.remove_revision(from)?;
        Ok(())
    }

    /// Renames line `id`, or moves it to another category.
    pub fn rename_line(
        &mut self,
        id: LineId,
        name: impl Into<String>,
        category: impl Into<String>,
    ) -> Result<(), BudgetError> {
        let line = self.line_mut(id)?;
        line.name = name.into();
        line.category = category.into();
        Ok(())
    }

    /// Removes line `id`, refused while transactions still realise it. To
    /// stop a line that was real, end it instead: its past stays planned.
    pub fn remove_line(&mut self, id: LineId) -> Result<(), BudgetError> {
        self.line(id).ok_or(BudgetError::UnknownLine)?;
        if self.transactions.iter().any(|t| t.line == Some(id)) {
            return Err(BudgetError::LineInUse);
        }
        self.lines.retain(|line| line.id != id);
        Ok(())
    }

    /// Removes transaction `id`.
    pub fn remove_transaction(&mut self, id: TransactionId) -> Result<(), BudgetError> {
        // `position` is `std::find_if` returning an index.
        let index = self
            .transactions
            .iter()
            .position(|transaction| transaction.id == id)
            .ok_or(BudgetError::UnknownTransaction)?;
        self.transactions.remove(index);
        Ok(())
    }

    /// Line `id`, to change it. Private: `pub` access would let callers
    /// turn an allocation into an income, bypassing `add_line`'s checks.
    // `iter_mut` yields `&mut Line`, so the result can be modified.
    fn line_mut(&mut self, id: LineId) -> Result<&mut Line, BudgetError> {
        self.lines
            .iter_mut()
            .find(|line| line.id == id)
            .ok_or(BudgetError::UnknownLine)
    }

    /// This budget with `extra` lines and transactions added without
    /// checks: the reflections of other budgets in a personal budget (see
    /// `shared::reflections`), for display. Never save the result: the
    /// reflections would then be counted twice.
    pub fn with_reflections(&self, extra: Reflections) -> Budget {
        let mut budget = self.clone();
        // `extend` appends every item of an iterator, like
        // `v.insert(v.end(), ...)`; amounts in another currency are left
        // out.
        budget.lines.extend(
            extra
                .lines
                .into_iter()
                .filter(|line| line.revisions()[0].amount.currency() == self.currency),
        );
        budget.transactions.extend(
            extra
                .transactions
                .into_iter()
                .filter(|transaction| transaction.amount.currency() == self.currency),
        );
        budget
    }

    /// What pot `id` holds at the end of `date`: what was allocated to it
    /// minus what was paid from it. Negative when overspent, which is
    /// allowed (doc/scope.md). `None` for an unknown pot.
    pub fn pot_balance(&self, id: PotId, date: Date) -> Option<Money> {
        self.pot(id)?;
        let mut balance = Money::zero(self.currency);
        for transaction in &self.transactions {
            if transaction.date > date {
                continue;
            }
            match transaction.pot {
                Some(PotMove::Into(pot)) if pot == id => balance += transaction.amount,
                // `if` after a pattern is a "match guard": an extra test.
                Some(PotMove::PaidFrom(pot)) if pot == id => balance = balance - transaction.amount,
                // `_` matches everything else: other pots, or no pot.
                _ => {}
            }
        }
        Some(balance)
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

    /// The balances of every month from the budget's first one to `month`
    /// included, added up (GLOSSARY.md: Running balance), seen from
    /// `today`: actual for past months, forecast for the current one, plan
    /// for the coming ones. It shows the reserve building up for a yearly
    /// bill. Zero for an empty budget.
    pub fn running_balance(&self, month: Month, today: Date) -> Money {
        let mut balance = Money::zero(self.currency);
        // `let ... else` again: an empty budget has no first month.
        let Some(mut current) = self.first_month() else {
            return balance;
        };
        while current <= month {
            balance += self.forecast(current, today).balance();
            current = current.next();
        }
        balance
    }

    /// The month of the earliest line start or transaction, if any.
    fn first_month(&self) -> Option<Month> {
        // Two iterators of dates chained into one, then the smallest:
        // `min` returns None when there is no date at all.
        self.lines
            .iter()
            .map(|line| line.starts_on())
            .chain(self.transactions.iter().map(|transaction| transaction.date))
            .min()
            .map(Month::of)
    }

    /// The transactions dated in `month` that count in the month's budget:
    /// all but the spending paid from a pot.
    // Returns "some iterator over &Transaction" without naming its exact
    // (long, compiler-generated) type, like returning `auto` in C++. The
    // `'_` says the iterator borrows from `self`: it can't outlive the
    // budget. That's a *lifetime*, a compile-time check that references
    // never dangle; C++ has no equivalent.
    fn transactions_in(&self, month: Month) -> impl Iterator<Item = &Transaction> + '_ {
        self.transactions.iter().filter(move |transaction| {
            // `matches!` is true if the value fits the pattern.
            month.contains(transaction.date)
                && !matches!(transaction.pot, Some(PotMove::PaidFrom(_)))
        })
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

    /// Adds a line that must be accepted.
    fn add(budget: &mut Budget, line: Line) -> LineId {
        budget.add_line(line).unwrap()
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
        add(
            &mut budget,
            line(
                "Contributions",
                "Contributions",
                Kind::Income,
                250000,
                every(1, date(2026, 1, 1)),
            ),
        );
        let rent = add(
            &mut budget,
            line(
                "Rent",
                "Housing",
                Kind::Cost,
                90000,
                every(1, date(2026, 1, 5)),
            ),
        );
        add(
            &mut budget,
            line(
                "Electricity",
                "Energy",
                Kind::Cost,
                12000,
                every(2, date(2026, 1, 15)),
            ),
        );
        add(
            &mut budget,
            line(
                "Home insurance",
                "Housing",
                Kind::Cost,
                48000,
                every(12, date(2026, 3, 10)),
            ),
        );
        let groceries = add(
            &mut budget,
            line(
                "Groceries",
                "Food",
                Kind::Cost,
                60000,
                every(1, date(2026, 1, 1)),
            ),
        );
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
        // `let _ =` drops the Result on purpose: we expect a panic before.
        let _ = budget.add_line(
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

    #[test]
    fn running_balance_builds_the_reserve_for_the_yearly_bill() {
        let (budget, ..) = household();
        // Seen from December 2025: every month of 2026 is planned.
        let today = date(2025, 12, 1);
        let running = |m| budget.running_balance(month(2026, m), today);
        // January: 2,500 in, rent 900 + electricity 120 + groceries 600.
        assert_eq!(running(1), eur(250000 - 162000));
        // February: no electricity bill, so more is set aside.
        assert_eq!(running(2), eur(88000 + 100000));
        // March: insurance and electricity are paid from what built up.
        assert_eq!(running(3), eur(188000 + 250000 - 210000));
        // Over a full year, the smoothed surplus: 12 × 900 = 10,800.
        assert_eq!(running(12), eur(12 * 90000));
    }

    #[test]
    fn running_balance_of_an_empty_budget_is_zero() {
        let budget = Budget::new("Empty", Currency::EUR);
        assert_eq!(
            budget.running_balance(month(2026, 5), date(2026, 5, 1)),
            eur(0)
        );
    }

    /// The household with a "Bike" pot fed €100 a month from January, and
    /// the allocation transactions of January to April.
    fn with_bike_pot() -> (Budget, PotId) {
        let (mut budget, ..) = household();
        let bike = budget.add_pot(Pot::new("Bike"));
        let mut allocation = Line::new(
            "Bike savings",
            "Savings",
            Kind::Cost,
            date(2026, 1, 1),
            eur(10000),
            every(1, date(2026, 1, 1)),
        )
        .unwrap();
        allocation.pot = Some(bike);
        let allocation = add(&mut budget, allocation);
        for m in 1..=4 {
            let mut transaction = cost(date(2026, m, 1), 10000, "Savings", Some(allocation));
            transaction.pot = Some(PotMove::Into(bike));
            budget.add_transaction(transaction).unwrap();
        }
        (budget, bike)
    }

    #[test]
    fn allocations_fill_the_pot_and_cost_the_month() {
        let (budget, bike) = with_bike_pot();
        assert_eq!(
            budget.pot_balance(bike, date(2026, 4, 30)),
            Some(eur(40000))
        );
        assert_eq!(
            budget.pot_balance(bike, date(2026, 2, 15)),
            Some(eur(20000))
        );
        // The allocation is a cost of the month, planned and actual.
        assert_eq!(budget.actual(month(2026, 4)).costs, eur(10000));
        let plan = budget.plan(month(2026, 4), View::Smoothed);
        assert_eq!(plan.costs, eur(160000 + 10000));
    }

    #[test]
    fn spending_from_a_pot_is_not_a_cost_of_the_month() {
        let (mut budget, bike) = with_bike_pot();
        // The bike costs more than the pot holds: allowed, and visible.
        let mut purchase = cost(date(2026, 4, 20), 45000, "Transport", None);
        purchase.pot = Some(PotMove::PaidFrom(bike));
        budget.add_transaction(purchase).unwrap();
        assert_eq!(
            budget.pot_balance(bike, date(2026, 4, 30)),
            Some(eur(-5000))
        );
        // April's actual only has the allocation, not the bike.
        assert_eq!(budget.actual(month(2026, 4)).costs, eur(10000));
        let transport = budget.compare_category(Kind::Cost, "Transport", month(2026, 4));
        assert_eq!(transport.actual, eur(0));
    }

    #[test]
    fn refuses_inconsistent_pot_moves() {
        let (mut budget, bike) = with_bike_pot();
        // An unknown pot.
        let mut unknown = cost(date(2026, 5, 1), 100, "Savings", None);
        unknown.pot = Some(PotMove::Into(PotId::new()));
        assert_eq!(
            budget.add_transaction(unknown),
            Err(BudgetError::UnknownPot)
        );
        // An income can't go into a pot.
        let mut income =
            Transaction::new(date(2026, 5, 1), Kind::Income, eur(100), "Savings", None);
        income.pot = Some(PotMove::Into(bike));
        assert_eq!(
            budget.add_transaction(income),
            Err(BudgetError::PotNeedsACost)
        );
        // A transaction on the allocation line must feed its pot.
        let allocation = budget.lines().iter().find(|l| l.pot.is_some()).unwrap().id;
        let missing = cost(date(2026, 5, 1), 10000, "Savings", Some(allocation));
        assert_eq!(
            budget.add_transaction(missing),
            Err(BudgetError::PotMismatch)
        );
    }

    #[test]
    fn changes_ends_and_removes_lines() {
        let (mut budget, rent, groceries) = household();
        // The rent goes up in April: March keeps the old amount.
        budget
            .change_line(
                rent,
                date(2026, 4, 1),
                eur(95000),
                every(1, date(2026, 4, 5)),
            )
            .unwrap();
        assert_eq!(
            budget.compare_line(rent, month(2026, 3)).unwrap().planned,
            eur(90000)
        );
        assert_eq!(
            budget.compare_line(rent, month(2026, 4)).unwrap().planned,
            eur(95000)
        );
        // Ending it before it starts is the line's error, wrapped.
        assert_eq!(
            budget.end_line(rent, Some(date(2025, 1, 1))),
            Err(BudgetError::Line(LineError::BeforeStart))
        );
        // Ended after June: nothing planned in July.
        budget.end_line(rent, Some(date(2026, 6, 30))).unwrap();
        assert_eq!(
            budget.compare_line(rent, month(2026, 7)).unwrap().planned,
            eur(0)
        );
        // A line with transactions can't be removed until they are.
        let shopping = budget
            .add_transaction(cost(date(2026, 1, 10), 5000, "Food", Some(groceries)))
            .unwrap();
        assert_eq!(budget.remove_line(groceries), Err(BudgetError::LineInUse));
        budget.remove_transaction(shopping).unwrap();
        assert_eq!(
            budget.remove_transaction(shopping),
            Err(BudgetError::UnknownTransaction)
        );
        budget.remove_line(groceries).unwrap();
        assert!(budget.line(groceries).is_none());
        assert_eq!(budget.remove_line(groceries), Err(BudgetError::UnknownLine));
    }
}
