//! The commands: each reads its words, calls the domain and the store, and
//! returns the text to print. Nothing here computes a budget figure: the
//! numbers all come from `bougette-core`.

use std::collections::HashMap;
use std::num::NonZeroU8;

use bougette_core::budget::{Budget, BudgetError, Comparison, Status};
use bougette_core::id::{BudgetId, MemberId};
use bougette_core::line::{Kind, Line, View};
use bougette_core::money::{Currency, Money};
use bougette_core::month::Month;
use bougette_core::pot::{Goal, Pot};
use bougette_core::schedule::Schedule;
use bougette_core::split::{SplitRule, split};
use bougette_core::transaction::{PotMove, Transaction};
use bougette_store::{Role, Store, StoreError};
use jiff::civil::Date;

/// The help text. `\` at the end of a line continues the string without
/// a line break.
pub const USAGE: &str = "\
Usage: bougette [--db FILE] [--today DATE] COMMAND

Budgets
  budgets                          list them, with their balances
  budget new NAME [CURRENCY]       EUR by default, fixed for good
  budget personal BUDGET           yours: your contributions, advances and
                                   repayments in shared budgets show in it
  show BUDGET [MONTH]              a month: plan, actual, forecast, pots
  year BUDGET [YEAR]               the balances, month by month

Lines (the plan)
  line add BUDGET NAME CATEGORY income|cost AMOUNT SCHEDULE FIRST [--pot POT]
         [--payer MEMBER]  paid by a member: a planned contribution if an
                           income, shown in their personal budget
  line change BUDGET LINE AMOUNT SCHEDULE FIRST    from FIRST on
  line end BUDGET LINE LAST_DAY|never
  line list BUDGET
  line remove BUDGET LINE

Transactions (the actual)
  tx add BUDGET DATE income|cost AMOUNT CATEGORY [--line LINE]
         [--into POT | --from POT] [--note TEXT]
         [--payer MEMBER]  paid by a member: a contribution if an income,
                           an advance if a cost
         [--payee MEMBER]  a cost paid to a member, e.g. a repayment
  tx list BUDGET [MONTH]
  tx remove BUDGET TRANSACTION

Pots
  pot add BUDGET NAME [GOAL [BY]]

Members
  name NAME                        how you appear to the other members
  members BUDGET                   who is in it, with which role
  invite BUDGET [--role admin|editor|reader] [--days N]
                                   an invitation to give (editor, 7 days)
  accept INVITATION [--username NAME]
                                   ask to join; NAME on the server if new
  role BUDGET MEMBER admin|editor|reader
  remove BUDGET MEMBER             they keep what they had, nothing new
  leave BUDGET

Devices and server
  id                               this device's and member's identifiers
  server join ADDRESS TOKEN USERNAME
                                   with an invitation from the operator
  sync [ADDRESS]                   with the server, or with your device at
                                   ADDRESS (ID@IP:PORT)
  serve [--port PORT]              let your other devices sync with this one
  device code                      the secret to link another device
  device link CODE                 make this new device one of yours
  recovery setup [--phrase TEXT]   lock a backup on the server with a
                                   recovery phrase (generated if not given)
  recover ADDRESS TOKEN USERNAME PHRASE...
                                   after losing all your devices, with a new
                                   invitation from the operator

Fair-split calculator (stores nothing)
  split TOTAL|BUDGET INCOME... [--rule income|equal] [--month MONTH]
                                   BUDGET: its smoothed costs of the month

SCHEDULE: monthly, quarterly, yearly, every-N-months, weekly or once, with
FIRST its first due date. Dates are written 2026-11-16, months 2026-11.
BUDGET, LINE, POT, TRANSACTION, MEMBER: a name, or the end of an
identifier; MEMBER can also be me.
Amounts: 12, 12.5 or -12.34 (a negative cost is a refund).
Database: --db FILE, or $XDG_DATA_HOME/bougette/bougette.sqlite.
";

/// The options, each followed by a value.
const OPTIONS: [&str; 16] = [
    "db", "today", "month", "pot", "line", "into", "from", "note", "rule", "port", "payer",
    "payee", "role", "days", "username", "phrase",
];

/// What every command may need.
// A struct holding a reference needs a lifetime parameter, `'a`: the
// Context borrows the store and can't outlive it. In C++ a struct with a
// `Store&` member could dangle; here the compiler rules that out.
pub struct Context<'a> {
    pub store: &'a mut Store,
    pub today: Date,
    pub colour: bool,
}

/// The options given as `--name value`, by name.
pub type Options = HashMap<String, String>;

/// Splits the arguments into words and `--name value` options.
pub fn parse_arguments(arguments: Vec<String>) -> Result<(Vec<String>, Options), String> {
    let mut words = Vec::new();
    let mut options = Options::new();
    // `into_iter` consumes the Vec and moves each String out: no copies.
    let mut arguments = arguments.into_iter();
    // `while let` loops as long as the pattern matches: until `next()`
    // returns `None`.
    while let Some(argument) = arguments.next() {
        match argument.strip_prefix("--") {
            Some(name) => {
                if !OPTIONS.contains(&name) {
                    return Err(format!("unknown option --{name}"));
                }
                let value = arguments
                    .next()
                    .ok_or_else(|| format!("--{name} needs a value"))?;
                options.insert(name.to_string(), value);
            }
            None => words.push(argument),
        }
    }
    Ok((words, options))
}

