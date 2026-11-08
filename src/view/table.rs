//! The recipe table: a sortable header over virtual rows.

use std::borrow::Cow;

use rune::{Color, Cursor, El, GrowthSpec, ScrollSpec, col, el, row, text};

use super::widgets::{Money, flexible, icon_frame, unknown};
use super::{App, Dims, Ready, recipe_at};
use crate::rows::{Column, per_day, percent};
use crate::theme::{self, p};

const ROW: f32 = 32.0;

pub fn table(s: &App, r: &Ready, d: Dims) -> El {
    let rows = col().key("rows").w_full().growth(GrowthSpec::default()).virtual_rows(
        r.rows.len(),
        ROW,
        |s: &App, i| s.ready().map_or(Cow::Borrowed(""), |r| Cow::Owned(r.market.recipes[r.rows[i]].spell_id.to_string())),
        move |s: &App, i| s.ready().map_or_else(el, |r| table_row(s, r, r.rows[i], d)),
    );
    let body = if r.rows.is_empty() {
        col().grow(1.0).center().child(text("No recipes match these filters.").font(theme::BODY).color(p().muted))
    } else {
        col().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(rows)
    };
    col().grow(1.0).min_h(0.0).w_full().child(head(s, d)).child(body).role("table").label("Leatherworking recipes")
}

fn head(s: &App, d: Dims) -> El {
    let mut out = row().w_full().h(ROW).px(d.pad).gap(d.gap).center_y().border_sides([0.0, 0.0, 1.0, 0.0], p().line);
    for (i, column) in Column::ALL.into_iter().enumerate() {
        let sorted = s.sort.column == column;
        let title = if sorted { format!("{} {}", column.title(), if s.sort.descending { "↓" } else { "↑" }) } else { column.title().to_string() };
        let label = text(title).font(theme::LABEL).color(if sorted { p().text } else { p().muted });
        let cell = cell(d, i).child(label).role("columnheader").cursor(Cursor::Pointer).on_press(move |s: &mut App| {
            s.sort = s.sort.clicked(column);
            s.refresh();
        });
        out = out.child(cell);
    }
    out
}

/// The cell for column `i`: the recipe column takes the leftover width, the rest are fixed and
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
    let money = Money::new(theme::BODY, p().text).short();
    let number = |v: String| text(v).font(theme::BODY).color(p().text);
    let item = row()
        .gap(8.0)
        .center_y()
        .w_full()
        .child(icon_frame(s.icons.item(recipe.icon), 24.0, quality))
        .child(text(&recipe.item_name).font(theme::BODY).color(quality).truncate().shrink().min_w(0.0));
    let cells = [
        item,
        recipe.skill.map_or_else(unknown, |k| number(k.learn.to_string())),
        e.cost.map_or_else(unknown, |c| money.show(c)),
        e.resale.map_or_else(unknown, |c| money.show(c)),
        e.profit.map_or_else(unknown, |pr| Money::new(theme::BODY_STRONG, p().text).short().signed(pr)),
        e.margin.map_or_else(unknown, |m| number(percent(m)).color(if m < 0.0 { p().loss } else { p().text })),
        e.sold_per_day.map_or_else(unknown, |v| number(per_day(v))),
        e.auctions.map_or_else(unknown, |v| number(v.to_string())),
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
        out = out.fill(p().selected).child(el().pin(Some(0.0), Some(0.0), None, Some(0.0)).w(2.0).fill(p().brass));
    }
    let id = recipe.spell_id;
    out.on_press(move |s: &mut App| s.selected = Some(id))
}
