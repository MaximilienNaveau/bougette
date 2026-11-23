//! What the app can ask and do. Only plain types cross to Dart: texts,
//! numbers, lists, and enums and structs made of them. The `App` itself is
//! *opaque*: Dart holds a handle to it, never its insides.

use std::num::NonZeroU8;
use std::sync::{Mutex, MutexGuard, PoisonError};

use std::collections::HashSet;

use bougette_core::budget::{Budget, BudgetError, Comparison, MonthTotals};
use bougette_core::id::{BudgetId, LineId, MemberId, PotId, TransactionId};
use bougette_core::line::{Kind, Line, View};
use bougette_core::money::{Currency, Money};
use bougette_core::month::Month;
use bougette_core::pot::{Goal, Pot};
use bougette_core::schedule::Schedule;
use bougette_core::transaction::PotMove;
use bougette_core::transaction::Transaction;
use bougette_store::{Store, StoreError, Trashed};
use jiff::civil::Date;

// Public: the generated bridge names its types by their path.
pub mod members;

/// The app's access to this device's data.
pub struct App {
    // A Mutex because Dart calls come from a pool of threads.
    store: Mutex<Store>,
    // Drives the network operations (members.rs): iroh runs on tokio.
    runtime: tokio::runtime::Runtime,
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
    /// What the line amounts to each time it's due, this month.
    pub amount: i64,
    /// How it repeats this month; `None` for a schedule the app can't
    /// show (e.g. every 2 months, entered with the command line).
    pub repeat: Option<Repeat>,
    /// The last day it applies, if it ends.
    pub ends_on: Option<String>,
    /// For an allocation, the pot it feeds (its identifier).
    pub pot: Option<String>,
    /// In a personal budget, a line of another budget this member pays
    /// (a planned contribution). Changing it changes it there.
    pub reflected: bool,
}

/// One amount of a line, from a date until the next change (GLOSSARY.md:
/// Revision).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionRow {
    /// `2026-11-30`: for the first, the line's start and first due date.
    pub from: String,
    pub amount: i64,
    /// `None` for a schedule the app can't show (see `LineRow::repeat`).
    pub repeat: Option<Repeat>,
}

/// A line with all its amounts over time, to edit it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineDetail {
    pub id: String,
    pub name: String,
    pub category: String,
    /// As seen from the budget asked about.
    pub income: bool,
    /// The other budget holding it, if it's seen in a personal budget (a
    /// planned contribution): changes go there, and so does its money.
    pub held_in: Option<BudgetChoice>,
    /// For an allocation, the pot it feeds (its identifier).
    pub pot: Option<String>,
    pub ends_on: Option<String>,
    /// Oldest first; never empty.
    pub revisions: Vec<RevisionRow>,
}

/// Another budget, to choose where a line's money goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetChoice {
    pub id: String,
    pub name: String,
}

/// A line or transaction deleted from a budget, restorable for 30 days.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashRow {
    pub id: String,
    /// A line, else a transaction.
    pub line: bool,
    /// `2026-11-20`: when it was deleted.
    pub deleted_on: String,
    /// A line's name, a transaction's category.
    pub name: String,
    /// A line's category, a transaction's note.
    pub detail: String,
    /// A transaction's date.
    pub date: Option<String>,
    pub income: bool,
    /// A transaction's amount, a line's latest.
    pub amount: i64,
}

