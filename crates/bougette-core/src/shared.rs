//! Shared budgets as seen from a member's personal budget: "entered once,
//! visible in two budgets" (doc/scope.md). A contribution or an advance is
//! entered in the shared budget only, and shows as a cost in the personal
//! budget of the member who paid.

use crate::budget::Budget;
use crate::id::MemberId;
use crate::line::Kind;
use crate::transaction::Transaction;

/// What `member` paid or received through `shared`'s transactions, as
/// transactions of their personal budget, in the shared budget's category
/// named after it:
/// - a contribution (an income the member paid) and an advance (a cost the
///   member paid) are costs for the member;
/// - a cost paid to the member (e.g. paying back their advances) is an
///   income for the member.
pub fn reflections(shared: &Budget, member: &MemberId) -> Vec<Transaction> {
    let mut reflected = Vec::new();
    for transaction in shared.transactions() {
        let kind = if transaction.payer.as_ref() == Some(member) {
            Kind::Cost
        } else if transaction.payee.as_ref() == Some(member) {
            Kind::Income
        } else {
            continue;
        };
        let mut mirror = Transaction::new(
            transaction.date,
            kind,
            transaction.amount,
            shared.name.clone(),
            None,
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
