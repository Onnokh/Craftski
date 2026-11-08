//! The numbers behind the small charts: bar widths, sold-per-day pips, the breakdown's stacked
//! shares and where a recipe stands among the others. UI-free.

/// The sales a day at which each pip lights.
pub const PIP_STEPS: [f64; 3] = [1.0, 5.0, 20.0];

/// How many of the three pips light for a sale rate.
pub fn pips_lit(sold_per_day: f64) -> usize {
    PIP_STEPS.iter().filter(|&&step| sold_per_day >= step).count()
}

/// A bar `full` points long at `max`, or None when there is nothing to draw: a loss, zero, or
/// no positive `max` to measure against.
pub fn bar(value: i64, max: i64, full: f32) -> Option<f32> {
    (value > 0 && max > 0).then(|| (value.min(max) as f64 / max as f64 * f64::from(full)) as f32)
}

/// `part` as a fraction of `whole`, 0 when `whole` is.
pub fn share(part: u64, whole: u64) -> f32 {
    if whole == 0 { 0.0 } else { (part as f64 / whole as f64) as f32 }
}

/// Where a sale price goes, as fractions of it that add up to one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Split {
    pub cut: f32,
    pub materials: f32,
    pub profit: f32,
    /// The materials cost more than the cut leaves, so `materials` is cut short at the end of
    /// the bar and there is no profit.
    pub over: bool,
}

/// The auction house cut, the materials and the profit as shares of the sale price `gross`;
/// None when nothing sells.
pub fn split(gross: u64, cut: u64, materials: u64) -> Option<Split> {
    if gross == 0 {
        return None;
    }
    let left = gross.saturating_sub(cut);
    Some(match left.checked_sub(materials) {
        Some(profit) => Split { cut: share(cut, gross), materials: share(materials, gross), profit: share(profit, gross), over: false },
        None => Split { cut: share(cut, gross), materials: share(left, gross), profit: 0.0, over: true },
    })
}

/// One measure across every priced recipe: a tick per distinct position on a strip `width`
/// points long, the chosen recipe's position, and its rank from the top.
#[derive(Clone, Debug, PartialEq)]
pub struct Standing {
    pub ticks: Vec<f32>,
    pub at: f32,
    pub rank: usize,
}

/// `values` holds every recipe's measure, `mine` among them. Each sits at value / max of the
/// strip, a loss on its left edge; ticks that land on the same half point merge. Ties share the
/// better rank.
pub fn standing(values: &[f64], mine: f64, width: f32) -> Standing {
    let max = values.iter().copied().fold(mine, f64::max);
    let x = |v: f64| if max > 0.0 { ((v / max).clamp(0.0, 1.0) * f64::from(width) * 2.0).round() as f32 / 2.0 } else { 0.0 };
    let mut ticks: Vec<f32> = values.iter().map(|&v| x(v)).collect();
    ticks.sort_by(f32::total_cmp);
    ticks.dedup();
    Standing { ticks, at: x(mine), rank: 1 + values.iter().filter(|&&v| v > mine).count() }
}

/// "1st", "2nd", "3rd", "4th", "11th", "12th", "13th", "21st".
pub fn ordinal(n: usize) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pips_light_at_one_five_and_twenty_a_day() {
        let lit: Vec<usize> = [0.0, 0.99, 1.0, 4.9, 5.0, 19.9, 20.0, 41.0].into_iter().map(pips_lit).collect();
        assert_eq!(lit, vec![0, 0, 1, 1, 2, 2, 3, 3]);
    }

    #[test]
    fn a_bar_measures_profit_against_the_largest_and_skips_losses() {
        assert_eq!(bar(38_173, 38_173, 160.0), Some(160.0));
        assert_eq!(bar(3, 8, 160.0), Some(60.0));
        assert_eq!(bar(1, 40, 160.0), Some(4.0));
        assert_eq!(bar(0, 40, 160.0), None);
        assert_eq!(bar(-500, 40, 160.0), None);
        assert_eq!(bar(10, 0, 160.0), None);
        assert_eq!(bar(80, 40, 160.0), Some(160.0));
    }

    #[test]
    fn shares_are_fractions_of_the_whole() {
        assert_eq!(share(3, 8), 0.375);
        assert_eq!(share(1, 4), 0.25);
        assert_eq!(share(5, 0), 0.0);
    }

    #[test]
    fn a_sale_splits_into_cut_materials_and_profit() {
        assert_eq!(split(1_000, 50, 200), Some(Split { cut: 0.05, materials: 0.2, profit: 0.75, over: false }));
        assert_eq!(split(2_000, 100, 500), Some(Split { cut: 0.05, materials: 0.25, profit: 0.7, over: false }));
        assert_eq!(split(1_000, 50, 950), Some(Split { cut: 0.05, materials: 0.95, profit: 0.0, over: false }));
        assert_eq!(split(0, 0, 10), None);
    }

    #[test]
    fn a_loss_clamps_materials_to_what_the_cut_leaves() {
        assert_eq!(split(1_000, 50, 1_400), Some(Split { cut: 0.05, materials: 0.95, profit: 0.0, over: true }));
        assert_eq!(split(1_000, 50, 951), Some(Split { cut: 0.05, materials: 0.95, profit: 0.0, over: true }));
    }

    #[test]
    fn a_standing_places_ticks_at_value_over_max_and_ranks_from_the_top() {
        let s = standing(&[400.0, 100.0, 200.0, 100.0, -50.0, 0.0], 200.0, 200.0);
        assert_eq!(s, Standing { ticks: vec![0.0, 50.0, 100.0, 200.0], at: 100.0, rank: 2 });
        assert_eq!(standing(&[400.0, 100.0], 400.0, 200.0).rank, 1);
        assert_eq!(standing(&[10.0, 30.0, 30.0, 20.0], 20.0, 100.0).rank, 3);
        assert_eq!(standing(&[30.0, 30.0, 10.0], 30.0, 100.0).rank, 1);
    }

    #[test]
    fn ticks_round_to_half_points_and_merge() {
        let s = standing(&[1000.0, 1.0, 2.0, 3.0, 333.0], 1.0, 231.0);
        assert_eq!(s.ticks, vec![0.0, 0.5, 77.0, 231.0]);
        assert_eq!((s.at, s.rank), (0.0, 5));
    }

    #[test]
    fn with_no_positive_value_everything_sits_on_the_left_edge() {
        assert_eq!(standing(&[-10.0, -20.0], -20.0, 100.0), Standing { ticks: vec![0.0], at: 0.0, rank: 2 });
    }

    #[test]
    fn ordinals_take_their_english_suffix() {
        let words: Vec<String> = [1, 2, 3, 4, 11, 12, 13, 21, 22, 23, 101, 111, 112, 113].into_iter().map(ordinal).collect();
        assert_eq!(words, ["1st", "2nd", "3rd", "4th", "11th", "12th", "13th", "21st", "22nd", "23rd", "101st", "111th", "112th", "113th"]);
    }
}