/// A transaction in a month.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionRow {
    pub id: String,
    /// In a personal budget, a transaction of another budget that this
    /// member paid or received. Removing it removes it there.
    pub reflected: bool,
    /// `2026-11-16`.
    pub date: String,
    pub income: bool,
    pub amount: i64,
    pub category: String,
    /// The line it realises, by name, if any.
    pub line: Option<String>,
    /// That line's identifier, to edit the transaction.
    pub line_id: Option<String>,
    pub note: String,
    /// The pot it moves money into, or out of if `from_pot`.
    pub pot: Option<String>,
    pub from_pot: bool,
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
    pub id: String,
    pub name: String,
    pub balance: i64,
    pub goal: Option<i64>,
    /// When the goal is to be reached (`2027-06-30`), if set.
    pub goal_by: Option<String>,
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
    /// Where a new cost line's money can go besides a pot: the other
    /// budgets in the same currency, for a personal budget only (that's
    /// where the line shows, see `add_line`).
    pub destinations: Vec<BudgetChoice>,
    /// The earliest year to show: that of the budget's first entry, or
    /// the current year if it has none before (no empty years to go
    /// through).
    pub first_year: i16,
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
        App::with(store)
    }

    fn with(store: Store) -> Result<App, String> {
        // A runtime with one worker thread of its own: calls come from
        // Dart's threads and wait for it (`block_on`).
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .map_err(|error| format!("cannot start the async runtime: {error}"))?;
        Ok(App {
            store: Mutex::new(store),
            runtime,
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
    /// `first` (`2026-11-05`) on. Its money goes into pot `pot`, or to
    /// budget `to_budget`, if given.
    ///
    /// A cost going to another budget is this member's planned
    /// contribution to it: the line is written there, as an income they
    /// pay, and shows here as a cost line (see `MonthView::destinations`).
    /// Entered once, it is the same line seen from both budgets.
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
        pot: Option<String>,
        to_budget: Option<String>,
    ) -> Result<String, String> {
        let first = date(&first)?;
        let pot = pot.map(|id| parse_id(&id).map(PotId)).transpose()?;
        let line = Line::new(
            name,
            category,
            kind(income),
            first,
            Money::new(amount, self.load(&budget)?.currency()),
            schedule(repeat, first),
        )
        .map_err(|error| BudgetError::from(error).to_string())?;
        let (target, line) = self.sent(budget, line, pot, to_budget)?;
        self.edit(&target, |budget| {
            let id = budget.add_line(line).map_err(|error| error.to_string())?;
            Ok(id.to_string())
        })
    }

    /// From `from` (`2026-11-05`) on, the money of cost line `line`, seen
    /// in `budget`, goes into pot `pot`, or to budget `to_budget`, or
    /// neither, instead. Before, it went where it did: the line is cut in
    /// two (see `Budget::take_line_from`), the part from `from` on being a
    /// new line with the same amounts and due days, written where the
    /// money now goes, as `add_line` would.
    pub fn redirect_line(
        &self,
        budget: String,
        line: String,
        from: String,
        pot: Option<String>,
        to_budget: Option<String>,
    ) -> Result<(), String> {
        let id = LineId(parse_id(&line)?);
        let from = date(&from)?;
        let pot = pot.map(|id| parse_id(&id).map(PotId)).transpose()?;
        let home = self.line_home(budget.clone(), id)?;
        let mut held = self.load(&home)?;
        let line = held.line(id).ok_or(BudgetError::UnknownLine.to_string())?;
        // Seen from `budget`, a line held there is what it is; one held
        // elsewhere is a contribution this member pays: a cost.
        if home == budget && line.kind != Kind::Cost {
            return Err("only a cost's money goes into a pot or another budget".into());
        }
        let mut rest = held
            .take_line_from(id, from)
            .map_err(|error| error.to_string())?;
        // A cost as seen from `budget`, where it now leaves from.
        rest.kind = Kind::Cost;
        let (target, rest) = self.sent(budget, rest, pot, to_budget)?;
        // Every check done in memory first, then saved: the budget it goes
        // to, and the one it leaves (they may be the same).
        if target == home {
            held.add_line(rest).map_err(|error| error.to_string())?;
        } else {
            let mut there = self.load(&target)?;
            there.add_line(rest).map_err(|error| error.to_string())?;
            self.lock().save(&there).map_err(stored)?;
        }
        self.lock().save(&held).map_err(stored)?;
        Ok(())
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
        pot: Option<String>,
        from_pot: bool,
    ) -> Result<String, String> {
        let day = date(&date_text)?;
        let line = line.map(|id| parse_id(&id).map(LineId)).transpose()?;
        let pot = pot_move(pot, from_pot)?;
        // On a line seen here but held by another budget (a planned
        // contribution), the transaction goes there, as that line's: an
        // income this member paid, in its category.
        let mut target = budget.clone();
        let mut kind = kind(income);
        let mut category = category;
        let mut payer = None;
        if let Some(id) = line
            && let Some(other) = self.holder_of_line(&budget, id)?
            && let Some(held) = other.line(id)
        {
            kind = held.kind;
            category = held.category.clone();
            payer = held.payer.clone();
            target = other.id.to_string();
        }
        self.edit(&target, |budget| {
            let mut transaction = Transaction::new(
                day,
                kind,
                Money::new(amount, budget.currency()),
                category,
                line,
            );
            transaction.note = note;
            transaction.pot = pot;
            transaction.payer = payer;
            let id = budget
                .add_transaction(transaction)
                .map_err(|error| error.to_string())?;
            Ok(id.to_string())
        })
    }

    /// From `from` on, line `line` amounts to `amount` cents, due as
    /// `repeat` says from `from`; the months before keep the old amount.
    pub fn change_line(
        &self,
        budget: String,
        line: String,
        amount: i64,
        repeat: Repeat,
        from: String,
    ) -> Result<(), String> {
        let id = LineId(parse_id(&line)?);
        let from = date(&from)?;
        let budget = self.line_home(budget, id)?;
        self.change(&budget, |budget| {
            let amount = Money::new(amount, budget.currency());
            // Still due on the same days: the new schedule follows on from
            // the line's first due date. `from` only says from when the
            // change applies (a salary due on the 30th, changed from the
            // 17th, stays due on the 30th).
            let first = budget
                .line(id)
                .and_then(|line| line.revision_on(from).or(line.revisions().last()))
                .map_or(from, |revision| revision.schedule.first());
            budget.change_line(id, from, amount, schedule(repeat, first))
        })
    }

    /// Line `line` of `budget`, with all its amounts over time.
    pub fn line_detail(&self, budget: String, line: String) -> Result<LineDetail, String> {
        let id = LineId(parse_id(&line)?);
        let holder = self.holder_of_line(&budget, id)?;
        // `as_ref` borrows the Budget inside the Option, so `holder` can
        // still be moved into `budget` below.
        let held_in = holder.as_ref().map(|holder| BudgetChoice {
            id: holder.id.to_string(),
            name: holder.name.clone(),
        });
        let budget = match holder {
            Some(holder) => holder,
            None => self.load(&budget)?,
        };
        let line = budget
            .line(id)
            .ok_or_else(|| BudgetError::UnknownLine.to_string())?;
        Ok(LineDetail {
            id: line.id.to_string(),
            name: line.name.clone(),
            category: line.category.clone(),
            // Held elsewhere, it's this member's contribution: a cost.
            income: held_in.is_none() && line.kind == Kind::Income,
            held_in,
            pot: line.pot.map(|pot| pot.to_string()),
            ends_on: line.ends_on().map(|day| day.to_string()),
            revisions: line
                .revisions()
                .iter()
                .map(|revision| RevisionRow {
                    from: revision.from.to_string(),
                    amount: revision.amount.minor_units(),
                    repeat: repeat_of(revision.schedule),
                })
                .collect(),
        })
    }

    /// Sets one amount of line `line`: the one from `replacing` if given
    /// (possibly moved to `from`), else a new one from `from`. Others stay.
    /// `repeat`: how it repeats, or `None` to keep how it did.
    ///
    /// The due days stay the line's: an amount keeps its own, a new or
    /// moved one follows the amount before it, and the first amount's date
    /// is the line's first due date.
    #[allow(clippy::too_many_arguments)] // the fields of a revision
    pub fn set_revision(
        &self,
        budget: String,
        line: String,
        replacing: Option<String>,
        from: String,
        amount: i64,
        repeat: Option<Repeat>,
    ) -> Result<(), String> {
        let id = LineId(parse_id(&line)?);
        let replacing = replacing.map(|day| date(&day)).transpose()?;
        let from = date(&from)?;
        let budget = self.line_home(budget, id)?;
        self.change(&budget, |budget| {
            let line = budget.line(id).ok_or(BudgetError::UnknownLine)?;
            let revisions = line.revisions();
            // The amount edited, if any, and the last other one before
            // `from`.
            let old = revisions
                .iter()
                .find(|revision| Some(revision.from) == replacing);
            // `rfind`: the first match from the end, the revisions being
            // in date order.
            let previous = revisions
                .iter()
                .rfind(|revision| Some(revision.from) != replacing && revision.from < from);
            let first = match (old, previous) {
                (Some(old), _) if old.from == from => old.schedule.first(),
                (_, Some(previous)) => previous.schedule.first(),
                (_, None) => from, // the new start
            };
            let schedule = match repeat {
                Some(repeat) => schedule(repeat, first),
                // `or` falls back on the next Option; the line has at least
                // one revision, so `revisions[0]` exists.
                None => old
                    .or(previous)
                    .unwrap_or(&revisions[0])
                    .schedule
                    .starting(first),
            };
            let amount = Money::new(amount, budget.currency());
            budget.set_revision(id, replacing, from, amount, schedule)
        })
    }

    /// Removes the amount of line `line` starting on `from`.
    pub fn remove_revision(
        &self,
        budget: String,
        line: String,
        from: String,
    ) -> Result<(), String> {
        let id = LineId(parse_id(&line)?);
        let from = date(&from)?;
        let budget = self.line_home(budget, id)?;
        self.change(&budget, |budget| budget.remove_revision(id, from))
    }

    /// Renames line `line`, or moves it to another category.
    pub fn rename_line(
        &self,
        budget: String,
        line: String,
        name: String,
        category: String,
    ) -> Result<(), String> {
        let id = LineId(parse_id(&line)?);
        let budget = self.line_home(budget, id)?;
        self.change(&budget, |budget| budget.rename_line(id, name, category))
    }

    /// Line `line` stops after `last_day`, or goes on again with `None`.
    pub fn end_line(
        &self,
        budget: String,
        line: String,
        last_day: Option<String>,
    ) -> Result<(), String> {
        let id = LineId(parse_id(&line)?);
        let last_day = last_day.map(|day| date(&day)).transpose()?;
        let budget = self.line_home(budget, id)?;
        self.change(&budget, |budget| budget.end_line(id, last_day))
    }

    /// Removes line `line`, refused while transactions realise it.
    pub fn remove_line(&self, budget: String, line: String) -> Result<(), String> {
        let id = LineId(parse_id(&line)?);
        let budget = self.line_home(budget, id)?;
        self.change(&budget, |budget| budget.remove_line(id))
    }

    /// Changes transaction `transaction`, seen in `budget`, to the values
    /// given (as for `add_transaction`); it keeps its identifier and its
    /// payer or payee. One held by another budget (a contribution seen in
    /// a personal budget) changes there, and only in its date, amount and
    /// note: its kind, category, line and pot are that budget's.
    #[allow(clippy::too_many_arguments)] // the fields of a transaction
    pub fn change_transaction(
        &self,
        budget: String,
        transaction: String,
        date_text: String,
        income: bool,
        amount: i64,
        category: String,
        line: Option<String>,
        note: String,
        pot: Option<String>,
        from_pot: bool,
    ) -> Result<(), String> {
        let id = TransactionId(parse_id(&transaction)?);
        let day = date(&date_text)?;
        let held_here = self
            .load(&budget)?
            .transactions()
            .iter()
            .any(|transaction| transaction.id == id);
        // The budget to change, and whether all of it can change there.
        let (target, all) = if held_here {
            (budget, true)
        } else {
            let holder = self
                .lock()
                .budget_of_transaction(id)
                .map_err(stored)?
                .ok_or_else(|| BudgetError::UnknownTransaction.to_string())?;
            (holder.id.to_string(), false)
        };
        let line = line.map(|id| parse_id(&id).map(LineId)).transpose()?;
        let pot = pot_move(pot, from_pot)?;
        self.change(&target, |budget| {
            // `.cloned()`: an owned copy to edit, the original untouched
            // until `replace_transaction` accepts the copy.
            let mut edited = budget
                .transactions()
                .iter()
                .find(|transaction| transaction.id == id)
                .cloned()
                .ok_or(BudgetError::UnknownTransaction)?;
            edited.date = day;
            edited.amount = Money::new(amount, budget.currency());
            edited.note = note;
            if all {
                edited.kind = kind(income);
                edited.category = category;
                edited.line = line;
                edited.pot = pot;
            }
            budget.replace_transaction(edited)
        })
    }

    /// Removes transaction `transaction`: from the budget that holds it,
    /// if it's seen here as a reflection.
    /// What was deleted from `budget` in the last 30 days, the latest
    /// first (doc/scope.md: Corbeille).
    pub fn trash(&self, budget: String) -> Result<Vec<TrashRow>, String> {
        let id = BudgetId(
            budget
                .parse()
                .map_err(|_| format!("not a budget: {budget}"))?,
        );
        let since = self.trash_since(&budget)?;
        // In its own statement: the lock is released at the `;` (see
        // `export_on`).
        let trash = self.lock().trash(id, since).map_err(stored)?;
        Ok(trash
            .into_iter()
            .map(|(when, item)| trash_row(when, item))
            .collect())
    }

    /// Puts line or transaction `item`, deleted in the last 30 days, back:
    /// in `budget`, or in the budget it was deleted from if it's not
    /// there (a contribution removed from a personal budget). Checked like
    /// a new one: a transaction whose line is in the trash too waits for
    /// its line.
    pub fn restore(&self, budget: String, item: String) -> Result<(), String> {
        let ids = self.lock().budgets().map_err(stored)?;
        // `budget` first, then the others: `partition` splits the list in
        // two by a test, like `std::partition_copy`.
        let (first, others): (Vec<_>, Vec<_>) = ids
            .into_iter()
            .partition(|(id, _)| id.to_string() == budget);
        for (id, _) in first.into_iter().chain(others) {
            let since = self.trash_since(&id.to_string())?;
            let trash = self.lock().trash(id, since).map_err(stored)?;
            let found = trash.into_iter().find_map(|(_, trashed)| {
                let matches = match &trashed {
                    Trashed::Line(line) => line.id.to_string() == item,
                    Trashed::Transaction(transaction) => transaction.id.to_string() == item,
                };
                matches.then_some(trashed)
            });
            if let Some(trashed) = found {
                return self.change(&id.to_string(), |budget| match trashed {
                    Trashed::Line(line) => budget.add_line(line).map(|_| ()),
                    Trashed::Transaction(transaction) => budget
                        .add_transaction(transaction)
                        .map(|_| ())
                        .map_err(|error| {
                            if error == BudgetError::UnknownLine {
                                BudgetError::LineGone
                            } else {
                                error
                            }
                        }),
                });
            }
        }
        Err("it's no longer in the trash".into())
    }

    /// Empties the trash of `budget` on this device: what was deleted
    /// until now can no longer be restored here. (The deletions stay, as
    /// every change does, for the other devices.)
    pub fn empty_trash(&self, budget: String) -> Result<(), String> {
        let now = jiff::Timestamp::now().as_millisecond().to_string();
        self.lock()
            .set_setting(&format!("trash-emptied/{budget}"), &now)
            .map_err(stored)
    }

    /// Since when deletions from `budget` can be restored (milliseconds):
    /// the last 30 days, or since its trash was emptied.
    fn trash_since(&self, budget: &str) -> Result<u64, String> {
        let days = jiff::Timestamp::now().as_millisecond() - TRASH_DAYS * 86_400_000;
        // In its own statement, as above: `self.lock()` lives until `;`.
        let emptied = self
            .lock()
            .setting(&format!("trash-emptied/{budget}"))
            .map_err(stored)?;
        let emptied = emptied
            .and_then(|millis| millis.parse::<i64>().ok())
            .unwrap_or(0);
        // The later of the two, never below zero.
        Ok(u64::try_from(days.max(emptied)).unwrap_or(0))
    }

    pub fn remove_transaction(&self, budget: String, transaction: String) -> Result<(), String> {
        let id = TransactionId(parse_id(&transaction)?);
        let held_here = self
            .load(&budget)?
            .transactions()
            .iter()
            .any(|transaction| transaction.id == id);
        let budget = match self.lock().budget_of_transaction(id).map_err(stored)? {
            Some(holder) if !held_here => holder.id.to_string(),
            _ => budget,
        };
        self.change(&budget, |budget| budget.remove_transaction(id))
    }

    /// Adds a pot, with a goal of `goal` cents by `by` if given; returns
    /// its identifier.
    pub fn add_pot(
        &self,
        budget: String,
        name: String,
        goal: Option<i64>,
        by: Option<String>,
    ) -> Result<String, String> {
        let by = by.map(|day| date(&day)).transpose()?;
        self.edit(&budget, |budget| {
            let mut pot = Pot::new(name);
            pot.goal = goal.map(|cents| Goal {
                amount: Money::new(cents, budget.currency()),
                by,
            });
            Ok(budget.add_pot(pot).to_string())
        })
    }

    /// Renames budget `budget`.
    pub fn rename_budget(&self, budget: String, name: String) -> Result<(), String> {
        self.change(&budget, |budget| {
            budget.name = name;
            Ok(())
        })
    }

    /// Makes `budget` this member's personal budget, or not: their
    /// contributions and advances in shared budgets show in it.
    pub fn set_personal(&self, budget: String, personal: bool) -> Result<(), String> {
        self.change(&budget, |budget| {
            budget.personal = personal;
            Ok(())
        })
    }

    /// Writes every budget as JSON, and their transactions as CSV, into
    /// `folder` (see `bougette_core::export::files`); returns the paths
    /// written.
    pub fn export_files(&self, folder: String) -> Result<Vec<String>, String> {
        self.export_on(&folder, today())
    }

    // The same, with `today` given: for the tests.

    fn export_on(&self, folder: &str, today: Date) -> Result<Vec<String>, String> {
        // The list first, in its own statement: the lock is released at its
        // `;`. In the `for` line itself, the lock (a temporary) would last
        // the whole loop, and `load` locking again would wait forever (a
        // Mutex can't be locked twice by the same thread).
        let ids = self.lock().budgets().map_err(stored)?;
        let mut budgets = Vec::new();
        for (id, _) in ids {
            budgets.push(self.load(&id.to_string())?);
        }
        let mut paths = Vec::new();
        for (name, contents) in bougette_core::export::files(&budgets, today) {
            let path = std::path::Path::new(folder).join(name);
            std::fs::write(&path, contents)
                .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
            paths.push(path.display().to_string());
        }
        Ok(paths)
    }

    fn budgets_on(&self, today: Date) -> Result<Vec<BudgetSummary>, String> {
        let month = Month::of(today);
        let mut store = self.lock();
        let mut summaries = Vec::new();
        for (id, _) in store.budgets().map_err(stored)? {
            let Some(budget) = store.load(id).map_err(stored)? else {
                continue;
            };
            // With what a personal budget shows of the others.
            let budget = store.shown(budget).map_err(stored)?;
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
        let budget = self.shown(budget)?;
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
        let own = self.load(budget)?;
        let month = parse_month(month)?;
        // The budget's own lines and transactions, before reflections are
        // added: the others are reflections.
        let own_lines: HashSet<LineId> = own.lines().iter().map(|line| line.id).collect();
        let own: HashSet<TransactionId> = own.transactions().iter().map(|t| t.id).collect();
        let budget = self.shown(budget)?;
        let destinations = self.destinations(&budget)?;
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
                // The revision in force at the end of the month (a change
                // from the 5th shows from that month on), or the last one
                // if the line ended earlier.
                let revision = line
                    .revision_on(month.last_day())
                    .or(line.revisions().last());
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
                    amount: revision.map_or(0, |revision| revision.amount.minor_units()),
                    repeat: revision.and_then(|revision| repeat_of(revision.schedule)),
                    ends_on: line.ends_on().map(|day| day.to_string()),
                    pot: line.pot.map(|pot| pot.to_string()),
                    reflected: !own_lines.contains(&line.id),
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
                reflected: !own.contains(&transaction.id),
                date: transaction.date.to_string(),
                income: transaction.kind == Kind::Income,
                amount: transaction.amount.minor_units(),
                category: transaction.category.clone(),
                line: transaction
                    .line
                    .and_then(|id| budget.line(id))
                    .map(|line| line.name.clone()),
                line_id: transaction.line.map(|id| id.to_string()),
                note: transaction.note.clone(),
                pot: match transaction.pot {
                    Some(PotMove::Into(pot) | PotMove::PaidFrom(pot)) => Some(pot.to_string()),
                    None => None,
                },
                from_pot: matches!(transaction.pot, Some(PotMove::PaidFrom(_))),
            })
            .collect();
        let due = budget.plan(month, View::Due);
        let actual = budget.actual(month);
        let pots = budget
            .pots()
            .iter()
            .map(|pot| PotRow {
                id: pot.id.to_string(),
                name: pot.name.clone(),
                balance: budget
                    .pot_balance(pot.id, month.last_day())
                    .map_or(0, |balance| balance.minor_units()),
                goal: pot.goal.map(|goal| goal.amount.minor_units()),
                goal_by: pot.goal.and_then(|goal| goal.by).map(|day| day.to_string()),
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
            destinations,
            // `map_or`: the Option's value transformed, or the default.
            first_year: budget
                .first_day()
                .map_or(today.year(), |day| day.year().min(today.year())),
        })
    }

    /// The budgets a new cost line of `budget` can send its money to (see
    /// `MonthView::destinations`).
    fn destinations(&self, budget: &Budget) -> Result<Vec<BudgetChoice>, String> {
        if !budget.personal {
            return Ok(Vec::new());
        }
        let store = self.lock();
        let mut choices = Vec::new();
        for (id, name) in store.budgets().map_err(stored)? {
            if id != budget.id
                && let Some(other) = store.load(id).map_err(stored)?
                && other.currency() == budget.currency()
            {
                choices.push(BudgetChoice {
                    id: id.to_string(),
                    name,
                });
            }
        }
        Ok(choices)
    }

    /// Where `line`, a line of `budget` (a cost if it goes somewhere), is
    /// written: in `budget`, feeding pot `pot` if given; or, for a cost
    /// going to budget `to_budget`, there, as this member's planned
    /// contribution: an income they pay, shown here as a cost (see
    /// `MonthView::destinations`). Returns that budget and the line as
    /// written there.
    fn sent(
        &self,
        budget: String,
        mut line: Line,
        pot: Option<PotId>,
        to_budget: Option<String>,
    ) -> Result<(String, Line), String> {
        line.pot = pot;
        line.payer = None;
        // `let ... else`: the else block runs when the pattern doesn't
        // match (here, no other budget), and must leave the function.
        let Some(other) = to_budget else {
            return Ok((budget, line));
        };
        if line.kind != Kind::Cost || pot.is_some() {
            return Err("only a cost, without a pot, goes to another budget".into());
        }
        if self.load(&other)?.currency() != self.load(&budget)?.currency() {
            return Err("the other budget has another currency".into());
        }
        // Seen from the other budget, a contribution comes in.
        line.kind = Kind::Income;
        line.payer = Some(MemberId(self.lock().member().map_err(stored)?));
        Ok((other, line))
    }

    /// The other budget holding line `id`, if `budget` shows it without
    /// holding it (a planned contribution seen in a personal budget).
    fn holder_of_line(&self, budget: &str, id: LineId) -> Result<Option<Budget>, String> {
        if self.load(budget)?.line(id).is_some() {
            return Ok(None);
        }
        self.lock().budget_of_line(id).map_err(stored)
    }

    /// The budget to change line `id`, seen in `budget`: the one holding
    /// it.
    fn line_home(&self, budget: String, id: LineId) -> Result<String, String> {
        Ok(match self.holder_of_line(&budget, id)? {
            Some(holder) => holder.id.to_string(),
            None => budget,
        })
    }

    /// Budget `id` as this member sees it (see `Store::shown`).
    fn shown(&self, id: &str) -> Result<Budget, String> {
        let budget = self.load(id)?;
        self.lock().shown(budget).map_err(stored)
    }

    pub(crate) fn load(&self, id: &str) -> Result<Budget, String> {
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

    /// Like `edit`, for a change the budget may refuse with a
    /// BudgetError, shown in words.
    fn change(
        &self,
        id: &str,
        change: impl FnOnce(&mut Budget) -> Result<(), BudgetError>,
    ) -> Result<(), String> {
        self.edit(id, |budget| {
            change(budget).map_err(|error| error.to_string())?;
            Ok(String::new())
        })
        .map(|_| ())
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, Store> {
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

/// The schedule of a line repeating as `repeat` from `first`.
fn schedule(repeat: Repeat, first: Date) -> Schedule {
    let months = |every| Schedule::Months {
        every: NonZeroU8::new(every).expect("not zero"),
        first,
    };
    match repeat {
        Repeat::Monthly => months(1),
        Repeat::Quarterly => months(3),
        Repeat::Yearly => months(12),
        Repeat::Weekly => Schedule::Weekly { first },
        Repeat::Once => Schedule::Once { on: first },
    }
}

/// The reverse, if the app can show it.
fn repeat_of(schedule: Schedule) -> Option<Repeat> {
    match schedule {
        Schedule::Months { every, .. } => match every.get() {
            1 => Some(Repeat::Monthly),
            3 => Some(Repeat::Quarterly),
            12 => Some(Repeat::Yearly),
            _ => None,
        },
        Schedule::Weekly { .. } => Some(Repeat::Weekly),
        Schedule::Once { .. } => Some(Repeat::Once),
    }
}

fn parse_id(text: &str) -> Result<uuid::Uuid, String> {
    text.parse()
        .map_err(|_| format!("not an identifier: {text}"))
}

/// Money moving into pot `pot` (an allocation), or paid from it if
/// `from_pot`; `None` without a pot.
fn pot_move(pot: Option<String>, from_pot: bool) -> Result<Option<PotMove>, String> {
    pot.map(|id| {
        parse_id(&id).map(|id| {
            if from_pot {
                PotMove::PaidFrom(PotId(id))
            } else {
                PotMove::Into(PotId(id))
            }
        })
    })
    .transpose()
}

fn kind(income: bool) -> Kind {
    if income { Kind::Income } else { Kind::Cost }
}

/// How long deleted lines and transactions can be restored.
const TRASH_DAYS: i64 = 30;

/// A trash entry as the app shows it, deleted at `when` (milliseconds).
fn trash_row(when: u64, item: Trashed) -> TrashRow {
    // The day of deletion where the user is.
    let deleted_on = i64::try_from(when)
        .ok()
        .and_then(|millis| jiff::Timestamp::from_millisecond(millis).ok())
        .map(|time| {
            time.to_zoned(jiff::tz::TimeZone::system())
                .date()
                .to_string()
        })
        .unwrap_or_default();
    match item {
        Trashed::Line(line) => TrashRow {
            id: line.id.to_string(),
            line: true,
            deleted_on,
            date: None,
            income: line.kind == Kind::Income,
            // `last()` exists: a line has at least one revision.
            amount: line
                .revisions()
                .last()
                .map_or(0, |r| r.amount.minor_units()),
            name: line.name,
            detail: line.category,
        },
        Trashed::Transaction(transaction) => TrashRow {
            id: transaction.id.to_string(),
            line: false,
            deleted_on,
            date: Some(transaction.date.to_string()),
            income: transaction.kind == Kind::Income,
            amount: transaction.amount.minor_units(),
            name: transaction.category,
            detail: transaction.note,
        },
    }
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

pub(crate) fn stored(error: StoreError) -> String {
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
        App::with(Store::open_in_memory().unwrap()).unwrap()
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
            None,
            None,
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
                None,
                None,
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
            None,
            None,
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
            None,
            false,
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

    #[test]
    fn lines_change_end_and_pots_fill() {
        let app = app();
        let id = household(&app);
        let today = day(2026, 11, 16);
        let rent = |app: &App, month: &str| {
            let view = app.month_on(&id, month, today).unwrap();
            view.lines
                .into_iter()
                .find(|line| line.name == "Rent")
                .unwrap()
        };
        let line = rent(&app, "2026-11").id;
        assert_eq!(rent(&app, "2026-11").repeat, Some(Repeat::Monthly));

        // The rent goes up in December; November keeps the old amount.
        app.change_line(
            id.clone(),
            line.clone(),
            95000,
            Repeat::Monthly,
            "2026-12-05".into(),
        )
        .unwrap();
        assert_eq!(rent(&app, "2026-11").amount, 90000);
        assert_eq!(rent(&app, "2026-12").amount, 95000);
        app.end_line(id.clone(), line.clone(), Some("2027-06-30".into()))
            .unwrap();
        assert_eq!(rent(&app, "2026-12").ends_on.as_deref(), Some("2027-06-30"));
        // Its November payment keeps it from being removed, in words.
        let refused = app.remove_line(id.clone(), line.clone()).unwrap_err();
        assert!(refused.contains("has transactions"), "{refused}");
        let payment = app.month_on(&id, "2026-11", today).unwrap().transactions[0].clone();
        assert!(!payment.reflected);
        app.remove_transaction(id.clone(), payment.id).unwrap();
        app.remove_line(id.clone(), line).unwrap();

        // A pot with a goal, and 100 put in it.
        let bike = app
            .add_pot(
                id.clone(),
                "Bike".into(),
                Some(120000),
                Some("2027-06-30".into()),
            )
            .unwrap();
        app.add_transaction(
            id.clone(),
            "2026-11-02".into(),
            false,
            10000,
            "Savings".into(),
            None,
            String::new(),
            Some(bike),
            false,
        )
        .unwrap();
        let pots = app.month_on(&id, "2026-11", today).unwrap().pots;
        assert_eq!(pots[0].balance, 10000);
        assert_eq!(pots[0].goal_by.as_deref(), Some("2027-06-30"));
    }

    #[test]
    fn a_line_going_to_another_budget_is_one_line_seen_from_both() {
        let app = app();
        let today = day(2026, 11, 16);
        let home = app.create_budget("Home".into(), "EUR".into()).unwrap();
        app.set_personal(home.clone(), true).unwrap();
        let flat = app.create_budget("Flat".into(), "EUR".into()).unwrap();
        // The flat budget is offered as a destination, from the personal
        // budget only.
        let view = app.month_on(&home, "2026-11", today).unwrap();
        assert_eq!(view.destinations.len(), 1);
        assert_eq!(view.destinations[0].name, "Flat");
        assert!(
            app.month_on(&flat, "2026-11", today)
                .unwrap()
                .destinations
                .is_empty()
        );

        // 100 a month from home to the flat, entered from home.
        let line = app
            .add_line(
                home.clone(),
                "Flat".into(),
                "Contributions".into(),
                false,
                10000,
                Repeat::Monthly,
                "2026-01-01".into(),
                None,
                Some(flat.clone()),
            )
            .unwrap();
        // The flat sees an income, home a cost: the same line.
        let line_in = |budget: &str| {
            app.month_on(budget, "2026-11", today)
                .unwrap()
                .lines
                .into_iter()
                .find(|row| row.id == line)
                .unwrap()
        };
        let (at_flat, at_home) = (line_in(&flat), line_in(&home));
        assert!(at_flat.income && !at_flat.reflected);
        assert!(!at_home.income && at_home.reflected);
        assert_eq!((at_home.category.as_str(), at_home.due), ("Flat", 10000));

        // November's transfer, entered from home on that line, lands in
        // the flat and counts once, against the line, in both.
        app.add_transaction(
            home.clone(),
            "2026-11-01".into(),
            false,
            10000,
            "Flat".into(),
            Some(line.clone()),
            String::new(),
            None,
            false,
        )
        .unwrap();
        assert_eq!(line_in(&flat).actual, 10000);
        assert_eq!(line_in(&home).actual, 10000);
        let home_view = app.month_on(&home, "2026-11", today).unwrap();
        assert_eq!(home_view.actual.costs, 10000);
        assert_eq!(home_view.transactions[0].line.as_deref(), Some("Flat"));
        // The home screen counts it too.
        let budgets = app.budgets_on(today).unwrap();
        let summary = budgets.iter().find(|b| b.id == home).unwrap();
        assert_eq!(summary.monthly_balance, -10000);

        // Changed from home, it changes in the flat.
        app.change_line(
            home.clone(),
            line.clone(),
            12000,
            Repeat::Monthly,
            "2026-12-01".into(),
        )
        .unwrap();
        let december = app.month_on(&flat, "2026-12", today).unwrap();
        assert_eq!(december.lines[0].amount, 12000);
        // The transfer is removed from home, then the line.
        let transfer = home_view.transactions[0].id.clone();
        app.remove_transaction(home.clone(), transfer).unwrap();
        app.remove_line(home.clone(), line).unwrap();
        assert!(
            app.month_on(&flat, "2026-11", today)
                .unwrap()
                .lines
                .is_empty()
        );

        // An income doesn't go to another budget.
        let refused = app
            .add_line(
                home.clone(),
                "Gift".into(),
                "Gifts".into(),
                true,
                100,
                Repeat::Once,
                "2026-11-20".into(),
                None,
                Some(flat),
            )
            .unwrap_err();
        assert!(refused.contains("only a cost"), "{refused}");
    }

    #[test]
    fn a_transaction_is_edited_where_it_is_held() {
        let app = app();
        let today = day(2026, 11, 16);
        let home = app.create_budget("Home".into(), "EUR".into()).unwrap();
        app.set_personal(home.clone(), true).unwrap();
        let flat = app.create_budget("Flat".into(), "EUR".into()).unwrap();
        let line = app
            .add_line(
                home.clone(),
                "Flat".into(),
                "Contributions".into(),
                false,
                10000,
                Repeat::Monthly,
                "2026-01-01".into(),
                None,
                Some(flat.clone()),
            )
            .unwrap();
        let add = |budget: &str, line: Option<String>, amount| {
            app.add_transaction(
                budget.into(),
                "2026-11-03".into(),
                false,
                amount,
                "Food".into(),
                line,
                String::new(),
                None,
                false,
            )
            .unwrap()
        };
        let groceries = add(&home, None, 4200);
        let transfer = add(&home, Some(line.clone()), 10000);
        let row = |budget: &str, id: &str| {
            app.month_on(budget, "2026-11", today)
                .unwrap()
                .transactions
                .into_iter()
                .find(|row| row.id == id)
                .unwrap()
        };
        let change = |id: &str, income, amount, category: &str, note: &str| {
            app.change_transaction(
                home.clone(),
                id.into(),
                "2026-11-04".into(),
                income,
                amount,
                category.into(),
                None,
                note.into(),
                None,
                false,
            )
        };

        // Home's own: anything changes, the identifier stays.
        change(&groceries, true, 4500, "Refunds", "back").unwrap();
        let edited = row(&home, &groceries);
        assert_eq!(
            (edited.date.as_str(), edited.income, edited.amount),
            ("2026-11-04", true, 4500)
        );
        assert_eq!(
            (edited.category.as_str(), edited.note.as_str()),
            ("Refunds", "back")
        );

        // The contribution, edited from home: changed in the flat, its
        // amount, date and note only (still the flat's income, on the line).
        change(&transfer, true, 12000, "Ignored", "November").unwrap();
        let there = row(&flat, &transfer);
        assert_eq!((there.amount, there.date.as_str()), (12000, "2026-11-04"));
        assert!(there.income);
        assert_eq!(there.line_id.as_deref(), Some(line.as_str()));
        assert_eq!(there.category, "Contributions");
        assert_eq!(row(&home, &transfer).amount, 12000);
    }

    #[test]
    fn what_is_removed_waits_in_the_trash() {
        let app = app();
        let today = day(2026, 11, 16);
        let id = app.create_budget("Flat".into(), "EUR".into()).unwrap();
        let rent = app
            .add_line(
                id.clone(),
                "Rent".into(),
                "Rent".into(),
                true,
                50000,
                Repeat::Monthly,
                "2026-11-05".into(),
                None,
                None,
            )
            .unwrap();
        let paid = app
            .add_transaction(
                id.clone(),
                "2026-11-06".into(),
                true,
                50000,
                "Rent".into(),
                Some(rent.clone()),
                "November".into(),
                None,
                false,
            )
            .unwrap();
        assert!(app.trash(id.clone()).unwrap().is_empty());

        // The payment, then the rent line, removed.
        app.remove_transaction(id.clone(), paid.clone()).unwrap();
        app.remove_line(id.clone(), rent.clone()).unwrap();
        let trash = app.trash(id.clone()).unwrap();
        assert_eq!(trash.len(), 2);
        assert!(trash[0].line && trash[0].name == "Rent" && trash[0].amount == 50000);
        assert!(!trash[1].line);
        assert_eq!(
            (trash[1].date.as_deref(), trash[1].detail.as_str()),
            (Some("2026-11-06"), "November")
        );
        assert_eq!(trash[1].deleted_on, today_text());

        // The payment can't come back before its line.
        let refused = app.restore(id.clone(), paid.clone()).unwrap_err();
        assert!(refused.contains("restore the line first"), "{refused}");
        app.restore(id.clone(), rent.clone()).unwrap();
        app.restore(id.clone(), paid.clone()).unwrap();
        let month = app.month_on(&id, "2026-11", today).unwrap();
        assert_eq!(month.lines[0].actual, 50000);
        assert!(app.trash(id.clone()).unwrap().is_empty());
        // Restored once only.
        let again = app.restore(id.clone(), paid.clone()).unwrap_err();
        assert!(again.contains("no longer in the trash"), "{again}");

        // Emptied: what was deleted until then is gone for good, what is
        // deleted after shows.
        app.remove_transaction(id.clone(), paid.clone()).unwrap();
        app.empty_trash(id.clone()).unwrap();
        assert!(app.trash(id.clone()).unwrap().is_empty());
        assert!(app.restore(id.clone(), paid).is_err());
        // The clock moves on by at least a millisecond.
        std::thread::sleep(std::time::Duration::from_millis(2));
        app.remove_line(id.clone(), rent).unwrap();
        assert_eq!(app.trash(id).unwrap().len(), 1);
    }

    /// Today where the tests run, as `2026-11-20`.
    fn today_text() -> String {
        jiff::Zoned::now().date().to_string()
    }

    #[test]
    fn every_budget_is_exported_into_a_folder() {
        let app = app();
        household(&app);
        app.create_budget("Flat".into(), "EUR".into()).unwrap();
        let folder = std::env::temp_dir().join(format!("bougette-export-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let paths = app
            .export_on(folder.to_str().unwrap(), day(2026, 11, 20))
            .unwrap();
        assert_eq!(paths.len(), 2);
        assert!(paths[0].ends_with("bougette-2026-11-20.json"));
        assert!(paths[1].ends_with("bougette-2026-11-20-operations.csv"));
        let json = std::fs::read_to_string(&paths[0]).unwrap();
        assert!(json.contains("\"name\": \"Flat\""));
        assert!(json.contains("\"name\": \"Household\""));
        let csv = std::fs::read_to_string(&paths[1]).unwrap();
        assert!(csv.starts_with("budget,date,kind,amount,"));
        // A folder that isn't there: an error in words, nothing panics.
        let missing = folder.join("nowhere");
        let refused = app.export_on(missing.to_str().unwrap(), day(2026, 11, 20));
        assert!(refused.unwrap_err().starts_with("cannot write"));
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn the_calendar_starts_at_the_first_entry() {
        let app = app();
        let today = day(2026, 11, 16);
        let id = app.create_budget("Flat".into(), "EUR".into()).unwrap();
        let first_year = || app.month_on(&id, "2026-11", today).unwrap().first_year;
        // Empty, or with entries to come only: this year.
        assert_eq!(first_year(), 2026);
        app.add_line(
            id.clone(),
            "Rent".into(),
            "Rent".into(),
            true,
            50000,
            Repeat::Monthly,
            "2027-01-05".into(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(first_year(), 2026);
        // A transaction back in 2021: the calendar goes back to it.
        app.add_transaction(
            id.clone(),
            "2021-03-25".into(),
            true,
            26870,
            "Rent".into(),
            None,
            String::new(),
            None,
            false,
        )
        .unwrap();
        assert_eq!(first_year(), 2021);
    }

    #[test]
    fn a_line_is_redirected_from_a_date_keeping_its_past() {
        let app = app();
        let today = day(2026, 11, 16);
        let home = app.create_budget("Home".into(), "EUR".into()).unwrap();
        app.set_personal(home.clone(), true).unwrap();
        let flat = app.create_budget("Flat".into(), "EUR".into()).unwrap();
        let savings = app
            .add_pot(home.clone(), "Savings".into(), None, None)
            .unwrap();
        // 100 a month to the flat since January, paid in November.
        let line = app
            .add_line(
                home.clone(),
                "Flat".into(),
                "Contributions".into(),
                false,
                10000,
                Repeat::Monthly,
                "2026-01-01".into(),
                None,
                Some(flat.clone()),
            )
            .unwrap();
        app.add_transaction(
            home.clone(),
            "2026-11-01".into(),
            false,
            10000,
            "Flat".into(),
            Some(line.clone()),
            String::new(),
            None,
            false,
        )
        .unwrap();
        let lines = |budget: &str, month: &str| app.month_on(budget, month, today).unwrap().lines;

        // Not from November: its transfer is on the line.
        let refused = app
            .redirect_line(
                home.clone(),
                line.clone(),
                "2026-11-01".into(),
                Some(savings.clone()),
                None,
            )
            .unwrap_err();
        assert!(refused.contains("choose a later date"), "{refused}");

        // From December, into the savings pot instead.
        app.redirect_line(
            home.clone(),
            line.clone(),
            "2026-12-01".into(),
            Some(savings.clone()),
            None,
        )
        .unwrap();
        // The flat keeps November, paid; nothing from December.
        assert_eq!(lines(&flat, "2026-11")[0].actual, 10000);
        assert!(lines(&flat, "2026-12").is_empty());
        // Home pays the pot from December, with a line of its own.
        let december = lines(&home, "2026-12");
        assert_eq!(december.len(), 1);
        let saved = &december[0];
        assert_eq!(saved.pot.as_ref(), Some(&savings));
        assert!(!saved.reflected && !saved.income);
        assert_eq!(
            (saved.due, saved.category.as_str()),
            (10000, "Contributions")
        );

        // And to the flat again from March.
        app.redirect_line(
            home.clone(),
            saved.id.clone(),
            "2027-03-01".into(),
            None,
            Some(flat.clone()),
        )
        .unwrap();
        assert_eq!(lines(&home, "2027-02")[0].pot.as_ref(), Some(&savings));
        let march = lines(&flat, "2027-03");
        assert!(march[0].income && march[0].due == 10000);
        let seen = &lines(&home, "2027-03")[0];
        assert!(seen.reflected);
        // In full, from home: a cost, held by the flat.
        let detail = app.line_detail(home.clone(), seen.id.clone()).unwrap();
        assert!(!detail.income && detail.pot.is_none());
        assert_eq!(
            detail.held_in.map(|held| held.name).as_deref(),
            Some("Flat")
        );

        // An income's money doesn't go anywhere.
        let salary = app
            .add_line(
                home.clone(),
                "Salary".into(),
                "Work".into(),
                true,
                300000,
                Repeat::Monthly,
                "2026-01-30".into(),
                None,
                None,
            )
            .unwrap();
        let refused = app
            .redirect_line(home, salary, "2026-12-01".into(), Some(savings), None)
            .unwrap_err();
        assert!(refused.contains("only a cost"), "{refused}");
    }

    #[test]
    fn a_line_changed_before_its_first_due_date_keeps_its_due_day() {
        let app = app();
        let id = app.create_budget("Home".into(), "EUR".into()).unwrap();
        // A salary first due on the 30th, changed on the 17th (what the
        // form offers by default).
        let salary = app
            .add_line(
                id.clone(),
                "Salary".into(),
                "Income".into(),
                true,
                300000,
                Repeat::Monthly,
                "2026-11-30".into(),
                None,
                None,
            )
            .unwrap();
        app.change_line(
            id.clone(),
            salary.clone(),
            310000,
            Repeat::Monthly,
            "2026-11-17".into(),
        )
        .unwrap();
        let line = app.load(&id).unwrap().lines()[0].clone();
        assert_eq!(line.starts_on(), day(2026, 11, 30));
        assert_eq!(line.revisions()[0].schedule.first(), day(2026, 11, 30));
        assert_eq!(line.revisions()[0].amount.minor_units(), 310000);

        // Changed again from mid-December: still due on the 30th.
        app.change_line(
            id.clone(),
            salary.clone(),
            320000,
            Repeat::Monthly,
            "2026-12-15".into(),
        )
        .unwrap();
        let line = app.load(&id).unwrap().lines()[0].clone();
        assert_eq!(line.revisions()[1].schedule.first(), day(2026, 11, 30));
        let december = app.month_on(&id, "2026-12", day(2026, 11, 17)).unwrap();
        assert_eq!(december.lines[0].due, 320000);

        // Ending it before it starts: refused, with what to do instead.
        let refused = app
            .end_line(id, salary, Some("2026-11-17".into()))
            .unwrap_err();
        assert!(refused.contains("remove it instead"), "{refused}");
    }

    #[test]
    fn a_line_is_edited_as_a_table_of_amounts() {
        let app = app();
        let id = app.create_budget("Home".into(), "EUR".into()).unwrap();
        let salary = app
            .add_line(
                id.clone(),
                "Salary".into(),
                "Income".into(),
                true,
                300000,
                Repeat::Monthly,
                "2026-01-30".into(),
                None,
                None,
            )
            .unwrap();
        let set = |replacing: Option<&str>, from: &str, amount| {
            app.set_revision(
                id.clone(),
                salary.clone(),
                replacing.map(String::from),
                from.into(),
                amount,
                None,
            )
        };
        // Two raises, entered on the 1st: still paid on the 30th.
        set(None, "2026-04-01", 310000).unwrap();
        set(None, "2026-09-01", 320000).unwrap();
        // The first raise was 3,150 in fact.
        set(Some("2026-04-01"), "2026-04-01", 315000).unwrap();
        let detail = app.line_detail(id.clone(), salary.clone()).unwrap();
        let rows: Vec<(&str, i64)> = detail
            .revisions
            .iter()
            .map(|row| (row.from.as_str(), row.amount))
            .collect();
        assert_eq!(
            rows,
            [
                ("2026-01-30", 300000),
                ("2026-04-01", 315000),
                ("2026-09-01", 320000)
            ]
        );
        assert_eq!(detail.revisions[1].repeat, Some(Repeat::Monthly));
        assert_eq!(detail.held_in, None);
        let line = app.load(&id).unwrap().lines()[0].clone();
        assert!(
            line.revisions()
                .iter()
                .all(|revision| revision.schedule.first() == day(2026, 1, 30))
        );

        // The start moves to February: the first due date with it.
        set(Some("2026-01-30"), "2026-02-27", 300000).unwrap();
        let line = app.load(&id).unwrap().lines()[0].clone();
        assert_eq!(line.starts_on(), day(2026, 2, 27));
        assert_eq!(line.revisions()[0].schedule.first(), day(2026, 2, 27));

        // A change removed, then a rename.
        app.remove_revision(id.clone(), salary.clone(), "2026-09-01".into())
            .unwrap();
        app.rename_line(id.clone(), salary.clone(), "Pay".into(), "Work".into())
            .unwrap();
        let detail = app.line_detail(id, salary).unwrap();
        assert_eq!(detail.revisions.len(), 2);
        assert_eq!(
            (detail.name.as_str(), detail.category.as_str()),
            ("Pay", "Work")
        );
    }
}
