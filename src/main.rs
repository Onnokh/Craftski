// A release build opens no console window beside its own; debug builds keep it for their logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use craftski::{cache, copper, flip_rows, flips, icons, market, merchants, profit, public_data, recipes, rows, settings, skin, theme, view, wow};
use rune::{AppSpec, Display, WindowOptions, WindowSpec};

const SIZE: (f32, f32) = (1640.0, 1000.0);

fn main() {
    cache::adopt_old_folder();
    let cli = std::env::args().find_map(|a| match a.as_str() {
        "--dump" => Some(dump as fn() -> Result<(), String>),
        "--fetch-icons" => Some(fetch_icons),
        "--fetch-skin" => Some(fetch_skin),
        "--flips" => Some(flips),
        "--icon-names" => Some(icon_names),
        _ => None,
    });
    if cli.is_some() {
        attach_console();
    }
    if let Err(e) = settings::start_profession() {
        attach_console();
        eprintln!("{e}");
        std::process::exit(2);
    }
    if let Some(run) = cli {
        std::process::exit(match run() {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("{e}");
                1
            }
        });
    }
    let (fonts, families) = (theme::FONTS, theme::FAMILIES);
    rune::run_windows(move |displays: &[Display]| {
        let spec = AppSpec::<view::App> {
            scenes: &[("loaded", view::scene_loaded), ("crafted", view::scene_crafted), ("flips", view::scene_flips), ("flips-shared", view::scene_flips_shared), ("zones", view::scene_zones), ("flips-sold", view::scene_flips_sold), ("crafts-hidden", view::scene_crafts_hidden), ("crafts-sold", view::scene_crafts_sold), ("flips-hidden", view::scene_flips_hidden), ("stale", view::scene_stale), ("realms", view::scene_realms), ("reload-failed", view::scene_reload_failed), ("no-appdata", view::scene_no_appdata), ("welcome", view::scene_welcome), ("welcome-public", view::scene_welcome_public), ("settings", view::scene_settings), ("public", view::scene_public), ("settings-public", view::scene_settings_public)],
            setup: Some(view::setup),
            poll: Some(view::poll),
            on_key: Some(view::on_key),
            fonts,
            families,
            ..AppSpec::new("Craftski", SIZE, view::App::new, view::view)
        };
        let window = WindowSpec::new("main", spec);
        match displays.iter().find(|d| !d.primary) {
            Some(d) => [window.options(WindowOptions { position: Some(centered(d.position, d.size, d.scale, SIZE)), ..Default::default() })],
            None => [window],
        }
    });
}

/// Writes what follows to the terminal the program was run from, which a release build, being a
/// windowed program, has none of. Without one, or when the debug build has its own, it does
/// nothing.
#[cfg(windows)]
fn attach_console() {
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn AttachConsole(process: u32) -> i32;
    }
    // SAFETY: takes a process id by value and touches no Rust memory.
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(not(windows))]
fn attach_console() {}

/// Top-left that centers a window of `size` points on a display, in physical desktop pixels.
fn centered(origin: (i32, i32), display: (u32, u32), scale: f64, size: (f32, f32)) -> (i32, i32) {
    let along = |o: i32, span: u32, points: f32| o + ((f64::from(span) - f64::from(points) * scale) / 2.0).max(0.0).round() as i32;
    (along(origin.0, display.0, size.0), along(origin.1, display.1, size.1))
}

/// Where the CLI reads prices, the source chosen in the app's settings. Err naming `--appdata`
/// when that is the desktop app's AppData.lua and none is found.
fn origin() -> Result<market::Origin, String> {
    match settings::load_source() {
        market::PriceSource::Public(house) => Ok(market::Origin::Public(house, settings::load_rules().side)),
        market::PriceSource::App => wow::locate().map(|a| market::Origin::AppData(a.path)).ok_or_else(|| "No TSM AppData.lua found under a WoW Forever install; pass --appdata PATH (the TradeSkillMaster_AppHelper addon's AppData.lua) or choose the folder in the app.".into()),
    }
}

