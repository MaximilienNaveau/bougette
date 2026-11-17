//! Other budgets as seen from a member's personal budget: "entered once,
//! visible in two budgets" (doc/scope.md). A contribution or an advance,
//! planned or actual, is entered in the other budget only, and shows as a
//! cost in the personal budget of the member who pays.

use crate::budget::Budget;
use crate::id::MemberId;
use crate::line::{Kind, Line};
use crate::transaction::Transaction;

/// What a member's personal budget shows of another budget (see
/// `Budget::with_reflections`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
// `Default` lets `Reflections::default()` build an empty one: both Vecs
// empty, like a C++ default constructor generated for you.
pub struct Reflections {
    pub lines: Vec<Line>,
    pub transactions: Vec<Transaction>,
}

/// What `member` pays or receives through `other`'s lines and
/// transactions, as lines and transactions of their personal budget, in
/// the category named after `other`:
/// - a contribution (an income the member pays) and an advance (a cost the
///   member pays) are costs for the member, planned or actual;
/// - a cost paid to the member (e.g. paying back their advances) is an
///   income for the member.
///
/// Each keeps its identifier, so it can be traced back to `other`, and a
/// transaction on a line the member pays realises that line's reflection.
pub fn reflections(other: &Budget, member: &MemberId) -> Reflections {
    let mut reflected = Reflections::default();
    for line in other.lines() {
        if line.payer.as_ref() != Some(member) {
            continue;
        }
        // A copy, turned into a cost of the personal budget. The
        // revisions (amounts, schedule) and the end date stay as they are.
        let mut mirror = line.clone();
        mirror.kind = Kind::Cost;
        mirror.category = other.name.clone();
        mirror.pot = None;
        mirror.payer = None;
        reflected.lines.push(mirror);
    }
    reflected.transactions = transactions(other, member);
    reflected
}

/// The transactions part of `reflections`.
fn transactions(shared: &Budget, member: &MemberId) -> Vec<Transaction> {
    // Whether line `id` of `shared` is one the member pays (so reflected).
    let pays_line = |id| {
        shared
            .line(id)
            .is_some_and(|line| line.payer.as_ref() == Some(member))
    };
    let mut reflected = Vec::new();
    for transaction in shared.transactions() {
        let kind = if transaction.payer.as_ref() == Some(member) {
            Kind::Cost
        } else if transaction.payee.as_ref() == Some(member) {
            Kind::Income
        } else {
            continue;
        };
        // Its line, if that line is reflected too (and as a cost, like
        // this transaction: only a payer's lines are).
        let line = transaction
            .line
            .filter(|&id| kind == Kind::Cost && pays_line(id));
        let mut mirror = Transaction::new(
            transaction.date,
            kind,
            transaction.amount,
            shared.name.clone(),
            line,
        );
        // The same identifier: it tells where the reflection comes from.
        mirror.id = transaction.id;
        mirror.note = format!("{}: {}", transaction.category, transaction.note);
        reflected.push(mirror);
    }
    reflected
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::money::{Currency, Money};
    use crate::month::Month;
    use jiff::civil::date;

    fn eur(cents: i64) -> Money {
        Money::new(cents, Currency::EUR)
    }

    #[test]
    fn contributions_and_advances_show_in_the_personal_budget() {
        let alice = MemberId("a".repeat(64));
        let bob = MemberId("b".repeat(64));
        let mut household = Budget::new("Household", Currency::EUR);
        let paid_by = |kind, cents, category: &str, member: &MemberId| {
            let mut transaction =
                Transaction::new(date(2026, 11, 2), kind, eur(cents), category, None);
            transaction.payer = Some(member.clone());
            transaction
        };
        // Alice's contribution, Bob's contribution, Alice's advance.
        for transaction in [
            paid_by(Kind::Income, 100000, "Contributions", &alice),
            paid_by(Kind::Income, 50000, "Contributions", &bob),
            paid_by(Kind::Cost, 3000, "Food", &alice),
        ] {
            household.add_transaction(transaction).unwrap();
        }
        // The budget pays Alice's advance back.
        let mut repayment = Transaction::new(
            date(2026, 11, 20),
            Kind::Cost,
            eur(3000),
            "Settlement",
            None,
        );
        repayment.payee = Some(alice.clone());
        household.add_transaction(repayment).unwrap();

        let mut mine = Budget::new("Alice", Currency::EUR);
        mine.personal = true;
        let shown = mine.with_reflections(reflections(&household, &alice));
        assert!(shown.lines().is_empty());
        let november = Month::new(2026, 11).unwrap();
        assert_eq!(shown.actual(november).costs, eur(103000));
        assert_eq!(shown.actual(november).incomes, eur(3000));
        assert!(
            shown
                .transactions()
                .iter()
                .all(|t| t.category == "Household")
        );
        // The personal budget itself is unchanged.
        assert!(mine.transactions().is_empty());
    }

    #[test]
    fn a_planned_contribution_is_one_line_seen_from_both_budgets() {
        use crate::line::{Line, View};
        use crate::schedule::Schedule;
        use std::num::NonZeroU8;

        let alice = MemberId("a".repeat(64));
        // Alice puts 100 a month into her flat's budget.
        let mut flat = Budget::new("Flat", Currency::EUR);
        let mut line = Line::new(
            "From Alice",
            "Contributions",
            Kind::Income,
            date(2026, 1, 1),
            eur(10000),
            Schedule::Months {
                every: NonZeroU8::MIN,
                first: date(2026, 1, 1),
            },
        )
        .unwrap();
        line.payer = Some(alice.clone());
        let id = flat.add_line(line).unwrap();
        let mut transfer = Transaction::new(
            date(2026, 11, 1),
            Kind::Income,
            eur(10000),
            "Contributions",
            Some(id),
        );
        transfer.payer = Some(alice.clone());
        flat.add_transaction(transfer).unwrap();

        let mut mine = Budget::new("Alice", Currency::EUR);
        mine.personal = true;
        let shown = mine.with_reflections(reflections(&flat, &alice));
        let november = Month::new(2026, 11).unwrap();
        // One cost line of 100, realised by the transfer: counted once.
        let mirror = &shown.lines()[0];
        assert_eq!((mirror.id, mirror.kind), (id, Kind::Cost));
        assert_eq!(mirror.category, "Flat");
        assert_eq!(shown.plan(november, View::Due).costs, eur(10000));
        assert_eq!(shown.actual(november).costs, eur(10000));
        let compared = shown.compare_line(id, november).unwrap();
        assert_eq!(
            (compared.planned, compared.actual),
            (eur(10000), eur(10000))
        );
        // Seen from the flat, it stays an income.
        assert_eq!(flat.plan(november, View::Due).incomes, eur(10000));
    }

    #[test]
    fn only_costs_are_paid_to_a_member() {
        let alice = MemberId("a".repeat(64));
        let mut household = Budget::new("Household", Currency::EUR);
        let mut income = Transaction::new(date(2026, 11, 2), Kind::Income, eur(1), "X", None);
        income.payee = Some(alice);
        assert_eq!(
            household.add_transaction(income),
            Err(crate::budget::BudgetError::PayeeNeedsACost)
        );
    }
}
