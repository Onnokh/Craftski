//! Which of a table's columns show in the width its rows have.

/// The width a table's name keeps before another column goes.
const NAME: f32 = 160.0;

/// `columns` in their order, each with its width, less the fewest from the front of `drops` that
/// leave the name `NAME` of `room`. Each column takes `gap` before it too. A column `drops` leaves
/// out always shows.
pub fn fit<C: Copy + PartialEq>(columns: &[(C, f32)], drops: &[C], room: f32, gap: f32) -> Vec<(C, f32)> {
    let kept = |n: usize| columns.iter().copied().filter(move |(c, _)| !drops[..n].contains(c));
    let n = (0..drops.len()).find(|&n| NAME + kept(n).map(|(_, w)| w + gap).sum::<f32>() <= room).unwrap_or(drops.len());
    kept(n).collect()
}

#[cfg(test)]
mod tests {
    use super::fit;

    #[test]
    fn columns_go_in_their_drop_order_until_the_name_has_its_width_and_the_rest_always_stay() {
        let columns = [("Skill", 56.0), ("Profit", 112.0), ("Bar", 160.0), ("Margin", 72.0), ("Sold", 80.0)];
        let drops = ["Bar", "Margin", "Skill"];
        let shown = |room: f32| fit(&columns, &drops, room, 16.0).into_iter().map(|(c, _)| c).collect::<Vec<_>>();
        assert_eq!(shown(f32::INFINITY), ["Skill", "Profit", "Bar", "Margin", "Sold"]);
        assert_eq!(shown(720.0), ["Skill", "Profit", "Bar", "Margin", "Sold"]);
        assert_eq!(shown(719.0), ["Skill", "Profit", "Margin", "Sold"]);
        assert_eq!(shown(548.0), ["Skill", "Profit", "Margin", "Sold"]);
        assert_eq!(shown(400.0), ["Profit", "Sold"]);
        assert_eq!(shown(100.0), ["Profit", "Sold"]);
        assert_eq!(fit(&columns, &drops, 548.0, 16.0)[1], ("Profit", 112.0));
    }
}
