mod copper;
mod market;
mod prices;
mod profit;
mod recipes;
mod rows;
mod view;

use rune::{AppSpec, Display, WindowOptions, WindowSpec};

const SIZE: (f32, f32) = (1280.0, 800.0);

fn main() {
    if std::env::args().any(|a| a == "--dump") {
        std::process::exit(match dump() {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("{e}");
                1
            }
        });
    }
    rune::run_windows(|displays: &[Display]| {
        let spec = AppSpec::<view::App> {
            scenes: &[("loaded", view::scene_loaded)],
            setup: Some(view::setup),
            poll: Some(view::poll),
            ..AppSpec::new("TSM Crafts", SIZE, view::App::new, view::view)
        };
        let window = WindowSpec::new("main", spec);
        match displays.iter().find(|d| !d.primary) {
            Some(d) => [window.options(WindowOptions { position: Some(centered(d.position, d.size, d.scale, SIZE)), ..Default::default() })],
            None => [window],
        }
    });
}

/// Top-left that centers a window of `size` points on a display, in physical desktop pixels.
fn centered(origin: (i32, i32), display: (u32, u32), scale: f64, size: (f32, f32)) -> (i32, i32) {
    let along = |o: i32, span: u32, points: f32| o + ((f64::from(span) - f64::from(points) * scale) / 2.0).max(0.0).round() as i32;
    (along(origin.0, display.0, size.0), along(origin.1, display.1, size.1))
}

fn dump() -> Result<(), String> {
    use profit::PriceSource;
    use rows::{Column, Filter, Sort};
    let m = market::load()?;
    let evals: Vec<_> = m.recipes.iter().map(|r| profit::evaluate(r, &m.book, PriceSource::Realm)).collect();
    let rows = rows::visible(&m.recipes, &evals, Filter { hide_unpriced: true, min_sold_per_day: 0.0 }, Sort { column: Column::Profit, descending: true });
    println!("{} recipes, {} realm-priced items, {} region-priced items, {} fully priced (realm)", m.recipes.len(), m.book.realm.len(), m.book.region.len(), rows.len());
    println!("{:<36} {:>4} {:>14} {:>14} {:>14} {:>9} {:>9} {:>6}", "Item", "Qty", "Cost", "Resale", "Profit", "Margin", "Sold/day", "Auct");
    let na = || "n/a".to_string();
    for &i in rows.iter().take(15) {
        let (r, e) = (&m.recipes[i], &evals[i]);
        println!(
            "{:<36} {:>4} {:>14} {:>14} {:>14} {:>9} {:>9} {:>6}",
            r.item_name,
            r.quantity,
            e.cost.map_or_else(na, |c| c.to_string()),
            e.resale.map_or_else(na, |c| c.to_string()),
            e.profit.map_or_else(na, copper::signed),
            e.margin.map_or_else(na, |m| format!("{:.1}%", m * 100.0)),
            e.sold_per_day.map_or_else(na, |d| format!("{d:.2}")),
            e.auctions.map_or_else(na, |n| n.to_string()),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::centered;

    #[test]
    fn centers_on_a_display_left_of_and_below_the_primary() {
        assert_eq!(centered((-2048, 1000), (2048, 1152), 1.0, (1280.0, 800.0)), (-1664, 1176));
        assert_eq!(centered((-2048, 1000), (2048, 1152), 1.5, (1280.0, 800.0)), (-1984, 1000));
    }
}
