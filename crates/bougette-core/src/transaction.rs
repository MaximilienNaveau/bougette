//! Transactions: money that actually came in or went out.

use jiff::civil::Date;

use crate::id::{LineId, TransactionId};
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
    /// Free text, e.g. "Engie, November".
    pub note: String,
}

impl Transaction {
    /// A new transaction with a fresh identifier and no note.
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
            // `String::new()` is an empty string.
            note: String::new(),
        }
    }
}
