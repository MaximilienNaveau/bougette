//! Exporting budgets (doc/scope.md: Export): the transactions as CSV, to
//! open in a spreadsheet, and everything as JSON, a backup that doesn't
//! depend on the app. Both written by hand: two small formats, no crate.

use jiff::civil::Date;

use crate::budget::Budget;
use crate::id::{MemberId, PotId};
use crate::line::{Kind, Line};
use crate::pot::Pot;
use crate::schedule::Schedule;
use crate::transaction::{PotMove, Transaction};

/// The two export files of `budgets` on `today`, as (name, contents):
/// everything as JSON, then the transactions as CSV. Writing them is the
/// caller's job (the core does no I/O).
pub fn files(budgets: &[Budget], today: Date) -> [(String, String); 2] {
    [
        (format!("bougette-{today}.json"), json(budgets, today)),
        (
            format!("bougette-{today}-operations.csv"),
            transactions_csv(budgets),
        ),
    ]
}

/// The transactions of `budgets`, one per row, by budget then date:
/// `budget,date,kind,amount,currency,category,line,pot,note`. The amount
/// is signed (an income positive, a cost negative), with a decimal point,
/// so a spreadsheet can add it up; the line and pot are named.
pub fn transactions_csv(budgets: &[Budget]) -> String {
    let mut out = String::from("budget,date,kind,amount,currency,category,line,pot,note\n");
    for budget in budgets {
        // Kept in a variable: `code()` borrows from it, and a temporary
        // would be gone before the row using that borrow (the compiler
        // refuses what C++ would let dangle).
        let currency = budget.currency();
        // A sorted copy of references: `sort_by_key` on a Vec<&T>, leaving
        // the budget untouched (like sorting a vector of pointers).
        let mut transactions: Vec<&Transaction> = budget.transactions().iter().collect();
        transactions.sort_by_key(|transaction| transaction.date);
        for transaction in transactions {
            let signed = match transaction.kind {
                Kind::Income => transaction.amount.minor_units(),
                Kind::Cost => -transaction.amount.minor_units(),
            };
            // `and_then`: the line's name if the transaction has a line and
            // the budget knows it; `map_or` turns None into "".
            let line = transaction
                .line
                .and_then(|id| budget.line(id))
                .map_or("", |line| line.name.as_str());
            let pot = pot_of(transaction.pot)
                .and_then(|(id, _)| budget.pot(id))
                .map_or("", |pot| pot.name.as_str());
            let row = [
                budget.name.as_str(),
                &transaction.date.to_string(),
                kind(transaction.kind),
                &decimal(signed),
                currency.code(),
                &transaction.category,
                line,
                pot,
                &transaction.note,
            ];
            // `map` on an array gives an array of the escaped fields, then
            // joined: like Python's `",".join(map(csv_field, row))`.
            out.push_str(&row.map(csv_field).join(","));
            out.push('\n');
        }
    }
    out
}

/// Everything in `budgets`, exported on `exported_on`: their pots, their
/// lines with every amount over time, and their transactions. Amounts are
/// in cents, as stored, so nothing is rounded.
pub fn json(budgets: &[Budget], exported_on: Date) -> String {
    let document = Json::Object(vec![
        ("format", Json::text("bougette")),
        ("version", Json::Int(1)),
        ("exported_on", Json::date(exported_on)),
        (
            "budgets",
            Json::Array(budgets.iter().map(budget_json).collect()),
        ),
    ]);
    let mut out = String::new();
    document.write(&mut out, 0);
    out.push('\n');
    out
}

fn budget_json(budget: &Budget) -> Json {
    Json::Object(vec![
        ("id", Json::text(budget.id.to_string())),
        ("name", Json::text(&budget.name)),
        ("currency", Json::text(budget.currency().code())),
        ("personal", Json::Bool(budget.personal)),
        (
            "pots",
            Json::Array(budget.pots().iter().map(pot_json).collect()),
        ),
        (
            "lines",
            Json::Array(budget.lines().iter().map(line_json).collect()),
        ),
        (
            "transactions",
            Json::Array(budget.transactions().iter().map(transaction_json).collect()),
        ),
    ])
}

fn pot_json(pot: &Pot) -> Json {
    Json::Object(vec![
        ("id", Json::text(pot.id.to_string())),
        ("name", Json::text(&pot.name)),
        (
            "goal",
            Json::option(pot.goal.map(|goal| {
                Json::Object(vec![
                    ("amount", Json::Int(goal.amount.minor_units())),
                    ("by", Json::option(goal.by.map(Json::date))),
                ])
            })),
        ),
    ])
}

fn line_json(line: &Line) -> Json {
    let revisions = line
        .revisions()
        .iter()
        .map(|revision| {
            Json::Object(vec![
                ("from", Json::date(revision.from)),
                ("amount", Json::Int(revision.amount.minor_units())),
                ("schedule", schedule_json(revision.schedule)),
            ])
        })
        .collect();
    Json::Object(vec![
        ("id", Json::text(line.id.to_string())),
        ("name", Json::text(&line.name)),
        ("category", Json::text(&line.category)),
        ("kind", Json::text(kind(line.kind))),
        (
            "pot",
            Json::option(line.pot.map(|id| Json::text(id.to_string()))),
        ),
        (
            "payer",
            Json::option(line.payer.as_ref().map(|id| Json::text(id.to_string()))),
        ),
        ("ends_on", Json::option(line.ends_on().map(Json::date))),
        ("revisions", Json::Array(revisions)),
    ])
}

