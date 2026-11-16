//! What the app can ask and do. Only plain types cross to Dart: texts,
//! numbers, lists, and enums and structs made of them. The `App` itself is
//! *opaque*: Dart holds a handle to it, never its insides.

use std::num::NonZeroU8;
use std::sync::{Mutex, MutexGuard, PoisonError};

use bougette_core::budget::{Budget, Comparison, MonthTotals};
use bougette_core::id::{BudgetId, LineId, MemberId};
use bougette_core::line::{Kind, Line, View};
use bougette_core::money::{Currency, Money};
use bougette_core::month::Month;
use bougette_core::schedule::Schedule;
use bougette_core::shared::reflections;
use bougette_core::transaction::Transaction;
use bougette_store::{Store, StoreError};
use jiff::civil::Date;

/// The app's access to this device's data.
pub struct App {
    // A Mutex because Dart calls come from a pool of threads.
    store: Mutex<Store>,
}

/// How a cost compares with its plan: the colour shown (doc/scope.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// No colour: an income, or a month to come.
    None,
    /// Green: under 90% of the plan.
    Under,
    /// Yellow or orange: from 90% to 100% of the plan.
    Close,
    /// Red: more than planned.
    Over,
}

/// Where a month's figures come from, seen from today.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// A past month: what happened.
    Actual,
    /// The current month: what happened so far, and the rest of the plan.
    Forecast,
    /// A month to come: the plan.
    Plan,
}

/// A budget on the home screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetSummary {
    pub id: String,
    pub name: String,
    pub currency: String,
    /// This month's planned balance, smoothed (GLOSSARY.md: Monthly
    /// balance), in cents.
    pub monthly_balance: i64,
    /// The running balance at the end of this month, in cents.
    pub running_balance: i64,
    pub personal: bool,
}

/// One month of the calendar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonthCell {
    /// `2026-11`.
    pub month: String,
    /// The smoothed plan's balance.
    pub planned: i64,
    /// The balance expected: actual, forecast or plan (see `source`).
    pub expected: i64,
    /// The running balance at the end of the month.
    pub running: i64,
    pub source: Source,
    /// The costs compared with their plan, for past and current months.
    pub status: Status,
}

/// A line in a month.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineRow {
    pub id: String,
    pub name: String,
    pub category: String,
    pub income: bool,
    pub due: i64,
    pub smoothed: i64,
    pub actual: i64,
    /// For a cost, its actual compared with what is due.
    pub status: Status,
}

/// A transaction in a month.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionRow {
    pub id: String,
    /// `2026-11-16`.
    pub date: String,
    pub income: bool,
    pub amount: i64,
    pub category: String,
    /// The line it realises, by name, if any.
    pub line: Option<String>,
    pub note: String,
}

/// Incomes, costs and their balance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Totals {
    pub incomes: i64,
    pub costs: i64,
    pub balance: i64,
}

/// A pot and what it holds at the end of the month.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PotRow {
    pub name: String,
    pub balance: i64,
    pub goal: Option<i64>,
}

/// Everything about one month of a budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonthView {
    pub budget: String,
    pub currency: String,
    pub month: String,
    pub source: Source,
    /// The lines that apply this month, incomes first.
    pub lines: Vec<LineRow>,
    /// The month's transactions by date; in a personal budget, with what
    /// the member paid or received in shared ones.
    pub transactions: Vec<TransactionRow>,
    pub due: Totals,
    pub smoothed: Totals,
    pub actual: Totals,
    pub forecast: Totals,
    /// The actual costs compared with the costs due.
    pub status: Status,
    pub running: i64,
    pub pots: Vec<PotRow>,
}

/// When a new line is due, from its first due date on. A plain enum (no
/// data in its variants) becomes a plain Dart enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repeat {
    /// Every month on the first date's day.
    Monthly,
    Quarterly,
    Yearly,
    Weekly,
    Once,
}

impl App {
    /// Opens (or creates) the database at `path`.
    pub fn open(path: String) -> Result<App, String> {
        let store = Store::open(std::path::Path::new(&path)).map_err(stored)?;
        Ok(App {
            store: Mutex::new(store),
        })
    }

    /// The budgets, by name, with this month's balances.
    pub fn budgets(&self) -> Result<Vec<BudgetSummary>, String> {
        self.budgets_on(today())
    }

