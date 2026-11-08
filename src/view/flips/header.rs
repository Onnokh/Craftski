//! The tab title, where the prices come from, and three tiles that sum up the zone.

use rune::{El, col, row, spacer, text};

use super::Stocked;
use crate::rows::per_day;
use crate::theme::{self, p};
use crate::view::header::{live_dot, origin, tile};
use crate::view::widgets::Money;
use crate::view::{App, Dims, Ready};

pub fn header(s: &App, r: &Ready, f: &Stocked, d: Dims) -> El {
    let zone = &s.flips.zone;
    let merchants = f.zones.iter().find(|z| &z.name == zone).map_or(0, |z| z.merchants);
    let title = col()
        .shrink()
        .min_w(0.0)
        .child(text("Vendor flips").font(theme::display(34.0).lh(34.0).tracking(-0.34)).color(p().text))
        .child(row().mt(8.0).items_start().child(live_dot().mt(6.6)).child(origin(s, r, &format!("{merchants} merchants")).shrink().min_w(0.0)));
    let o = f.summary;
    let pick_tile = |key: &str, pick: Option<usize>| match pick {
        Some(i) => {
            let (flip, merchant, item) = f.at(i);
            let name = item.map_or("Unknown item", |i| i.name.as_str());
            let sales = flip.sold_per_day.map_or_else(|| merchant.name.clone(), |s| format!("{} / day", per_day(s)));
            tile(key, Money::new(theme::mono(15.0, 500.0), p().profit).sign_gap(6.0).signed(flip.profit.unwrap_or(0)), &format!("{name} · {sales}"), d)
        }
        None => tile(key, text("none profitable").font(theme::sans(13.0, 400.0)).color(p().faint), "", d),
    };
    let profitable = row()
        .gap(6.0)
        .child(text(o.profitable.to_string()).font(theme::mono(15.0, 500.0)).color(p().text))
        .child(text(format!("/ {}", o.total)).font(theme::mono(15.0, 500.0)).color(p().faint));
    let tiles = row()
        .gap(10.0)
        .child(pick_tile("Top profit", o.top))
        .child(pick_tile("Best that sells", o.best_selling))
        .child(tile("Profitable", profitable, &format!("{} without a sale rate", o.no_rate), d));
    let out = if d.compact { col().gap(16.0).child(title).child(tiles.w_full()) } else { row().gap(28.0).items_end().child(title).child(spacer()).child(tiles) };
    out.w_full().pt(26.0).pb(19.0).px(d.pad).border_sides([0.0, 0.0, 1.0, 0.0], p().line)
}