fn schedule_json(schedule: Schedule) -> Json {
    match schedule {
        Schedule::Months { every, first } => Json::Object(vec![
            ("repeat", Json::text("months")),
            ("every", Json::Int(i64::from(every.get()))),
            ("first", Json::date(first)),
        ]),
        Schedule::Weekly { first } => Json::Object(vec![
            ("repeat", Json::text("weekly")),
            ("first", Json::date(first)),
        ]),
        Schedule::Once { on } => {
            Json::Object(vec![("repeat", Json::text("once")), ("on", Json::date(on))])
        }
    }
}

fn transaction_json(transaction: &Transaction) -> Json {
    let pot = pot_of(transaction.pot);
    Json::Object(vec![
        ("id", Json::text(transaction.id.to_string())),
        ("date", Json::date(transaction.date)),
        ("kind", Json::text(kind(transaction.kind))),
        ("amount", Json::Int(transaction.amount.minor_units())),
        ("category", Json::text(&transaction.category)),
        (
            "line",
            Json::option(transaction.line.map(|id| Json::text(id.to_string()))),
        ),
        (
            "pot",
            Json::option(pot.map(|(id, _)| Json::text(id.to_string()))),
        ),
        (
            "pot_move",
            Json::option(
                pot.map(|(_, paid_from)| Json::text(if paid_from { "paid_from" } else { "into" })),
            ),
        ),
        ("payer", member(&transaction.payer)),
        ("payee", member(&transaction.payee)),
        ("note", Json::text(&transaction.note)),
    ])
}

/// A member's identifier, or `null`.
fn member(id: &Option<MemberId>) -> Json {
    Json::option(id.as_ref().map(|id| Json::text(id.to_string())))
}

/// The pot a transaction moves money for, and whether it's paid from it.
fn pot_of(pot: Option<PotMove>) -> Option<(PotId, bool)> {
    match pot? {
        PotMove::Into(id) => Some((id, false)),
        PotMove::PaidFrom(id) => Some((id, true)),
    }
}

fn kind(kind: Kind) -> &'static str {
    match kind {
        Kind::Income => "income",
        Kind::Cost => "cost",
    }
}

/// `-123450` cents as `-1234.50`: a decimal point, always two decimals.
fn decimal(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let cents = cents.unsigned_abs();
    format!("{sign}{}.{:02}", cents / 100, cents % 100)
}