/// Runs the command in `words`.
pub fn run(words: &[&str], options: &Options, context: &mut Context) -> Result<String, String> {
    // An option's value, as `&str`.
    let option = |name: &str| options.get(name).map(String::as_str);
    // Slice patterns: `[a, b]` matches exactly two words and binds them,
    // `rest @ ..` binds whatever is left. The literal words select the
    // command; the arms are tried in order. The bound names are `&&str`
    // (references to the slice's elements); most uses dereference them
    // automatically, `*name` does it by hand.
    match words {
        ["budgets"] => budgets(context),
        ["budget", "new", name] => new_budget(context, name, "EUR"),
        ["budget", "new", name, currency] => new_budget(context, name, currency),
        ["budget", "personal", budget] => edit(context, budget, |budget| {
            budget.personal = true;
            Ok(format!("{} is your personal budget.\n", budget.name))
        }),
        ["name", name] => {
            stored(context.store.set_name(name))?;
            Ok(format!("You appear as {name} in your budgets.\n"))
        }
        ["members", budget] => list_members(context, budget),
        ["role", budget, member, role] => {
            let id = find_budget(context.store, budget)?.id;
            let member = find_member(context, id, member)?;
            let role = Role::parse(role).ok_or_else(|| format!("not a role: {role}"))?;
            stored(context.store.set_role(id, &member.0, role))?;
            Ok(format!("Role changed to {}.\n", role.as_str()))
        }
        ["remove", budget, member] => {
            let id = find_budget(context.store, budget)?.id;
            let member = find_member(context, id, member)?;
            stored(context.store.remove(id, &member.0))?;
            Ok("Removed. They keep what they had, and get nothing new.\n".to_string())
        }
        ["leave", budget] => {
            let id = find_budget(context.store, budget)?.id;
            let me = stored(context.store.member())?;
            stored(context.store.remove(id, &me))?;
            Ok("You left the budget.\n".to_string())
        }
        ["show", budget] => show(context, budget, None),
        ["show", budget, month] => show(context, budget, Some(month)),
        ["year", budget] => year(context, budget, None),
        ["year", budget, year_] => year(context, budget, Some(year_)),

        [
            "line",
            "add",
            budget,
            name,
            category,
            kind,
            amount,
            schedule,
            first,
        ] => {
            let id = find_budget(context.store, budget)?.id;
            let payer = option("payer")
                .map(|member| find_member(context, id, member))
                .transpose()?;
            // The closure is the change to make to the loaded budget.
            edit(context, budget, |budget| {
                let first = parse_date(first)?;
                let mut line = Line::new(
                    *name,
                    *category,
                    parse_kind(kind)?,
                    first,
                    parse_amount(amount, budget.currency())?,
                    parse_schedule(schedule, first)?,
                )
                .map_err(|error| explain(error.into()))?;
                if let Some(pot) = option("pot") {
                    line.pot = Some(find_pot(budget, pot)?.id);
                }
                line.payer = payer;
                let id = budget.add_line(line).map_err(explain)?;
                Ok(format!("Added line {name} ({}).\n", short(id)))
            })
        }
        ["line", "change", budget, line, amount, schedule, first] => {
            edit(context, budget, |budget| {
                let id = find_line(budget, line)?.id;
                let first = parse_date(first)?;
                let amount = parse_amount(amount, budget.currency())?;
                let schedule = parse_schedule(schedule, first)?;
                budget
                    .change_line(id, first, amount, schedule)
                    .map_err(explain)?;
                Ok(format!("Changed line {line} from {first}.\n"))
            })
        }
        ["line", "end", budget, line, last_day] => edit(context, budget, |budget| {
            let id = find_line(budget, line)?.id;
            let last_day = match *last_day {
                "never" => None,
                day => Some(parse_date(day)?),
            };
            budget.end_line(id, last_day).map_err(explain)?;
            Ok(match last_day {
                Some(day) => format!("Line {line} ends on {day}.\n"),
                None => format!("Line {line} goes on.\n"),
            })
        }),
        ["line", "list", budget] => list_lines(context, budget),
        ["line", "remove", budget, line] => edit(context, budget, |budget| {
            let id = find_line(budget, line)?.id;
            budget.remove_line(id).map_err(explain)?;
            Ok(format!("Removed line {line}.\n"))
        }),

        ["tx", "add", budget, day, kind, amount, category] => {
            // The members are resolved first: the closure below only sees
            // the budget, not the store.
            let id = find_budget(context.store, budget)?.id;
            let payer = option("payer")
                .map(|member| find_member(context, id, member))
                .transpose()?;
            let payee = option("payee")
                .map(|member| find_member(context, id, member))
                .transpose()?;
            edit(context, budget, |budget| {
                let mut transaction = Transaction::new(
                    parse_date(day)?,
                    parse_kind(kind)?,
                    parse_amount(amount, budget.currency())?,
                    *category,
                    None,
                );
                if let Some(line) = option("line") {
                    transaction.line = Some(find_line(budget, line)?.id);
                }
                // Matching a tuple of two Options covers the four cases.
                transaction.pot = match (option("into"), option("from")) {
                    (None, None) => None,
                    (Some(pot), None) => Some(PotMove::Into(find_pot(budget, pot)?.id)),
                    (None, Some(pot)) => Some(PotMove::PaidFrom(find_pot(budget, pot)?.id)),
                    (Some(_), Some(_)) => return Err("use --into or --from, not both".into()),
                };
                transaction.note = option("note").unwrap_or_default().to_string();
                transaction.payer = payer;
                transaction.payee = payee;
                let id = budget.add_transaction(transaction).map_err(explain)?;
                Ok(format!("Added transaction {}.\n", short(id)))
            })
        }
        ["tx", "list", budget] => list_transactions(context, budget, None),
        ["tx", "list", budget, month] => list_transactions(context, budget, Some(month)),
        ["tx", "remove", budget, transaction] => edit(context, budget, |budget| {
            let id = find_transaction(budget, transaction)?.id;
            budget.remove_transaction(id).map_err(explain)?;
            Ok(format!("Removed transaction {}.\n", short(id)))
        }),

        ["pot", "add", budget, name] => add_pot(context, budget, name, None, None),
        ["pot", "add", budget, name, goal] => add_pot(context, budget, name, Some(goal), None),
        ["pot", "add", budget, name, goal, by] => {
            add_pot(context, budget, name, Some(goal), Some(by))
        }

        // A match guard on a slice pattern: at least one income.
        ["split", source, incomes @ ..] if !incomes.is_empty() => {
            fair_split(context, source, incomes, option("rule"), option("month"))
        }
        _ => Err(format!("unknown command; see `bougette help`\n\n{USAGE}")),
    }
}

