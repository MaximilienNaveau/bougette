//! Turning a budget into fields, and fields back into a budget.
//!
//! A budget is flattened into *entities* (the budget itself, its pots,
//! lines, revisions and transactions), each with an identifier and named
//! fields holding text. Each field is the unit of change: two devices
//! editing different fields of the same line never conflict, and for the
//! same field the latest change wins (doc/adr/0001).

use std::collections::{BTreeMap, HashMap};
use std::num::NonZeroU8;

use bougette_core::budget::Budget;
use bougette_core::id::{BudgetId, LineId, MemberId, PotId, TransactionId};
use bougette_core::line::{Kind, Line};
use bougette_core::money::{Currency, Money};
use bougette_core::pot::{Goal, Pot};
use bougette_core::schedule::Schedule;
use bougette_core::transaction::{PotMove, Transaction};
use jiff::civil::Date;
use uuid::Uuid;

/// Which field of which entity: (entity kind, entity identifier, field).
pub type Key = (String, String, String);

/// Every field of a budget, sorted by key. `None` is an empty field (no
/// end date, no pot…), which is different from a missing one.
// `BTreeMap` is `std::map` (sorted); `HashMap` is `std::unordered_map`.
pub type Records = BTreeMap<Key, Option<String>>;

/// The field that marks a deleted entity.
pub const DELETED: &str = "deleted";

/// Why stored fields couldn't be turned back into a budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Corrupt(pub String);

/// Flattens `budget` into its fields.
pub fn encode(budget: &Budget) -> Records {
    let mut records = Records::new();
    let currency = budget.currency();
    // A closure that adds every field of one entity. `&mut records` is
    // captured mutably; the borrow checker lets nothing else touch
    // `records` while `add` exists.
    let mut add = |entity: &str, id: String, fields: Vec<(&str, Option<String>)>| {
        // Every entity also has a `deleted` field, empty while it lives,
        // so restoring an entity is a change like any other.
        for (field, value) in fields.into_iter().chain([(DELETED, None)]) {
            records.insert((entity.to_string(), id.clone(), field.to_string()), value);
        }
    };

    add(
        "budget",
        budget.id.to_string(),
        vec![
            ("name", Some(budget.name.clone())),
            ("currency", Some(currency.code().to_string())),
            // `then` gives `Some(...)` if the bool is true, else `None`.
            ("personal", budget.personal.then(|| "true".to_string())),
        ],
    );
    for pot in budget.pots() {
        add(
            "pot",
            pot.id.to_string(),
            vec![
                ("name", Some(pot.name.clone())),
                ("goal", pot.goal.map(encode_goal)),
            ],
        );
    }
    for line in budget.lines() {
        add(
            "line",
            line.id.to_string(),
            vec![
                ("name", Some(line.name.clone())),
                ("category", Some(line.category.clone())),
                ("kind", Some(encode_kind(line.kind))),
                ("pot", line.pot.map(|pot| pot.to_string())),
                ("payer", line.payer.as_ref().map(|member| member.0.clone())),
                ("ends_on", line.ends_on().map(|date| date.to_string())),
            ],
        );
        for revision in line.revisions() {
            // A line has at most one revision per date (`change_from`
            // replaces it), so line and date identify a revision. Two
            // devices revising the same line from the same date then edit
            // the same fields, and the latest wins.
            add(
                "revision",
                format!("{}/{}", line.id, revision.from),
                vec![
                    ("line", Some(line.id.to_string())),
                    ("from", Some(revision.from.to_string())),
                    ("amount", Some(revision.amount.minor_units().to_string())),
                    ("schedule", Some(encode_schedule(revision.schedule))),
                ],
            );
        }
    }
    for transaction in budget.transactions() {
        add(
            "transaction",
            transaction.id.to_string(),
            vec![
                ("date", Some(transaction.date.to_string())),
                ("kind", Some(encode_kind(transaction.kind))),
                ("amount", Some(transaction.amount.minor_units().to_string())),
                ("category", Some(transaction.category.clone())),
                ("line", transaction.line.map(|line| line.to_string())),
                ("pot", transaction.pot.map(encode_pot_move)),
                ("note", Some(transaction.note.clone())),
                (
                    "payer",
                    transaction.payer.as_ref().map(|member| member.0.clone()),
                ),
                (
                    "payee",
                    transaction.payee.as_ref().map(|member| member.0.clone()),
                ),
            ],
        );
    }
    records
}