    /// Creates a budget; returns its identifier.
    pub fn create_budget(&self, name: String, currency: String) -> Result<String, String> {
        let currency =
            Currency::from_code(&currency).ok_or_else(|| format!("not a currency: {currency}"))?;
        let budget = self
            .lock()
            .create(Budget::new(name, currency))
            .map_err(stored)?;
        Ok(budget.id.to_string())
    }

    /// The twelve months of `year` for budget `budget`.
    pub fn calendar(&self, budget: String, year: i16) -> Result<Vec<MonthCell>, String> {
        self.calendar_on(&budget, year, today())
    }

    /// Month `month` (`2026-11`) of budget `budget`.
    pub fn month(&self, budget: String, month: String) -> Result<MonthView, String> {
        self.month_on(&budget, &month, today())
    }

    /// Adds a planned line: `amount` cents, due as `repeat` says from
    /// `first` (`2026-11-05`) on.
    #[allow(clippy::too_many_arguments)] // the fields of a new line
    pub fn add_line(
        &self,
        budget: String,
        name: String,
        category: String,
        income: bool,
        amount: i64,
        repeat: Repeat,
        first: String,
    ) -> Result<String, String> {
        let first = date(&first)?;
        let months = |every| Schedule::Months {
            every: NonZeroU8::new(every).expect("not zero"),
            first,
        };
        let schedule = match repeat {
            Repeat::Monthly => months(1),
            Repeat::Quarterly => months(3),
            Repeat::Yearly => months(12),
            Repeat::Weekly => Schedule::Weekly { first },
            Repeat::Once => Schedule::Once { on: first },
        };
        self.edit(&budget, |budget| {
            let line = Line::new(
                name,
                category,
                kind(income),
                first,
                Money::new(amount, budget.currency()),
                schedule,
            )
            .map_err(|error| format!("{error:?}"))?;
            let id = budget
                .add_line(line)
                .map_err(|error| format!("{error:?}"))?;
            Ok(id.to_string())
        })
    }

    /// Adds a transaction of `amount` cents on `date`, realising the line
    /// `line` (its identifier) if given.
    #[allow(clippy::too_many_arguments)] // the fields of a transaction
    pub fn add_transaction(
        &self,
        budget: String,
        date_text: String,
        income: bool,
        amount: i64,
        category: String,
        line: Option<String>,
        note: String,
    ) -> Result<String, String> {
        let day = date(&date_text)?;
        let line = line
            .map(|id| id.parse().map(LineId).map_err(|_| "not a line".to_string()))
            .transpose()?;
        self.edit(&budget, |budget| {
            let mut transaction = Transaction::new(
                day,
                kind(income),
                Money::new(amount, budget.currency()),
                category,
                line,
            );
            transaction.note = note;
            let id = budget
                .add_transaction(transaction)
                .map_err(|error| format!("{error:?}"))?;
            Ok(id.to_string())
        })
    }

    // The same, with `today` given: for the tests.

    fn budgets_on(&self, today: Date) -> Result<Vec<BudgetSummary>, String> {
        let month = Month::of(today);
        let store = self.lock();
        let mut summaries = Vec::new();
        for (id, _) in store.budgets().map_err(stored)? {
            let Some(budget) = store.load(id).map_err(stored)? else {
                continue;
            };
            summaries.push(BudgetSummary {
                id: id.to_string(),
                name: budget.name.clone(),
                currency: budget.currency().code().to_string(),
                monthly_balance: budget.plan(month, View::Smoothed).balance().minor_units(),
                running_balance: budget.running_balance(month, today).minor_units(),
                personal: budget.personal,
            });
        }
        Ok(summaries)
    }

    fn calendar_on(&self, budget: &str, year: i16, today: Date) -> Result<Vec<MonthCell>, String> {
        let budget = self.load(budget)?;
        let current = Month::of(today);
        let mut month = Month::new(year, 1).ok_or_else(|| format!("not a year: {year}"))?;
        let mut cells = Vec::new();
        for _ in 0..12 {
            let expected = budget.forecast(month, today);
            let status = if month <= current {
                status(Comparison {
                    planned: budget.plan(month, View::Due).costs,
                    actual: expected.costs,
                })
            } else {
                Status::None
            };
            cells.push(MonthCell {
                month: month_text(month),
                planned: budget.plan(month, View::Smoothed).balance().minor_units(),
                expected: expected.balance().minor_units(),
                running: budget.running_balance(month, today).minor_units(),
                source: source(month, current),
                status,
            });
            month = month.next();
        }
        Ok(cells)
    }