/// Loads budget `name`, applies `change` to it, and saves it if the change
/// succeeded.
// `impl FnOnce(&mut Budget) -> ...` accepts any closure that can be called
// once with the budget: a generic parameter, like a template taking a
// callable.
fn edit(
    context: &mut Context,
    name: &str,
    change: impl FnOnce(&mut Budget) -> Result<String, String>,
) -> Result<String, String> {
    let mut budget = find_budget(context.store, name)?;
    let message = change(&mut budget)?;
    stored(context.store.save(&budget))?;
    Ok(message)
}

fn new_budget(context: &mut Context, name: &str, currency: &str) -> Result<String, String> {
    let currency =
        Currency::from_code(currency).ok_or_else(|| format!("not a currency code: {currency}"))?;
    // The store gives the budget its identifier, derived from its
    // creator's key.
    let budget = stored(context.store.create(Budget::new(name, currency)))?;
    Ok(format!("Created budget {name} ({}).\n", short(budget.id)))
}

fn add_pot(
    context: &mut Context,
    budget: &str,
    name: &str,
    goal: Option<&str>,
    by: Option<&str>,
) -> Result<String, String> {
    edit(context, budget, |budget| {
        let mut pot = Pot::new(name);
        if let Some(amount) = goal {
            pot.goal = Some(Goal {
                amount: parse_amount(amount, budget.currency())?,
                // `transpose` turns an Option<Result> into a Result<Option>,
                // so `?` can apply: a bad date fails, no date is `None`.
                by: by.map(parse_date).transpose()?,
            });
        }
        budget.add_pot(pot);
        Ok(format!("Added pot {name}.\n"))
    })
}

/// The home screen: each budget with its monthly balance (smoothed plan)
/// and running balance at the end of the current month.
fn budgets(context: &mut Context) -> Result<String, String> {
    let month = Month::of(context.today);
    let mut out = String::new();
    for (id, name) in stored(context.store.budgets())? {
        let budget = find_budget(context.store, &id.to_string())?;
        let budget = stored(context.store.shown(budget))?;
        // `+=` on a String appends a `&str`; `&format!(...)` borrows the
        // temporary String as one.
        out += &format!(
            "{}  {:<24} monthly {:>11}  running {:>11}\n",
            short(id),
            name,
            amount(budget.plan(month, View::Smoothed).balance()),
            amount(budget.running_balance(month, context.today)),
        );
    }
    if out.is_empty() {
        out = "No budget yet: `bougette budget new NAME`.\n".to_string();
    }
    Ok(out)
}

