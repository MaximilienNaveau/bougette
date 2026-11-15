//! The fair-split calculator (GLOSSARY.md: Fair-split calculator).
//!
//! A standalone tool: it takes the figures it is given and stores nothing.
//! Incomes in particular never leave this function. The total to split is
//! usually a shared budget's smoothed costs, `plan(month,
//! View::Smoothed).costs`, or a total typed in directly.

use crate::money::Money;

/// How to divide the total between members (GLOSSARY.md: Split rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitRule {
    /// Each member pays in proportion to their income.
    ProportionalToIncome,
    /// Each member pays the same.
    Equal,
}

/// Each member's suggested contribution to `total`, in the order of
/// `incomes` (one income per member; `Equal` ignores their values).
///
/// The shares add up exactly to `total`: the cents left over by rounding
/// go to the members with the largest remainders.
///
/// `None` if there are no members, if an income is negative or in another
/// currency, or if every income is zero with `ProportionalToIncome`.
pub fn split(total: Money, incomes: &[Money], rule: SplitRule) -> Option<Vec<Money>> {
    let currency = total.currency();
    if incomes.is_empty()
        || incomes
            .iter()
            .any(|income| income.minor_units() < 0 || income.currency() != currency)
    {
        return None;
    }
    // Each member's weight: their income, or 1 for an equal split.
    // `collect` builds the Vec from the iterator, guided by the type
    // annotation `Vec<i128>`.
    let weights: Vec<i128> = incomes
        .iter()
        .map(|income| match rule {
            SplitRule::ProportionalToIncome => i128::from(income.minor_units()),
            SplitRule::Equal => 1,
        })
        .collect();
    let total_weight: i128 = weights.iter().sum();
    if total_weight == 0 {
        return None;
    }
    let cents = i128::from(total.minor_units());
    // Each share rounded down (Euclidean division rounds towards minus
    // infinity, so a negative total, a refund, works too), with what
    // rounding left out of it.
    let mut shares: Vec<i128> = Vec::new();
    let mut remainders: Vec<i128> = Vec::new();
    for weight in &weights {
        shares.push((cents * weight).div_euclid(total_weight));
        remainders.push((cents * weight).rem_euclid(total_weight));
    }
    // Hand out the missing cents, one each, largest remainders first.
    let missing = cents - shares.iter().sum::<i128>();
    // The members' positions, sorted by decreasing remainder. A stable
    // sort keeps the original order between equal remainders.
    // `(0..n)` is the range 0, 1, …, n-1.
    let mut order: Vec<usize> = (0..shares.len()).collect();
    order.sort_by_key(|&member| std::cmp::Reverse(remainders[member]));
    // `take` keeps the first `missing` positions; `as usize` is safe as
    // fewer cents are missing than there are members.
    for &member in order.iter().take(missing as usize) {
        shares[member] += 1;
    }
    Some(
        shares
            .into_iter()
            .map(|share| Money::new(i64::try_from(share).expect("share fits in i64"), currency))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::money::Currency;

    fn eur(cents: i64) -> Money {
        Money::new(cents, Currency::EUR)
    }

    #[test]
    fn proportional_to_incomes() {
        // €2,500 of shared costs, incomes €3,000 and €2,000: 60% / 40%.
        let shares = split(
            eur(250000),
            &[eur(300000), eur(200000)],
            SplitRule::ProportionalToIncome,
        );
        assert_eq!(shares, Some(vec![eur(150000), eur(100000)]));
    }

    #[test]
    fn shares_always_add_up_to_the_total() {
        // €100.00 in three equal parts: 33.34 + 33.33 + 33.33.
        let shares = split(eur(10000), &[eur(0); 3], SplitRule::Equal).unwrap();
        assert_eq!(shares, vec![eur(3334), eur(3333), eur(3333)]);

        // Awkward incomes: the shares still add up to the cent.
        let shares = split(
            eur(123457),
            &[eur(271828), eur(314159), eur(161803)],
            SplitRule::ProportionalToIncome,
        )
        .unwrap();
        let sum = shares.iter().fold(eur(0), |total, &share| total + share);
        assert_eq!(sum, eur(123457));
    }

    #[test]
    fn refuses_what_cannot_be_split() {
        assert_eq!(split(eur(100), &[], SplitRule::Equal), None);
        assert_eq!(
            split(eur(100), &[eur(0), eur(0)], SplitRule::ProportionalToIncome),
            None
        );
        assert_eq!(
            split(
                eur(100),
                &[eur(-1), eur(5)],
                SplitRule::ProportionalToIncome
            ),
            None
        );
    }
}
