//! The numbers behind the small charts: bar widths, sold-per-day pips and where a recipe ranks
//! among the others. UI-free.

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

/// Where `mine` ranks from the top among `values`, which hold it. Ties share the better rank.
pub fn rank(values: &[f64], mine: f64) -> usize {
    1 + values.iter().filter(|&&v| v > mine).count()
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
    fn a_rank_counts_from_the_top_and_ties_share_the_better_one() {
        assert_eq!(rank(&[400.0, 100.0, 200.0, 100.0, -50.0, 0.0], 200.0), 2);
        assert_eq!(rank(&[400.0, 100.0], 400.0), 1);
        assert_eq!(rank(&[10.0, 30.0, 30.0, 20.0], 20.0), 3);
        assert_eq!(rank(&[30.0, 30.0, 10.0], 30.0), 1);
        assert_eq!(rank(&[-10.0, -20.0], -20.0), 2);
    }

    #[test]
    fn ordinals_take_their_english_suffix() {
        let words: Vec<String> = [1, 2, 3, 4, 11, 12, 13, 21, 22, 23, 101, 111, 112, 113].into_iter().map(ordinal).collect();
        assert_eq!(words, ["1st", "2nd", "3rd", "4th", "11th", "12th", "13th", "21st", "22nd", "23rd", "101st", "111th", "112th", "113th"]);
    }
}