/// One month of a budget: each line planned and actual, the unplanned
/// transactions, the totals, the running balance and the pots.
fn show(context: &mut Context, name: &str, month: Option<&str>) -> Result<String, String> {
    // A personal budget shows what its member pays or receives in the
    // other ones (computed, never saved).
    let budget = find_budget(context.store, name)?;
    let budget = stored(context.store.shown(budget))?;
    let month = match month {
        Some(month) => parse_month(month)?,
        None => Month::of(context.today),
    };
    let mut out = format!(
        "{}, {} ({})\n{:<34}{:>11}{:>11}{:>11}\n",
        budget.name,
        month_text(month),
        budget.currency().code(),
        "",
        "due",
        "smoothed",
        "actual"
    );
    // An array of tuples, iterated by value.
    for (kind, title) in [(Kind::Income, "Incomes"), (Kind::Cost, "Costs")] {
        out += title;
        out += "\n";
        for line in budget.lines().iter().filter(|line| line.kind == kind) {
            // `let ... else`: compare_line only fails for an unknown line.
            let Some(comparison) = budget.compare_line(line.id, month) else {
                continue;
            };
            let smoothed = line.amount_in(month, View::Smoothed);
            // A line that doesn't apply this month at all is left out.
            if smoothed.minor_units() == 0 && comparison == zero(&budget) {
                continue;
            }
            let actual = format!("{:>11}", amount(comparison.actual));
            out += &format!(
                "  {:<32}{:>11}{:>11}{}\n",
                format!("{} ({})", line.name, line.category),
                amount(comparison.planned),
                amount(smoothed),
                // Only costs get a colour (doc/scope.md).
                match kind {
                    Kind::Income => actual,
                    Kind::Cost => judged(actual, comparison, context.colour),
                }
            );
        }
    }
    let unplanned: Vec<&Transaction> = budget
        .transactions()
        .iter()
        .filter(|transaction| transaction.line.is_none() && month.contains(transaction.date))
        .collect();
    if !unplanned.is_empty() {
        out += "Unplanned\n";
        for transaction in unplanned {
            out += &format!("  {}\n", describe(&budget, transaction));
        }
    }

    let due = budget.plan(month, View::Due);
    let smoothed = budget.plan(month, View::Smoothed);
    let actual = budget.actual(month);
    let forecast = budget.forecast(month, context.today);
    let costs = Comparison {
        planned: due.costs,
        actual: actual.costs,
    };
    out += &format!(
        // Six spaces after "actual": the width of a cost's status word.
        "Totals{:<28}{:>11}{:>11}{:>11}      {:>11}\n",
        "", "due", "smoothed", "actual", "forecast"
    );
    out += &format!(
        "  {:<32}{:>11}{:>11}{:>11}      {:>11}\n",
        "Incomes",
        amount(due.incomes),
        amount(smoothed.incomes),
        amount(actual.incomes),
        amount(forecast.incomes)
    );
    out += &format!(
        "  {:<32}{:>11}{:>11}{}{:>11}\n",
        "Costs",
        amount(due.costs),
        amount(smoothed.costs),
        judged(
            format!("{:>11}", amount(actual.costs)),
            costs,
            context.colour
        ),
        amount(forecast.costs)
    );
    out += &format!(
        "  {:<32}{:>11}{:>11}{:>11}      {:>11}\n",
        "Balance",
        amount(due.balance()),
        amount(smoothed.balance()),
        amount(actual.balance()),
        amount(forecast.balance())
    );
    out += &format!(
        "Running balance at the end of {}: {}\n",
        month_text(month),
        amount(budget.running_balance(month, context.today))
    );

    if !budget.pots().is_empty() {
        out += "Pots\n";
    }
    for pot in budget.pots() {
        // `pot_balance` is `None` only for an unknown pot; Money has no
        // default value (which currency?), so give the fallback.
        let balance = budget
            .pot_balance(pot.id, month.last_day())
            .unwrap_or(Money::zero(budget.currency()));
        let goal = match pot.goal {
            Some(Goal { amount: goal, by }) => match by {
                Some(by) => format!(", goal {} by {by}", amount(goal)),
                None => format!(", goal {}", amount(goal)),
            },
            None => String::new(),
        };
        out += &format!("  {}: {}{goal}\n", pot.name, amount(balance));
    }
    Ok(out)
}

/// The twelve months of a year: the planned monthly balance, the expected
/// one (actual, forecast or plan, depending on the month) and the running
/// balance.
fn year(context: &mut Context, name: &str, year: Option<&str>) -> Result<String, String> {
    let budget = find_budget(context.store, name)?;
    let budget = stored(context.store.shown(budget))?;
    let year: i16 = match year {
        Some(text) => text.parse().map_err(|_| format!("not a year: {text}"))?,
        None => context.today.year(),
    };
    let current = Month::of(context.today);
    let mut month = Month::new(year, 1).ok_or_else(|| format!("not a year: {year}"))?;
    let mut out = format!(
        "{}, {year} ({})\n{:<10}{:>11}{:>11}{:<10}{:>11}\n",
        budget.name,
        budget.currency().code(),
        "",
        "planned",
        "expected",
        "",
        "running"
    );
    // `_`: the loop counter isn't used.
    for _ in 0..12 {
        let expected = budget.forecast(month, context.today);
        let mut cell = format!("{:>11}", amount(expected.balance()));
        // Overspending shows in past months and the current one; coming
        // months get blanks the width of a status word instead.
        if month <= current {
            let costs = Comparison {
                planned: budget.plan(month, View::Due).costs,
                actual: expected.costs,
            };
            cell = judged(cell, costs, context.colour);
        } else {
            cell += "      ";
        }
        let source = if month < current {
            "actual"
        } else if month == current {
            "forecast"
        } else {
            "plan"
        };
        out += &format!(
            "{:<10}{:>11}{cell} {:<9}{:>11}\n",
            month_text(month),
            amount(budget.plan(month, View::Smoothed).balance()),
            source,
            amount(budget.running_balance(month, context.today)),
        );
        month = month.next();
    }
    Ok(out)
}