/// A CSV field (RFC 4180): quoted when it holds a comma, a quote or a line
/// break, its quotes doubled.
fn csv_field(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// A JSON value, written indented by two spaces. Object keys keep the
/// order given, so the output reads in a fixed order.
enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Text(String),
    Array(Vec<Json>),
    // `&'static str`: keys are string literals, which live for the whole
    // program (`'static`), so no copy is needed.
    Object(Vec<(&'static str, Json)>),
}

impl Json {
    fn text(text: impl Into<String>) -> Json {
        Json::Text(text.into())
    }

    fn date(date: Date) -> Json {
        Json::Text(date.to_string())
    }

    /// The value, or `null`.
    fn option(value: Option<Json>) -> Json {
        value.unwrap_or(Json::Null)
    }

    /// Appends the value to `out`, its inner lines indented `depth` levels.
    fn write(&self, out: &mut String, depth: usize) {
        let indent = |depth: usize| "  ".repeat(depth);
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
            Json::Int(value) => out.push_str(&value.to_string()),
            Json::Text(text) => write_text(out, text),
            // An empty list or object on one line: `[]`, `{}`.
            Json::Array(items) if items.is_empty() => out.push_str("[]"),
            Json::Object(fields) if fields.is_empty() => out.push_str("{}"),
            Json::Array(items) => {
                out.push_str("[\n");
                // `enumerate`: (position, item), to put commas between.
                for (index, item) in items.iter().enumerate() {
                    out.push_str(&indent(depth + 1));
                    item.write(out, depth + 1);
                    out.push_str(if index + 1 < items.len() { ",\n" } else { "\n" });
                }
                out.push_str(&indent(depth));
                out.push(']');
            }
            Json::Object(fields) => {
                out.push_str("{\n");
                for (index, (key, value)) in fields.iter().enumerate() {
                    out.push_str(&indent(depth + 1));
                    write_text(out, key);
                    out.push_str(": ");
                    value.write(out, depth + 1);
                    out.push_str(if index + 1 < fields.len() {
                        ",\n"
                    } else {
                        "\n"
                    });
                }
                out.push_str(&indent(depth));
                out.push('}');
            }
        }
    }
}

/// A JSON string: quoted, with quotes, backslashes and control characters
/// escaped (RFC 8259); everything else, accents included, as is (UTF-8).
fn write_text(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // Other control characters as `\u00XX`: `{:04x}` is
            // hexadecimal padded to 4 digits, like printf's "%04x".
            c if c < ' ' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::money::{Currency, Money};
    use crate::pot::Goal;
    use jiff::civil::date;
    use std::num::NonZeroU8;

    fn eur(cents: i64) -> Money {
        Money::new(cents, Currency::EUR)
    }

    fn monthly(first: Date) -> Schedule {
        Schedule::Months {
            every: NonZeroU8::new(1).unwrap(),
            first,
        }
    }

    /// A small flat: rent in, a kitchen pot fed by a line, two
    /// transactions, one of them with a note that needs escaping.
    fn flat() -> Budget {
        let mut budget = Budget::new("Flat, rue Basse", Currency::EUR);
        let mut pot = Pot::new("Kitchen");
        pot.goal = Some(Goal {
            amount: eur(200000),
            by: None,
        });
        let kitchen = budget.add_pot(pot);
        let mut rent = Line::new(
            "Rent",
            "Rent",
            Kind::Income,
            date(2026, 1, 5),
            eur(50000),
            monthly(date(2026, 1, 5)),
        )
        .unwrap();
        rent.change_from(date(2026, 7, 5), eur(51000), monthly(date(2026, 1, 5)))
            .unwrap();
        rent.end_on(Some(date(2041, 3, 10))).unwrap();
        let rent = budget.add_line(rent).unwrap();
        let mut savings =
            Transaction::new(date(2026, 2, 1), Kind::Cost, eur(10000), "Savings", None);
        savings.pot = Some(PotMove::Into(kitchen));
        savings.note = "for the \"new\" kitchen,\nphase 1".into();
        budget.add_transaction(savings).unwrap();
        budget
            .add_transaction(Transaction::new(
                date(2026, 1, 6),
                Kind::Income,
                eur(50000),
                "Rent",
                Some(rent),
            ))
            .unwrap();
        budget
    }

    #[test]
    fn csv_has_one_signed_row_per_transaction_by_date() {
        assert_eq!(
            transactions_csv(&[flat()]),
            "budget,date,kind,amount,currency,category,line,pot,note\n\
             \"Flat, rue Basse\",2026-01-06,income,500.00,EUR,Rent,Rent,,\n\
             \"Flat, rue Basse\",2026-02-01,cost,-100.00,EUR,Savings,,Kitchen,\
             \"for the \"\"new\"\" kitchen,\nphase 1\"\n"
        );
    }

    #[test]
    fn decimals_keep_their_cents_and_sign() {
        assert_eq!(decimal(0), "0.00");
        assert_eq!(decimal(5), "0.05");
        assert_eq!(decimal(-123450), "-1234.50");
    }

    #[test]
    fn json_holds_everything_in_cents() {
        let budget = flat();
        // `from_ref`: a one-element slice borrowing `budget`, no copy.
        let out = json(std::slice::from_ref(&budget), date(2026, 11, 20));
        let pot = budget.pots()[0].id;
        let line = budget.lines()[0].id;
        let first = budget.transactions()[0].id;
        let second = budget.transactions()[1].id;
        let expected = format!(
            r#"{{
  "format": "bougette",
  "version": 1,
  "exported_on": "2026-11-20",
  "budgets": [
    {{
      "id": "{budget}",
      "name": "Flat, rue Basse",
      "currency": "EUR",
      "personal": false,
      "pots": [
        {{
          "id": "{pot}",
          "name": "Kitchen",
          "goal": {{
            "amount": 200000,
            "by": null
          }}
        }}
      ],
      "lines": [
        {{
          "id": "{line}",
          "name": "Rent",
          "category": "Rent",
          "kind": "income",
          "pot": null,
          "payer": null,
          "ends_on": "2041-03-10",
          "revisions": [
            {{
              "from": "2026-01-05",
              "amount": 50000,
              "schedule": {{
                "repeat": "months",
                "every": 1,
                "first": "2026-01-05"
              }}
            }},
            {{
              "from": "2026-07-05",
              "amount": 51000,
              "schedule": {{
                "repeat": "months",
                "every": 1,
                "first": "2026-01-05"
              }}
            }}
          ]
        }}
      ],
      "transactions": [
        {{
          "id": "{first}",
          "date": "2026-02-01",
          "kind": "cost",
          "amount": 10000,
          "category": "Savings",
          "line": null,
          "pot": "{pot}",
          "pot_move": "into",
          "payer": null,
          "payee": null,
          "note": "for the \"new\" kitchen,\nphase 1"
        }},
        {{
          "id": "{second}",
          "date": "2026-01-06",
          "kind": "income",
          "amount": 50000,
          "category": "Rent",
          "line": "{line}",
          "pot": null,
          "pot_move": null,
          "payer": null,
          "payee": null,
          "note": ""
        }}
      ]
    }}
  ]
}}
"#,
            budget = budget.id,
        );
        assert_eq!(out, expected);
    }

    #[test]
    fn json_text_escapes_controls_but_not_accents() {
        let mut out = String::new();
        write_text(&mut out, "Opération\t\\ \u{1}");
        assert_eq!(out, r#""Opération\t\\ \u0001""#);
    }
}
