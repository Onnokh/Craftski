//! The recipe table: a sortable header over virtual rows.

use std::borrow::Cow;

use rune::{Color, Cursor, El, GrowthSpec, ScrollFade, ScrollSpec, col, el, row, text};

use super::widgets::{Money, flexible, icon_frame, unknown};
use super::{App, Dims, Ready, recipe_at};
use crate::copper::Copper;
use crate::rows::{Column, margin_fill, per_day, percent};
use crate::theme::{self, CAPS, p};

const ROW: f32 = 50.0;

pub fn table(s: &App, r: &Ready, d: Dims) -> El {
    let rows = col().key("rows").w_full().growth(GrowthSpec::default()).virtual_rows(
        r.rows.len(),
        ROW,
        |s: &App, i| s.ready().map_or(Cow::Borrowed(""), |r| Cow::Owned(r.market.recipes[r.rows[i]].spell_id.to_string())),
        move |s: &App, i| s.ready().map_or_else(el, |r| table_row(s, r, r.rows[i], d)),
    );
    let body = if r.rows.is_empty() {
        col().grow(1.0).center().child(text("No recipes match these filters.").font(theme::sans(13.0, 400.0)).color(p().muted))
    } else {
        col().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).scroll_fade(ScrollFade::strength(70.0).full_at(1.0).end_only().linear()).child(rows)
    };
    col().grow(1.0).min_h(0.0).w_full().child(head(s, d)).child(body).role("table").label("Leatherworking recipes")
}

fn head(s: &App, d: Dims) -> El {
    let mut out = row().w_full().h(38.0).px(d.pad).gap(d.gap).center_y().fill(p().ground).border_sides([0.0, 0.0, 1.0, 0.0], p().line);
    for (i, column) in Column::ALL.into_iter().enumerate() {
        let sorted = s.sort.column == column;
        let title = if sorted { format!("{} {}", column.title(), if s.sort.descending { "↓" } else { "↑" }) } else { column.title().to_string() };
        let label = text(title).font(CAPS).color(if sorted { p().brass } else { p().faint }).text_case(rune::TextCase::Uppercase);
        let cell = cell(d, i).child(label).role("columnheader").cursor(Cursor::Pointer).on_press(move |s: &mut App| {
            s.sort = s.sort.clicked(column);
            s.refresh();
        });
        out = out.child(cell);
    }
    out
}

/// The cell for column `i`: the item column takes the leftover width, the rest are fixed and
/// right-aligned.
fn cell(d: Dims, i: usize) -> El {
    match i {
        0 => flexible().items_start(),
        _ => row().w(d.cols[i - 1]).justify_end().center_y(),
    }
}

fn table_row(s: &App, r: &Ready, index: usize, d: Dims) -> El {
    let (recipe, e) = recipe_at(r, index);
    let selected = s.selected == Some(recipe.spell_id);
    let quality = p().quality(recipe.quality);
    let n = Money::new(theme::mono(13.0, 400.0), p().text);
    let money = |c: Option<Copper>| c.map_or_else(|| unknown("no price"), |c| n.show(c));
    let number = |v: String| text(v).font(theme::mono(13.0, 400.0)).color(p().text);
    let item = row()
        .gap(12.0)
        .center_y()
        .w_full()
        .child(icon_frame(s.icons.item(recipe.icon), 32.0, 1.5, 7.0, quality))
        .child(text(&recipe.item_name).font(theme::sans(14.0, 500.0)).color(quality).truncate().shrink().min_w(0.0));
    let margin = e.margin.map_or_else(
        || unknown("—"),
        |m| {
            let label = number(percent(m));
            if d.compact {
                return label;
            }
            let bar = el().w_frac(margin_fill(m)).h_full().fill(if m < 0.0 { p().loss } else { p().profit }.alpha(0.75));
            row().gap(8.0).center_y().child(label).child(row().w(40.0).h(4.0).radius(2.0).clip_rounded(2.0).fill(p().track).child(bar))
        },
    );
    let cells = [
        item,
        recipe.skill.map_or_else(|| unknown("—"), |k| number(k.learn.to_string())),
        money(e.cost),
        money(e.resale),
        e.profit.map_or_else(|| unknown("no price"), |pr| Money::new(theme::mono(13.0, 500.0), p().profit).signed(pr)),
        margin,
        e.sold_per_day.map_or_else(|| unknown("no data"), |v| number(per_day(v))),
        e.auctions.map_or_else(|| unknown("—"), |v| number(v.to_string())),
    ];
    let mut out = row()
        .w_full()
        .h(ROW)
        .px(d.pad)
        .gap(d.gap)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], p().rule)
        .hover(Color::WHITE.alpha(0.025))
        .cursor(Cursor::Pointer)
        .role("row")
        .label(recipe.item_name.clone())
        .checked(selected);
    for (i, content) in cells.into_iter().enumerate() {
        out = out.child(cell(d, i).child(content));
    }
    if selected {
        let brass = p().brass;
        out = out
            .linear_gradient(std::f32::consts::FRAC_PI_2, [(0.0, brass.alpha(0.078)), (0.6, brass.alpha(0.024)), (1.0, brass.alpha(0.0))])
            .child(el().pin(Some(0.0), Some(0.0), None, Some(0.0)).w(3.0).fill(brass));
    }
    let id = recipe.spell_id;
    out.on_press(move |s: &mut App| s.selected = Some(id))
}