/// Each line with its revisions and end date.
fn list_lines(context: &Context, name: &str) -> Result<String, String> {
    let budget = find_budget(context.store, name)?;
    let mut out = String::new();
    for line in budget.lines() {
        let pot = line
            .pot
            .and_then(|id| budget.pot(id))
            .map(|pot| format!(", into {}", pot.name))
            .unwrap_or_default();
        let end = line
            .ends_on()
            .map(|day| format!(", until {day}"))
            .unwrap_or_default();
        out += &format!(
            "{}  {} ({}), {}{pot}{end}\n",
            short(line.id),
            line.name,
            line.category,
            kind_text(line.kind)
        );
        for revision in line.revisions() {
            out += &format!(
                "          {} {}\n",
                amount(revision.amount),
                schedule_text(&revision.schedule)
            );
        }
    }
    Ok(out)
}

/// The transactions of a month, by date.
fn list_transactions(context: &Context, name: &str, month: Option<&str>) -> Result<String, String> {
    let budget = find_budget(context.store, name)?;
    let month = match month {
        Some(month) => parse_month(month)?,
        None => Month::of(context.today),
    };
    let mut transactions: Vec<&Transaction> = budget
        .transactions()
        .iter()
        .filter(|transaction| month.contains(transaction.date))
        .collect();
    // A stable sort on the date, like `std::stable_sort` with a key.
    transactions.sort_by_key(|transaction| transaction.date);
    let mut out = String::new();
    for transaction in transactions {
        let line = transaction
            .line
            .and_then(|id| budget.line(id))
            .map(|line| format!(", {}", line.name))
            .unwrap_or_default();
        out += &format!(
            "{}  {}{line}\n",
            short(transaction.id),
            describe(&budget, transaction)
        );
    }
    Ok(out)
}

/// The fair-split calculator: splits `source`, a total or a budget's
/// smoothed costs of the month (allocations included), between members
/// with the given incomes. The incomes are only used here, never stored.
fn fair_split(
    context: &Context,
    source: &str,
    incomes: &[&str],
    rule: Option<&str>,
    month: Option<&str>,
) -> Result<String, String> {
    let total = match Money::parse(source, Currency::EUR) {
        Some(total) => total,
        None => {
            let budget = find_budget(context.store, source)?;
            let month = match month {
                Some(month) => parse_month(month)?,
                None => Month::of(context.today),
            };
            budget.plan(month, View::Smoothed).costs
        }
    };
    let rule = match rule.unwrap_or("income") {
        "income" => SplitRule::ProportionalToIncome,
        "equal" => SplitRule::Equal,
        other => return Err(format!("not a split rule (income or equal): {other}")),
    };
    // Collecting into `Result<Vec<_>, _>` stops at the first error and
    // returns it; otherwise it gives all the values. `_` lets the compiler
    // infer the element and error types.
    let incomes: Vec<Money> = incomes
        .iter()
        .map(|income| parse_amount(income, total.currency()))
        .collect::<Result<_, _>>()?;
    let shares = split(total, &incomes, rule)
        .ok_or("cannot split: an income is negative, or they are all zero")?;
    let mut out = format!("To share: {total}\n");
    // `zip` pairs the two lists; `enumerate` numbers the pairs from 0.
    for (number, (income, share)) in incomes.iter().zip(&shares).enumerate() {
        out += &format!(
            "  member {}: earns {income}, contributes {share}\n",
            number + 1
        );
    }
    Ok(out)
}

/// The one item called `name`, or whose identifier ends with `name` (at
/// least 4 characters). `what` names the kind of item, for errors.
// Generic over the item type `T`, like a function template. `'a` ties the
// returned reference to `items`: the result borrows from the slice.
fn pick<'a, T>(
    items: &'a [T],
    name: &str,
    what: &str,
    id: impl Fn(&T) -> String,
    label: impl Fn(&T) -> &str,
) -> Result<&'a T, String> {
    let lowercase = name.to_lowercase();
    let matches: Vec<&T> = items
        .iter()
        .filter(|item| {
            label(item).to_lowercase() == lowercase
                || (name.len() >= 4 && id(item).ends_with(&lowercase))
        })
        .collect();
    match matches.as_slice() {
        [one] => Ok(*one),
        [] => Err(format!("no {what} {name}")),
        _ => Err(format!(
            "several {what}s match {name}: use the end of its identifier"
        )),
    }
}