/// One entity's fields, by name.
type Fields = HashMap<String, Option<String>>;

/// Rebuilds a budget from its fields, skipping deleted entities. `None` if
/// there is no budget entity or it was deleted.
pub fn decode(records: &Records) -> Result<Option<Budget>, Corrupt> {
    let mut entities = group(records);
    // Drop the deleted entities. `retain` on a map keeps the entries for
    // which the closure returns true.
    for by_id in entities.values_mut() {
        by_id.retain(|_, fields| !matches!(fields.get(DELETED), Some(Some(_))));
    }
    // An empty map for the kinds with no entity at all.
    let empty = BTreeMap::new();
    let of_kind = |kind: &str| entities.get(kind).unwrap_or(&empty);

    // The budget itself: exactly one. `next()` takes the first entry.
    let Some((id, fields)) = of_kind("budget").iter().next() else {
        return Ok(None);
    };
    let code = text(fields, "currency")?;
    let currency = Currency::from_code(code).ok_or_else(|| corrupt("currency", code))?;
    let mut budget = Budget::new(text(fields, "name")?, currency);
    budget.id = BudgetId(parse_uuid(id)?);
    budget.personal = optional(fields, "personal").is_some();

    for (id, fields) in of_kind("pot") {
        let mut pot = Pot::new(text(fields, "name")?);
        pot.id = PotId(parse_uuid(id)?);
        // `transpose` swaps Option<Result<..>> into Result<Option<..>>, so
        // `?` can check the parse of a goal that may be absent.
        pot.goal = optional(fields, "goal")
            .map(|goal| decode_goal(goal, currency))
            .transpose()?;
        budget.add_pot(pot);
    }

    // The revisions of each line, sorted by date (`BTreeMap` keeps keys in
    // order, and dates order like their text).
    let mut revisions: HashMap<&str, BTreeMap<Date, &Fields>> = HashMap::new();
    for fields in of_kind("revision").values() {
        let from = parse_date(text(fields, "from")?)?;
        revisions
            .entry(text(fields, "line")?)
            .or_default()
            .insert(from, fields);
    }
    for (id, fields) in of_kind("line") {
        let line = decode_line(id, fields, revisions.get(id), currency)?;
        budget
            .add_line(line)
            .map_err(|error| Corrupt(format!("line {id}: {error:?}")))?;
    }

    for (id, fields) in of_kind("transaction") {
        budget
            .add_transaction(decode_transaction(id, fields, currency)?)
            .map_err(|error| Corrupt(format!("transaction {id}: {error:?}")))?;
    }
    Ok(Some(budget))
}

/// A deleted line or transaction, rebuilt as it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trashed {
    Line(Line),
    Transaction(Transaction),
}

/// The deleted lines and transactions in `records`, each with the
/// timestamp of its deletion mark (`deleted_at`: kind and identifier →
/// timestamp, for the deleted entities). A line comes back with the
/// revisions it had when it was deleted: those deleted with it, or
/// after; not those replaced before.
pub fn trashed(
    records: &Records,
    deleted_at: &HashMap<(String, String), String>,
) -> Result<Vec<(String, Trashed)>, Corrupt> {
    let entities = group(records);
    let empty = BTreeMap::new();
    let of_kind = |kind: &str| entities.get(kind).unwrap_or(&empty);
    let Some(budget) = of_kind("budget").values().next() else {
        return Ok(Vec::new());
    };
    let code = text(budget, "currency")?;
    let currency = Currency::from_code(code).ok_or_else(|| corrupt("currency", code))?;
    // When `kind`/`id` was deleted, if it was.
    let deleted = |kind: &str, id: &str| deleted_at.get(&(kind.to_string(), id.to_string()));

    let mut out = Vec::new();
    for (id, fields) in of_kind("transaction") {
        if let Some(when) = deleted("transaction", id) {
            let transaction = decode_transaction(id, fields, currency)?;
            out.push((when.clone(), Trashed::Transaction(transaction)));
        }
    }
    for (id, fields) in of_kind("line") {
        let Some(when) = deleted("line", id) else {
            continue;
        };
        // Its revisions as they were when the line was deleted. Timestamps
        // compare as text (see clock.rs).
        let mut revisions = BTreeMap::new();
        for (revision, revision_fields) in of_kind("revision") {
            let current = match deleted("revision", revision) {
                None => true,
                Some(gone) => gone >= when,
            };
            if current && text(revision_fields, "line")? == *id {
                let from = parse_date(text(revision_fields, "from")?)?;
                revisions.insert(from, revision_fields);
            }
        }
        let line = decode_line(id, fields, Some(&revisions), currency)?;
        out.push((when.clone(), Trashed::Line(line)));
    }
    // The most recent first: timestamps sort as text.
    out.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(out)
}

