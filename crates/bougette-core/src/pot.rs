//! Pots: money set aside inside a budget, optionally towards a goal.

use jiff::civil::Date;

use crate::id::PotId;
use crate::money::Money;

/// Money set aside inside a budget (GLOSSARY.md: Pot). Its balance comes
/// from the budget's transactions: see `Budget::pot_balance`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pot {
    pub id: PotId,
    pub name: String,
    pub goal: Option<Goal>,
}

/// What a pot saves towards: e.g. €1,200 by June.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Goal {
    pub amount: Money,
    pub by: Option<Date>,
}

impl Pot {
    /// A new pot with no goal.
    pub fn new(name: impl Into<String>) -> Pot {
        Pot {
            id: PotId::new(),
            name: name.into(),
            goal: None,
        }
    }
}