/// The members of budget `id`, each with role and name.
fn list_members(context: &mut Context, name: &str) -> Result<String, String> {
    let id = find_budget(context.store, name)?.id;
    let me = stored(context.store.member())?;
    let mut out = String::new();
    for member in stored(context.store.members(id))? {
        out += &format!(
            "{}  {:<20} {}{}\n",
            short(&member.id),
            member.name.as_deref().unwrap_or("(no name yet)"),
            member.role.as_str(),
            if member.id == me { ", you" } else { "" }
        );
    }
    Ok(out)
}

/// The member of budget `id` called `name`: "me", a display name, or the
/// end of their identifier.
fn find_member(context: &mut Context, id: BudgetId, name: &str) -> Result<MemberId, String> {
    if name == "me" {
        return Ok(MemberId(stored(context.store.member())?));
    }
    let members = stored(context.store.members(id))?;
    let member = pick(
        &members,
        name,
        "member",
        |member| member.id.clone(),
        |member| member.name.as_deref().unwrap_or_default(),
    )?;
    Ok(MemberId(member.id.clone()))
}

pub(crate) fn find_budget(store: &Store, name: &str) -> Result<Budget, String> {
    let budgets = stored(store.budgets())?;
    let (id, _) = pick(
        &budgets,
        name,
        "budget",
        |(id, _)| id.to_string(),
        |(_, name)| name,
    )?;
    stored(store.load(*id))?.ok_or_else(|| format!("budget {name} is gone"))
}

fn find_line<'a>(budget: &'a Budget, name: &str) -> Result<&'a Line, String> {
    pick(
        budget.lines(),
        name,
        "line",
        |line| line.id.to_string(),
        |line| &line.name,
    )
}

fn find_pot<'a>(budget: &'a Budget, name: &str) -> Result<&'a Pot, String> {
    pick(
        budget.pots(),
        name,
        "pot",
        |pot| pot.id.to_string(),
        |pot| &pot.name,
    )
}

fn find_transaction<'a>(budget: &'a Budget, name: &str) -> Result<&'a Transaction, String> {
    // Transactions have no name: only their identifier matches.
    pick(
        budget.transactions(),
        name,
        "transaction",
        |transaction| transaction.id.to_string(),
        |_| "",
    )
}

/// A storage error as text.
pub(crate) fn stored<T>(result: Result<T, StoreError>) -> Result<T, String> {
    result.map_err(|error| match error {
        StoreError::NotAllowed(reason) => reason,
        StoreError::NoKey => "this device has no key of this budget yet: sync first".to_string(),
        // `{:?}` uses the derived Debug formatting.
        other => format!("storage: {other:?}"),
    })
}

/// Why the budget refused, in words.
fn explain(error: BudgetError) -> String {
    // The core words it (`Display`); the CLI adds the option to use.
    match error {
        BudgetError::PotMismatch => {
            "a transaction on an allocation line goes --into that line's pot".to_string()
        }
        other => other.to_string(),
    }
}

pub fn parse_date(text: &str) -> Result<Date, String> {
    // `parse` uses Date's `FromStr`, which reads ISO 8601: 2026-11-16.
    text.parse()
        .map_err(|_| format!("not a date (2026-11-16): {text}"))
}

fn parse_month(text: &str) -> Result<Month, String> {
    text.split_once('-')
        .and_then(|(year, month)| Month::new(year.parse().ok()?, month.parse().ok()?))
        .ok_or_else(|| format!("not a month (2026-11): {text}"))
}

fn parse_kind(text: &str) -> Result<Kind, String> {
    match text {
        "income" => Ok(Kind::Income),
        "cost" => Ok(Kind::Cost),
        _ => Err(format!("income or cost, not {text}")),
    }
}

fn parse_amount(text: &str, currency: Currency) -> Result<Money, String> {
    Money::parse(text, currency).ok_or_else(|| format!("not an amount (12.34): {text}"))
}

fn parse_schedule(text: &str, first: Date) -> Result<Schedule, String> {
    let every = match text {
        "weekly" => return Ok(Schedule::Weekly { first }),
        "once" => return Ok(Schedule::Once { on: first }),
        "monthly" => Some(1),
        "quarterly" => Some(3),
        "yearly" => Some(12),
        _ => text
            .strip_prefix("every-")
            .and_then(|rest| rest.strip_suffix("-months"))
            .and_then(|count| count.parse().ok()),
    };
    // `NonZeroU8::new` refuses 0 ("every-0-months").
    every
        .and_then(NonZeroU8::new)
        .map(|every| Schedule::Months { every, first })
        .ok_or_else(|| format!("not a schedule: {text}"))
}

/// The last 8 hex digits of an identifier: they are random, so enough to
/// tell items apart, and `pick` accepts them in place of a name.
// `impl Display`: any type that `{}` can format, here the identifier types.
fn short(id: impl std::fmt::Display) -> String {
    let id = id.to_string();
    // A byte range of the string: safe here, the identifier is ASCII.
    id[id.len() - 8..].to_string()
}