    fn month_on(&self, budget: &str, month: &str, today: Date) -> Result<MonthView, String> {
        let mut budget = self.load(budget)?;
        let month = parse_month(month)?;
        // A personal budget shows what its member paid or received in the
        // shared ones (computed here, never saved).
        if budget.personal {
            let mut store = self.lock();
            let me = MemberId(store.member().map_err(stored)?);
            let mut reflected = Vec::new();
            for (id, _) in store.budgets().map_err(stored)? {
                if id != budget.id
                    && let Some(shared) = store.load(id).map_err(stored)?
                {
                    reflected.extend(reflections(&shared, &me));
                }
            }
            budget = budget.with_reflections(reflected);
        }
        let mut lines = Vec::new();
        for income in [true, false] {
            for line in budget
                .lines()
                .iter()
                .filter(|line| (line.kind == Kind::Income) == income)
            {
                let Some(comparison) = budget.compare_line(line.id, month) else {
                    continue;
                };
                let smoothed = line.amount_in(month, View::Smoothed);
                if smoothed.minor_units() == 0
                    && comparison.planned.minor_units() == 0
                    && comparison.actual.minor_units() == 0
                {
                    continue; // the line doesn't apply this month
                }
                lines.push(LineRow {
                    id: line.id.to_string(),
                    name: line.name.clone(),
                    category: line.category.clone(),
                    income,
                    due: comparison.planned.minor_units(),
                    smoothed: smoothed.minor_units(),
                    actual: comparison.actual.minor_units(),
                    status: if income {
                        Status::None
                    } else {
                        status(comparison)
                    },
                });
            }
        }
        let mut transactions: Vec<&Transaction> = budget
            .transactions()
            .iter()
            .filter(|transaction| month.contains(transaction.date))
            .collect();
        transactions.sort_by_key(|transaction| transaction.date);
        let transactions = transactions
            .into_iter()
            .map(|transaction| TransactionRow {
                id: transaction.id.to_string(),
                date: transaction.date.to_string(),
                income: transaction.kind == Kind::Income,
                amount: transaction.amount.minor_units(),
                category: transaction.category.clone(),
                line: transaction
                    .line
                    .and_then(|id| budget.line(id))
                    .map(|line| line.name.clone()),
                note: transaction.note.clone(),
            })
            .collect();
        let due = budget.plan(month, View::Due);
        let actual = budget.actual(month);
        let pots = budget
            .pots()
            .iter()
            .map(|pot| PotRow {
                name: pot.name.clone(),
                balance: budget
                    .pot_balance(pot.id, month.last_day())
                    .map_or(0, |balance| balance.minor_units()),
                goal: pot.goal.map(|goal| goal.amount.minor_units()),
            })
            .collect();
        Ok(MonthView {
            budget: budget.name.clone(),
            currency: budget.currency().code().to_string(),
            month: month_text(month),
            source: source(month, Month::of(today)),
            lines,
            transactions,
            due: totals(due),
            smoothed: totals(budget.plan(month, View::Smoothed)),
            actual: totals(actual),
            forecast: totals(budget.forecast(month, today)),
            status: status(Comparison {
                planned: due.costs,
                actual: actual.costs,
            }),
            running: budget.running_balance(month, today).minor_units(),
            pots,
        })
    }

    fn load(&self, id: &str) -> Result<Budget, String> {
        let id: BudgetId = BudgetId(id.parse().map_err(|_| format!("not a budget: {id}"))?);
        self.lock()
            .load(id)
            .map_err(stored)?
            .ok_or_else(|| "this budget is gone".to_string())
    }

    /// Loads budget `id`, applies `change`, saves it if that succeeded.
    fn edit(
        &self,
        id: &str,
        change: impl FnOnce(&mut Budget) -> Result<String, String>,
    ) -> Result<String, String> {
        let mut budget = self.load(id)?;
        let result = change(&mut budget)?;
        self.lock().save(&budget).map_err(stored)?;
        Ok(result)
    }

