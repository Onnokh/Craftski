//! The header's summary of a whole profession under one price source. UI-free.

use crate::profit::Evaluation;

/// A recipe the header calls out, by index into the recipe list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pick {
    pub index: usize,
    pub profit: i64,
    pub sold_per_day: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Overview {
    pub top: Option<Pick>,
    /// The most profitable recipe that sells at least once a day across the region.
    pub best_selling: Option<Pick>,
    /// Recipes with a known profit.
    pub priced: usize,
    pub total: usize,
    /// Recipes with at least one reagent nobody has priced.
    pub missing_reagent: usize,
}

pub fn overview(evals: &[Evaluation]) -> Overview {
    let picks = || evals.iter().enumerate().filter_map(|(index, e)| Some(Pick { index, profit: e.profit?, sold_per_day: e.sold_per_day }));
    // The first of equal profits wins, so ties resolve by recipe order rather than at random.
    let best = |a: Option<Pick>, p: Pick| if a.is_some_and(|a| a.profit >= p.profit) { a } else { Some(p) };
    Overview {
        top: picks().fold(None, best),
        best_selling: picks().filter(|p| p.sold_per_day.is_some_and(|s| s >= 1.0)).fold(None, best),
        priced: evals.iter().filter(|e| e.profit.is_some()).count(),
        total: evals.len(),
        missing_reagent: evals.iter().filter(|e| !e.unpriced.is_empty()).count(),
    }
}

/// How long ago a snapshot was taken, in its two largest units.
pub fn age(secs: u64) -> String {
    let (m, h, d) = (secs / 60, secs / 3600, secs / 86_400);
    if d > 0 {
        format!("{d}d {}h", h % 24)
    } else if h > 0 {
        format!("{h}h {:02}m", m % 60)
    } else {
        format!("{m} min")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::copper::Copper;

    fn eval(profit: Option<i64>, sold: Option<f64>, unpriced: Vec<u32>) -> Evaluation {
        Evaluation { cost: profit.map(|_| Copper(100)), unpriced, resale: None, profit, margin: None, sold_per_day: sold, auctions: None }
    }

    #[test]
    fn picks_the_top_earner_and_the_best_that_sells_daily() {
        let evals = [
            eval(Some(38_173), None, vec![]),
            eval(Some(6_942), Some(23.5), vec![]),
            eval(Some(9_000), Some(0.4), vec![]),
            eval(None, Some(50.0), vec![2321]),
            eval(Some(-500), Some(80.0), vec![]),
            eval(None, None, vec![]),
        ];
        assert_eq!(overview(&evals), Overview {
            top: Some(Pick { index: 0, profit: 38_173, sold_per_day: None }),
            best_selling: Some(Pick { index: 1, profit: 6_942, sold_per_day: Some(23.5) }),
            priced: 4,
            total: 6,
            missing_reagent: 1,
        });
    }

    #[test]
    fn nothing_priced_means_no_picks() {
        assert_eq!(overview(&[eval(None, Some(3.0), vec![7])]), Overview { top: None, best_selling: None, priced: 0, total: 1, missing_reagent: 1 });
    }

    #[test]
    fn ages_read_in_the_two_largest_units() {
        assert_eq!(age(59), "0 min");
        assert_eq!(age(9 * 60 + 30), "9 min");
        assert_eq!(age(3 * 3600 + 5 * 60), "3h 05m");
        assert_eq!(age(2 * 86_400 + 4 * 3600 + 120), "2d 4h");
    }
}