/// An amount without its currency, for tables: `-12.34`.
fn amount(money: Money) -> String {
    let cents = money.minor_units();
    let sign = if cents < 0 { "-" } else { "" };
    let cents = cents.unsigned_abs();
    format!("{sign}{}.{:02}", cents / 100, cents % 100)
}

/// A comparison where nothing is planned nor spent.
fn zero(budget: &Budget) -> Comparison {
    Comparison {
        planned: Money::zero(budget.currency()),
        actual: Money::zero(budget.currency()),
    }
}

/// `text` followed by the cost's status, in its colour on a terminal:
/// green under the plan, yellow close to it, red over it.
fn judged(text: String, comparison: Comparison, colour: bool) -> String {
    let (word, code) = match comparison.status() {
        // Same width for every word, so the columns after it stay aligned.
        Status::Under => ("      ", 32),
        Status::Close => (" close", 33),
        Status::Over => ("  over", 31),
    };
    if colour {
        // ANSI escape codes: `ESC[32m` turns text green, `ESC[0m` resets.
        format!("\x1b[{code}m{text}{word}\x1b[0m")
    } else {
        format!("{text}{word}")
    }
}

/// A transaction on one line: date, kind, amount, category, pot, note.
fn describe(budget: &Budget, transaction: &Transaction) -> String {
    let pot_name = |id| budget.pot(id).map(|pot| pot.name.as_str()).unwrap_or("?");
    let pot = match transaction.pot {
        Some(PotMove::Into(id)) => format!(", into {}", pot_name(id)),
        Some(PotMove::PaidFrom(id)) => format!(", from {}", pot_name(id)),
        None => String::new(),
    };
    let note = if transaction.note.is_empty() {
        String::new()
    } else {
        format!(", {}", transaction.note)
    };
    format!(
        "{} {:<6}{:>11}  {}{pot}{note}",
        transaction.date,
        kind_text(transaction.kind),
        amount(transaction.amount),
        transaction.category
    )
}

fn kind_text(kind: Kind) -> &'static str {
    match kind {
        Kind::Income => "income",
        Kind::Cost => "cost",
    }
}

fn month_text(month: Month) -> String {
    let day = month.first_day();
    format!("{}-{:02}", day.year(), day.month())
}