/// `--dump [--profession NAME]`: the profession's 15 most profitable recipes at realm prices.
fn dump() -> Result<(), String> {
    use rows::{Column, Filter, Sort};
    let profession = settings::start_profession()?;
    let m = market::load(&origin()?, public_data::Refresh::IfDue, &settings::load_realm(), profession)?;
    let evals: Vec<_> = m.recipes.iter().map(|r| profit::evaluate(r, &m.book, m.costs())).collect();
    let rows = rows::visible(&m.recipes, &evals, Filter { hide_unpriced: true, min_sold_per_day: None, max_skill: None, query: "" }, Sort { column: Column::Profit, descending: true });
    let v = &m.book.vendor;
    println!("{}: {} recipes, {} realm-priced items, {} region-priced items", profession.name(), m.recipes.len(), m.book.realm().map_or(0, |(_, b)| b.prices.len()), m.book.region.len());
    println!("{} vendor prices: {} in TSM's list, {} in its saved data", v.len(), v.from_list, v.from_saved);
    let reagents: std::collections::BTreeSet<u32> = m.recipes.iter().flat_map(|r| r.reagents.iter().map(|g| g.item_id)).collect();
    let realm = m.book.realm().map_or("?", |(name, _)| name);
    let priced = evals.iter().filter(|e| e.profit.is_some()).count();
    let missing = evals.iter().filter(|e| !e.unpriced.is_empty()).count();
    let crafted = reagents.iter().filter(|&&item| m.costs().get(item).is_some_and(|p| matches!(p.origin, profit::PriceOrigin::Crafted { .. }))).count();
    println!("fully priced ({realm}): {priced}/{}, {missing} miss a reagent price, {crafted} reagents cheapest to craft", evals.len());
    println!("{:<36} {:>5} {:>4} {:>14} {:>14} {:>14} {:>9} {:>9} {:>6}", "Item", "Skill", "Qty", "Cost", "Resale", "Profit", "Margin", "Sold/day", "Auct");
    let na = || "n/a".to_string();
    for &i in rows.iter().take(15) {
        let (r, e) = (&m.recipes[i], &evals[i]);
        println!(
            "{:<36} {:>5} {:>4} {:>14} {:>14} {:>14} {:>9} {:>9} {:>6}",
            r.item_name,
            r.learn().map_or_else(na, |learn| learn.to_string()),
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

/// `--flips [ZONE]`: the zone's 20 most profitable vendor flips at realm prices.
fn flips() -> Result<(), String> {
    use flip_rows::{Column, Filter};
    let args: Vec<String> = std::env::args().collect();
    let zone = args.iter().position(|a| a == "--flips").and_then(|i| args.get(i + 1)).filter(|z| !z.starts_with("--")).map_or(flip_rows::HOME, String::as_str);
    let (mut book, _) = market::read_prices(&origin()?, public_data::Refresh::IfDue)?;
    book.choose_realm(&settings::load_realm());
    let c = merchants::load()?;
    let all = flips::evaluate(&c.merchants, &c.items, &book, &settings::load_rules());
    let filter = Filter { zone, hide_unprofitable: false, min_sold_per_day: None, query: "", supply: flip_rows::Supply::All };
    let rows = flip_rows::visible(&all, &c.merchants, &c.items, filter, rows::Sort { column: Column::Profit, descending: true });
    let offers: Vec<usize> = rows.iter().flat_map(|r| std::iter::once(r.flip).chain(r.others.iter().copied())).collect();
    let merchants: std::collections::BTreeSet<usize> = offers.iter().map(|&i| all[i].merchant).collect();
    let profitable = rows.iter().filter(|r| all[r.flip].profit.is_some_and(|p| p > 0)).count();
    let from_tsm = rows.iter().filter(|r| matches!(all[r.flip].paid, Some(flips::Paid::Tsm(_)))).count();
    let shared = rows.iter().filter(|r| !r.others.is_empty()).count();
    println!(
        "{zone}: {} merchants, {} flips from {} offers ({shared} items sold by more than one merchant), {profitable} profitable, {from_tsm} paid at TSM's price; realm {}",
        merchants.len(),
        rows.len(),
        offers.len(),
        book.realm().map_or("?", |(name, _)| name)
    );
    let by_supply: Vec<String> = flip_rows::Supply::ALL.iter().map(|&supply| format!("{} {}", supply.label(), flip_rows::visible(&all, &c.merchants, &c.items, Filter { supply, ..filter }, rows::Sort { column: Column::Profit, descending: true }).len())).collect();
    println!("flips by supply: {}", by_supply.join(", "));
    println!("{:<34} {:<24} {:>10} {:<9} {:>11} {:>8} {:>11} {:>5} {:>7} {:>5} Stock", "Item", "Vendor", "Paid", "From", "Sells", "Deposit", "Profit", "Rate", "Sold/d", "Limit");
    let na = || "n/a".to_string();
    for r in rows.iter().take(20) {
        let (f, v) = (&all[r.flip], &c.merchants[all[r.flip].merchant]);
        let vendor: String = v.name.chars().take(20).collect();
        println!(
            "{:<34} {:<24} {:>10} {:<9} {:>11} {:>8} {:>11} {:>5} {:>7} {:>5} {}",
            c.items.get(&f.item).map_or_else(|| format!("Item #{}", f.item), |it| it.name.chars().take(34).collect()),
            if r.others.is_empty() { vendor } else { format!("{vendor} +{}", r.others.len()) },
            f.paid.map_or_else(na, |p| p.copper().to_string()),
            f.paid.map_or_else(na, flips::Paid::origin),
            f.sale.map_or_else(na, |s| s.unit.to_string()),
            f.deposit.loss.map_or_else(|| "?".into(), |l| copper::Copper(l.round() as u64).to_string()),
            f.profit.map_or_else(na, copper::signed),
            f.sale_rate.map_or_else(na, |r| format!("{:.0}%", r * 100.0)),
            f.sold_per_day.map_or_else(na, |d| format!("{d:.1}")),
            f.limit.map_or_else(|| "-".into(), |l| l.to_string()),
            v.source.label(),
        );
    }
    Ok(())
}

/// Every icon every profession's recipes draw.
fn fetch_icons() -> Result<(), String> {
    let each = recipes::load_each(&recipes::Profession::ALL)?;
    let ids: std::collections::BTreeSet<_> = each.iter().flat_map(|recipes| icons::referenced(recipes)).collect();
    let started = std::time::Instant::now();
    let report = icons::prefetch(&ids, |_, _| {})?;
    let dir = icons::dir().unwrap_or_default();
    println!(
        "{} icons in {}: {} cached, {} downloaded ({} bytes), {} failed, {:.2}s",
        report.icons,
        dir.display(),
        report.cached,
        report.downloaded,
        report.bytes,
        report.failed.len(),
        started.elapsed().as_secs_f64()
    );
    for (id, e) in &report.failed {
        println!("  {}: {e}", id.0);
    }
    Ok(())
}

/// Writes the Wowhead name of every icon the browser build shows, the recipes' of every
/// profession and the merchants' goods, to the path after `--icon-names`. Icons not yet cached
/// are fetched first, as `--fetch-icons` does. Its output is checked in as web/site/icon-names.json
/// and rerun when `recipes::BUILD` changes, so a deploy never looks names up on wago.tools.
fn icon_names() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let path = args.iter().position(|a| a == "--icon-names").and_then(|i| args.get(i + 1)).ok_or("--icon-names needs the path to write")?;
    let each = recipes::load_each(&recipes::Profession::ALL)?;
    let catalog = merchants::load()?;
    let ids: std::collections::BTreeSet<_> = each.iter().flat_map(|recipes| icons::referenced(recipes)).chain(catalog.items.values().filter_map(|i| i.icon)).collect();
    let report = icons::prefetch(&ids, |_, _| {})?;
    let known = icons::known_names();
    let names: std::collections::BTreeMap<u32, &String> = ids.iter().filter_map(|id| Some((id.0, known.get(&id.0)?))).collect();
    let json = serde_json::to_string(&names).map_err(|e| e.to_string())?;
    std::fs::write(path, json + "\n").map_err(|e| format!("{path}: {e}"))?;
    println!("{} of {} icons named in {path}, {} downloaded, {} failed", names.len(), ids.len(), report.downloaded, report.failed.len());
    for id in ids.iter().filter(|id| !names.contains_key(&id.0)) {
        println!("  no name for {}", id.0);
    }
    Ok(())
}

fn fetch_skin() -> Result<(), String> {
    let started = std::time::Instant::now();
    let ready = std::sync::atomic::AtomicUsize::new(0);
    let pieces: Vec<skin::Piece> = skin::Piece::all().collect();
    let result = skin::fetch(&pieces, |_, _| {
        ready.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    });
    println!("{} of {} pieces in {}, {:.2}s", ready.into_inner(), pieces.len(), skin::dir().unwrap_or_default().display(), started.elapsed().as_secs_f64());
    result
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
