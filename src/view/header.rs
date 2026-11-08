//! The profession title, where the prices come from, and three tiles that sum it up.

use rune::{El, Shadow, Span, col, el, row, spacer, text};

use super::widgets::{Money, caps};
use super::{App, Dims, Ready};
use crate::overview::{Pick, age};
use crate::profit::PriceSource;
use crate::rows::per_day;
use crate::theme::{self, p};

pub fn header(s: &App, r: &Ready, d: Dims) -> El {
    let title = col()
        .shrink()
        .min_w(0.0)
        .child(text("Leatherworking").font(theme::display(34.0).lh(34.0).tracking(-0.34)).color(p().text))
        .child(row().mt(8.0).items_start().child(live_dot().mt(6.6)).child(origin(s, r).shrink().min_w(0.0)));
    let o = &r.overview;
    let pick_tile = |key: &str, pick: Option<Pick>| match pick {
        Some(pick) => {
            let recipe = &r.market.recipes[pick.index];
            let sales = pick.sold_per_day.map_or_else(|| "no sales data".into(), |s| format!("{} / day", per_day(s)));
            tile(key, Money::new(theme::mono(15.0, 500.0), p().profit).signed(pick.profit), &format!("{} · {sales}", recipe.item_name), d)
        }
        None => tile(key, text("none priced").font(theme::sans(13.0, 400.0)).color(p().faint), "", d),
    };
    let priced = row()
        .gap(6.0)
        .child(text(o.priced.to_string()).font(theme::mono(15.0, 500.0)).color(p().text))
        .child(text(format!("/ {}", o.total)).font(theme::mono(15.0, 500.0)).color(p().faint));
    let tiles = row()
        .gap(10.0)
        .child(pick_tile("Top profit", o.top))
        .child(pick_tile("Best that sells", o.best_selling))
        .child(tile("Fully priced", priced, &format!("{} miss a reagent price", o.missing_reagent), d));
    // Beside the title a narrow window leaves each tile too little room, so they drop below it.
    let out = if d.compact { col().gap(16.0).child(title).child(tiles.w_full()) } else { row().gap(28.0).items_end().child(title).child(spacer()).child(tiles) };
    out.w_full().pt(26.0).pb(19.0).px(d.pad).border_sides([0.0, 0.0, 1.0, 0.0], p().line)
}

/// A green dot: the snapshot loaded.
fn live_dot() -> El {
    el().w(6.0)
        .h(6.0)
        .radius(3.0)
        .mr(6.0)
        .fill(p().profit)
        .box_shadow(Shadow { spread: 3.0, color: p().profit.alpha(0.133), ..Shadow::default() })
        .a11y_hidden()
}

/// "WoW Forever · PvP-Horde · TSM snapshot 9 min old · 592 recipes", with the realm and age
/// picked out.
fn origin(s: &App, r: &Ready) -> El {
    let book = &r.market.book;
    let (scope, updated) = match s.source {
        PriceSource::Realm => (book.realm_scope.as_deref(), book.realm_updated),
        PriceSource::Region => (book.region_scope.as_deref(), book.region_updated),
    };
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let mut line = String::from("WoW Forever · ");
    let mut bold = Vec::new();
    let mut push_bold = |line: &mut String, part: &str| {
        bold.push((line.len(), line.len() + part.len()));
        line.push_str(part);
    };
    if let Some(scope) = scope {
        push_bold(&mut line, scope);
        line.push_str(" · ");
    }
    match updated {
        Some(t) => {
            line.push_str("TSM snapshot ");
            push_bold(&mut line, &age(now.saturating_sub(t)));
            line.push_str(" old");
        }
        None => line.push_str("TSM snapshot of unknown age"),
    }
    line.push_str(&format!(" · {} recipes", r.market.recipes.len()));
    let spans: Vec<Span> = bold.into_iter().map(|(a, b)| Span::new(a, b).weight(500.0).color(p().text)).collect();
    text(line).font(theme::sans(13.0, 400.0)).color(p().muted).spans(spans).truncate()
}

fn tile(key: &str, value: El, sub: &str, d: Dims) -> El {
    let tile = if d.compact { col().w(0.0).grow(1.0) } else { col() };
    tile
        .min_w(if d.compact { 0.0 } else { 178.0 })
        .shrink()
        .px(15.0)
        .py(13.0)
        .radius(12.0)
        .fill(p().surface)
        .border(1.0, p().line)
        .child(caps(key, p().faint))
        .child(row().mt(6.0).center_y().child(value))
        .child(text(sub).font(theme::sans(12.0, 400.0)).color(p().muted).mt(3.0).truncate())
}