fn schedule_text(schedule: &Schedule) -> String {
    match schedule {
        Schedule::Months { every, first } if every.get() == 1 => format!("monthly from {first}"),
        Schedule::Months { every, first } => format!("every {every} months from {first}"),
        Schedule::Weekly { first } => format!("weekly from {first}"),
        Schedule::Once { on } => format!("once on {on}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    /// Runs `command` (words separated by spaces) on `store`, on 16
    /// November 2026.
    fn bougette(store: &mut Store, command: &str) -> Result<String, String> {
        let arguments = command.split(' ').map(String::from).collect();
        let (words, options) = parse_arguments(arguments)?;
        let words: Vec<&str> = words.iter().map(String::as_str).collect();
        let mut context = Context {
            store,
            today: date(2026, 11, 16),
            colour: false,
        };
        run(&words, &options, &mut context)
    }

    /// Runs commands that must succeed.
    fn all(store: &mut Store, commands: &[&str]) {
        for command in commands {
            // `unwrap_or_else` with a closure that panics: the test fails
            // with the command and its error.
            bougette(store, command).unwrap_or_else(|error| panic!("{command}: {error}"));
        }
    }

    fn household() -> Store {
        let mut store = Store::open_in_memory().unwrap();
        all(
            &mut store,
            &[
                "budget new Household",
                "line add Household Pay Income income 2500 monthly 2026-01-01",
                "line add Household Rent Housing cost 900 monthly 2026-01-05",
                "line add Household Insurance Housing cost 480 yearly 2026-03-10",
                "pot add Household Bike 1200 2027-06-30",
                "line add Household Savings Savings cost 100 monthly 2026-01-01 --pot Bike",
                "tx add Household 2026-11-05 cost 950 Housing --line rent",
                "tx add Household 2026-11-02 cost 100 Savings --line Savings --into Bike",
                "tx add Household 2026-11-12 cost 30 Leisure --note cinema",
            ],
        );
        store
    }

    /// The words after the label of the first row starting with `label`.
    // The result borrows from `text`: both share the lifetime `'t`.
    fn row<'t>(text: &'t str, label: &str) -> Vec<&'t str> {
        let row = text
            .lines()
            .find(|row| row.trim_start().starts_with(label))
            .unwrap();
        // `split_whitespace` skips runs of spaces; `skip(2)` drops the
        // label, "Rent (Housing)".
        row.split_whitespace().skip(2).collect()
    }

    #[test]
    fn shows_a_month_with_its_overspending() {
        let mut store = household();
        let show = bougette(&mut store, "show Household 2026-11").unwrap();
        // The rent went over: €950 paid for €900 planned.
        assert_eq!(row(&show, "Rent"), ["900.00", "900.00", "950.00", "over"]);
        // The yearly insurance is smoothed, €40 a month, nothing due.
        assert_eq!(row(&show, "Insurance"), ["0.00", "40.00", "0.00"]);
        assert!(show.contains("Leisure, cinema"), "{show}");
        assert!(
            show.contains("Bike: 100.00, goal 1200.00 by 2027-06-30"),
            "{show}"
        );
    }

    #[test]
    fn lists_budgets_lines_and_transactions() {
        let mut store = household();
        let budgets = bougette(&mut store, "budgets").unwrap();
        // 2500 - 900 - 40 - 100.
        assert!(budgets.contains("1460.00"), "{budgets}");
        let lines = bougette(&mut store, "line list Household").unwrap();
        assert!(
            lines.contains("Savings (Savings), cost, into Bike"),
            "{lines}"
        );
        assert!(
            lines.contains("480.00 every 12 months from 2026-03-10"),
            "{lines}"
        );
        let transactions = bougette(&mut store, "tx list Household").unwrap();
        // Sorted by date: the allocation of the 2nd comes first.
        assert!(
            transactions.find("2026-11-02") < transactions.find("2026-11-05"),
            "{transactions}"
        );
        let year = bougette(&mut store, "year Household").unwrap();
        assert!(year.contains("2026-11"), "{year}");
        assert!(year.contains("forecast"), "{year}");
    }

    #[test]
    fn changes_and_removes() {
        let mut store = household();
        all(
            &mut store,
            &["line change Household Rent 950 monthly 2026-12-05"],
        );
        let december = bougette(&mut store, "show Household 2026-12").unwrap();
        assert!(december.contains("950.00"), "{december}");
        // The rent has a transaction: it can't go before it does.
        let refused = bougette(&mut store, "line remove Household Rent").unwrap_err();
        assert!(refused.contains("has transactions"), "{refused}");
        let listed = bougette(&mut store, "tx list Household 2026-11").unwrap();
        // The identifier of the rent payment, the first word of its row.
        let row = listed.lines().find(|row| row.contains("Rent")).unwrap();
        let id = row.split(' ').next().unwrap();
        all(
            &mut store,
            &[
                &format!("tx remove Household {id}"),
                "line remove Household Rent",
                "line end Household Insurance 2026-12-31",
            ],
        );
        let lines = bougette(&mut store, "line list Household").unwrap();
        assert!(!lines.contains("Rent"), "{lines}");
        assert!(lines.contains("until 2026-12-31"), "{lines}");
    }

    #[test]
    fn splits_a_budget_or_a_total() {
        let mut store = household();
        // The smoothed costs, €1,040, split 3:1.
        let split = bougette(&mut store, "split Household 3000 1000").unwrap();
        assert!(split.contains("contributes 780.00 EUR"), "{split}");
        assert!(split.contains("contributes 260.00 EUR"), "{split}");
        let equal = bougette(&mut store, "split 100 3000 1000 --rule equal").unwrap();
        assert!(equal.contains("contributes 50.00 EUR"), "{equal}");
    }

    #[test]
    fn explains_what_it_refuses() {
        let mut store = household();
        let error = |store: &mut Store, command| bougette(store, command).unwrap_err();
        assert!(
            error(
                &mut store,
                "tx add Household 2026-11-10 income 5 Pay --line Rent"
            )
            .contains("can't realise")
        );
        assert!(error(&mut store, "show Nowhere").contains("no budget Nowhere"));
        assert!(
            error(
                &mut store,
                "line add Household X Y cost 0 monthly 2026-01-01"
            )
            .contains("positive")
        );
        assert!(error(&mut store, "show Household --colour yes").contains("unknown option"));
        assert!(error(&mut store, "frobnicate").contains("unknown command"));
    }

    #[test]
    fn a_personal_budget_shows_what_was_paid_in_shared_ones() {
        let mut store = household();
        all(
            &mut store,
            &[
                "name Alice",
                "tx add Household 2026-11-03 income 1000 Contributions --payer me",
                "tx add Household 2026-11-08 cost 42 Food --payer Alice --note market",
                "budget new Alice",
                "budget personal Alice",
            ],
        );
        let members = bougette(&mut store, "members Household").unwrap();
        assert!(members.contains("Alice"), "{members}");
        assert!(members.contains("admin, you"), "{members}");
        let mine = bougette(&mut store, "show Alice 2026-11").unwrap();
        // Both show as costs of Alice's own budget, in its "Household"
        // category: 1,042 in all.
        assert!(mine.contains("Household, Contributions: "), "{mine}");
        assert!(mine.contains("Household, Food: market"), "{mine}");
        // The totals' row, not the "Costs" heading of the lines; `row`
        // skips two words, the label and the due amount (0) here.
        let totals = mine.split("Totals").nth(1).unwrap();
        assert_eq!(row(totals, "Costs"), ["0.00", "1042.00", "over", "1042.00"]);
        // An unknown member is refused.
        let error = bougette(
            &mut store,
            "tx add Household 2026-11-08 cost 1 Food --payer Bob",
        );
        assert!(error.unwrap_err().contains("no member Bob"));
    }
}