/// The fields of `records` by entity: kind → identifier → fields.
fn group(records: &Records) -> HashMap<&str, BTreeMap<&str, Fields>> {
    let mut entities: HashMap<&str, BTreeMap<&str, Fields>> = HashMap::new();
    for ((entity, id, field), value) in records {
        entities
            .entry(entity.as_str()) // like `map[key]`, inserting if absent
            .or_default()
            .entry(id.as_str())
            .or_default()
            .insert(field.clone(), value.clone());
    }
    entities
}

/// Line `id` from its fields and its revisions, by date.
fn decode_line(
    id: &str,
    fields: &Fields,
    revisions: Option<&BTreeMap<Date, &Fields>>,
    currency: Currency,
) -> Result<Line, Corrupt> {
    // A line whose revisions were all deleted can't be rebuilt.
    let mut line_revisions = revisions.into_iter().flatten();
    let Some((from, first)) = line_revisions.next() else {
        return Err(Corrupt(format!("line {id} has no revision")));
    };
    let mut line = Line::new(
        text(fields, "name")?,
        text(fields, "category")?,
        decode_kind(text(fields, "kind")?)?,
        *from,
        amount(first, currency)?,
        decode_schedule(text(first, "schedule")?)?,
    )
    .map_err(|error| Corrupt(format!("line {id}: {error:?}")))?;
    for (from, revision) in line_revisions {
        line.change_from(
            *from,
            amount(revision, currency)?,
            decode_schedule(text(revision, "schedule")?)?,
        )
        .map_err(|error| Corrupt(format!("line {id}: {error:?}")))?;
    }
    let ends_on = optional(fields, "ends_on").map(parse_date).transpose()?;
    line.end_on(ends_on)
        .map_err(|error| Corrupt(format!("line {id}: {error:?}")))?;
    line.id = LineId(parse_uuid(id)?);
    line.pot = optional(fields, "pot")
        .map(|pot| parse_uuid(pot).map(PotId))
        .transpose()?;
    // Absent from lines written before lines had a payer: `None`.
    line.payer = optional(fields, "payer").map(|member| MemberId(member.to_string()));
    Ok(line)
}

/// Transaction `id` from its fields.
fn decode_transaction(
    id: &str,
    fields: &Fields,
    currency: Currency,
) -> Result<Transaction, Corrupt> {
    Ok(Transaction {
        id: TransactionId(parse_uuid(id)?),
        date: parse_date(text(fields, "date")?)?,
        kind: decode_kind(text(fields, "kind")?)?,
        amount: amount(fields, currency)?,
        category: text(fields, "category")?.to_string(),
        line: optional(fields, "line")
            .map(|line| parse_uuid(line).map(LineId))
            .transpose()?,
        pot: optional(fields, "pot").map(decode_pot_move).transpose()?,
        note: text(fields, "note")?.to_string(),
        payer: optional(fields, "payer").map(|member| MemberId(member.to_string())),
        payee: optional(fields, "payee").map(|member| MemberId(member.to_string())),
    })
}

// The helpers below read one field, and encode or decode one value.

fn corrupt(field: &str, value: &str) -> Corrupt {
    Corrupt(format!("invalid {field}: {value:?}"))
}

/// A required field's text.
// The returned `&str` borrows from `fields`. With two references coming
// in, the compiler can't guess which one the result borrows from, so the
// *lifetime* `'a` spells it out: the result lives as long as `fields`.
fn text<'a>(fields: &'a Fields, field: &str) -> Result<&'a str, Corrupt> {
    optional(fields, field).ok_or_else(|| Corrupt(format!("missing {field}")))
}

