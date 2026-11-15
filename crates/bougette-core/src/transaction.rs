//! Transactions: money that actually came in or went out.

use jiff::civil::Date;

use crate::id::{LineId, PotId, TransactionId};
use crate::line::Kind;
use crate::money::Money;

/// One amount that actually came in or went out on a given date
/// (GLOSSARY.md: Transaction).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub id: TransactionId,
    pub date: Date,
    pub kind: Kind,
    /// Positive in the direction of `kind`. A negative cost is a refund:
    /// it lowers what was spent (doc/scope.md).
    pub amount: Money,
    /// Required: every transaction belongs to a category.
    pub category: String,
    /// Optional: the line this transaction realises, if it was planned.
    pub line: Option<LineId>,
    /// Optional: the pot this transaction puts money into or is paid from.
    pub pot: Option<PotMove>,
    /// Free text, e.g. "Engie, November".
    pub note: String,
}

/// How a transaction moves money for a pot. Both are costs of the budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PotMove {
    /// An allocation: the money leaves the month's budget for the pot
    /// (GLOSSARY.md: Allocation). It counts as a cost of the month.
    Into(PotId),
    /// Spending paid from the pot, e.g. buying the bike: it lowers the
    /// pot's balance and does not count against the month's plan.
    PaidFrom(PotId),
}

impl Transaction {
    /// A new transaction with a fresh identifier, no pot and no note.
    pub fn new(
        date: Date,
        kind: Kind,
        amount: Money,
        category: impl Into<String>,
        line: Option<LineId>,
    ) -> Transaction {
        Transaction {
            id: TransactionId::new(),
            date,
            kind,
            amount,
            category: category.into(),
            line,
            pot: None,
            // `String::new()` is an empty string.
            note: String::new(),
        }
    }
}