    fn lock(&self) -> MutexGuard<'_, Store> {
        self.store.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn status(comparison: Comparison) -> Status {
    match comparison.status() {
        bougette_core::budget::Status::Under => Status::Under,
        bougette_core::budget::Status::Close => Status::Close,
        bougette_core::budget::Status::Over => Status::Over,
    }
}

fn source(month: Month, current: Month) -> Source {
    if month < current {
        Source::Actual
    } else if month == current {
        Source::Forecast
    } else {
        Source::Plan
    }
}

fn totals(totals: MonthTotals) -> Totals {
    Totals {
        incomes: totals.incomes.minor_units(),
        costs: totals.costs.minor_units(),
        balance: totals.balance().minor_units(),
    }
}

fn kind(income: bool) -> Kind {
    if income { Kind::Income } else { Kind::Cost }
}

fn today() -> Date {
    jiff::Zoned::now().date()
}

fn date(text: &str) -> Result<Date, String> {
    text.parse().map_err(|_| format!("not a date: {text}"))
}

fn parse_month(text: &str) -> Result<Month, String> {
    text.split_once('-')
        .and_then(|(year, month)| Month::new(year.parse().ok()?, month.parse().ok()?))
        .ok_or_else(|| format!("not a month: {text}"))
}

fn month_text(month: Month) -> String {
    let day = month.first_day();
    format!("{}-{:02}", day.year(), day.month())
}

fn stored(error: StoreError) -> String {
    match error {
        StoreError::NotAllowed(reason) => reason,
        other => format!("storage: {other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date as day;

    fn app() -> App {
        App {
            store: Mutex::new(Store::open_in_memory().unwrap()),
        }
    }

    /// A household budget with rent, an overspent month and a yearly line.
    fn household(app: &App) -> String {
        let id = app.create_budget("Household".into(), "EUR".into()).unwrap();
        let pay = Repeat::Monthly;
        app.add_line(
            id.clone(),
            "Pay".into(),
            "Income".into(),
            true,
            250000,
            pay,
            "2026-01-01".into(),
        )
        .unwrap();
        let rent = app
            .add_line(
                id.clone(),
                "Rent".into(),
                "Housing".into(),
                false,
                90000,
                pay,
                "2026-01-05".into(),
            )
            .unwrap();
        app.add_line(
            id.clone(),
            "Insurance".into(),
            "Housing".into(),
            false,
            48000,
            Repeat::Yearly,
            "2026-03-10".into(),
        )
        .unwrap();
        app.add_transaction(
            id.clone(),
            "2026-11-05".into(),
            false,
            95000,
            "Housing".into(),
            Some(rent),
            "November".into(),
        )
        .unwrap();
        id
    }

    #[test]
    fn the_home_screen_lists_budgets_with_their_balances() {
        let app = app();
        household(&app);
        let budgets = app.budgets_on(day(2026, 11, 16)).unwrap();
        assert_eq!(budgets.len(), 1);
        // 2,500 - 900 - 40 (480 a year).
        assert_eq!(budgets[0].monthly_balance, 156000);
        assert_eq!(budgets[0].currency, "EUR");
    }

    #[test]
    fn the_calendar_colours_past_and_current_months() {
        let app = app();
        let id = household(&app);
        let cells = app.calendar_on(&id, 2026, day(2026, 11, 16)).unwrap();
        assert_eq!(cells.len(), 12);
        assert_eq!(cells[0].month, "2026-01");
        assert_eq!(cells[0].source, Source::Actual);
        assert_eq!(cells[10].source, Source::Forecast);
        // November: 950 paid for 900 due.
        assert_eq!(cells[10].status, Status::Over);
        assert_eq!(cells[11].source, Source::Plan);
        assert_eq!(cells[11].status, Status::None);
    }

    #[test]
    fn a_month_shows_its_lines_and_transactions() {
        let app = app();
        let id = household(&app);
        let view = app.month_on(&id, "2026-11", day(2026, 11, 16)).unwrap();
        let names: Vec<&str> = view.lines.iter().map(|line| line.name.as_str()).collect();
        assert_eq!(names, ["Pay", "Rent", "Insurance"]);
        let rent = &view.lines[1];
        assert_eq!(
            (rent.due, rent.actual, rent.status),
            (90000, 95000, Status::Over)
        );
        assert_eq!(view.lines[2].smoothed, 4000);
        assert_eq!(view.transactions[0].line.as_deref(), Some("Rent"));
        assert_eq!(view.status, Status::Over);
        assert!(app.month_on(&id, "November", day(2026, 11, 16)).is_err());
    }
}