/// An optional field's text, `None` if empty or missing.
fn optional<'a>(fields: &'a Fields, field: &str) -> Option<&'a str> {
    // `get` gives `Option<&Option<String>>`: missing, or present and maybe
    // empty. `and_then` unwraps the outer level, and `as_deref` borrows the
    // inner String as a &str.
    fields.get(field).and_then(|value| value.as_deref())
}

fn amount(fields: &Fields, currency: Currency) -> Result<Money, Corrupt> {
    let value = text(fields, "amount")?;
    // `str::parse` converts text to any type that implements `FromStr`,
    // chosen here by the annotation `i64`.
    let cents: i64 = value.parse().map_err(|_| corrupt("amount", value))?;
    Ok(Money::new(cents, currency))
}

fn parse_uuid(value: &str) -> Result<Uuid, Corrupt> {
    Uuid::parse_str(value).map_err(|_| corrupt("identifier", value))
}

fn parse_date(value: &str) -> Result<Date, Corrupt> {
    value.parse().map_err(|_| corrupt("date", value))
}

fn encode_kind(kind: Kind) -> String {
    match kind {
        Kind::Income => "income",
        Kind::Cost => "cost",
    }
    .to_string()
}

fn decode_kind(value: &str) -> Result<Kind, Corrupt> {
    match value {
        "income" => Ok(Kind::Income),
        "cost" => Ok(Kind::Cost),
        _ => Err(corrupt("kind", value)),
    }
}

/// A schedule in one field, so a change replaces it whole: `months/2/
/// 2026-01-05`, `weekly/2026-11-02` or `once/2026-06-15`.
fn encode_schedule(schedule: Schedule) -> String {
    match schedule {
        Schedule::Months { every, first } => format!("months/{every}/{first}"),
        Schedule::Weekly { first } => format!("weekly/{first}"),
        Schedule::Once { on } => format!("once/{on}"),
    }
}

fn decode_schedule(value: &str) -> Result<Schedule, Corrupt> {
    // Split into at most three parts, then match on the parts as a slice:
    // `[a, b]` matches a slice of exactly two elements.
    let parts: Vec<&str> = value.splitn(3, '/').collect();
    match parts.as_slice() {
        ["months", every, first] => Ok(Schedule::Months {
            every: every
                .parse::<NonZeroU8>()
                .map_err(|_| corrupt("schedule", value))?,
            first: parse_date(first)?,
        }),
        ["weekly", first] => Ok(Schedule::Weekly {
            first: parse_date(first)?,
        }),
        ["once", on] => Ok(Schedule::Once {
            on: parse_date(on)?,
        }),
        _ => Err(corrupt("schedule", value)),
    }
}

/// `into/<pot>` or `paid_from/<pot>`.
fn encode_pot_move(pot_move: PotMove) -> String {
    match pot_move {
        PotMove::Into(pot) => format!("into/{pot}"),
        PotMove::PaidFrom(pot) => format!("paid_from/{pot}"),
    }
}

fn decode_pot_move(value: &str) -> Result<PotMove, Corrupt> {
    match value.split_once('/') {
        Some(("into", pot)) => Ok(PotMove::Into(PotId(parse_uuid(pot)?))),
        Some(("paid_from", pot)) => Ok(PotMove::PaidFrom(PotId(parse_uuid(pot)?))),
        _ => Err(corrupt("pot", value)),
    }
}

/// `120000` (an amount in cents), or `120000/2026-06-30` with a date.
fn encode_goal(goal: Goal) -> String {
    let cents = goal.amount.minor_units();
    match goal.by {
        Some(by) => format!("{cents}/{by}"),
        None => cents.to_string(),
    }
}

fn decode_goal(value: &str, currency: Currency) -> Result<Goal, Corrupt> {
    // `split_once` returns None when there is no '/': then it's all amount.
    let (cents, by) = match value.split_once('/') {
        Some((cents, by)) => (cents, Some(parse_date(by)?)),
        None => (value, None),
    };
    let cents: i64 = cents.parse().map_err(|_| corrupt("goal", value))?;
    Ok(Goal {
        amount: Money::new(cents, currency),
        by,
    })
}
