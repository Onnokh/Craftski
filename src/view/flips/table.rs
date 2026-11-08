//! The flip table: a sortable header over virtual rows.

use std::borrow::Cow;

use rune::{Color, Cursor, El, GrowthSpec, ScrollFade, ScrollSpec, col, el, row, text};

use super::{Stocked, row_key};
use crate::copper::Copper;
use crate::flip_rows::{Column, Row};
use crate::rows::per_day;
use crate::theme::{self, CAPS, p};
use crate::view::widgets::{Money, flexible, icon_frame, unknown};
use crate::view::{App, Dims};

const ROW: f32 = 54.0;

/// Vendor, paid, sells for, deposit loss, profit, sale rate, sold/day, limit.
fn widths(d: Dims) -> [f32; 8] {
    if d.compact { [132.0, 96.0, 92.0, 72.0, 96.0, 48.0, 56.0, 40.0] } else { [172.0, 112.0, 104.0, 92.0, 112.0, 72.0, 72.0, 48.0] }
}

pub fn table(s: &App, f: &Stocked, d: Dims) -> El {
    let rows = col().key("flip-rows").w_full().growth(GrowthSpec::default()).virtual_rows(
        f.rows.len(),
        ROW,
        |s: &App, i| s.flips.stocked().map_or(Cow::Borrowed(""), |f| Cow::Owned(row_key(f.flips[f.rows[i].flip].item))),
        move |s: &App, i| s.flips.stocked().map_or_else(el, |f| table_row(s, f, &f.rows[i], d)),
    );
    let body = if f.rows.is_empty() {
        col().grow(1.0).center().child(text("No flips match these filters.").font(theme::sans(13.0, 400.0)).color(p().muted))
    } else {
        col().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).scroll_fade(ScrollFade::strength(70.0).full_at(1.0).end_only().linear()).child(rows)
    };
    col().grow(1.0).min_h(0.0).w_full().child(head(s, d)).child(body).role("table").label("Vendor flips")
}

fn head(s: &App, d: Dims) -> El {
    let sort = s.flips.sort;
    let mut out = row().w_full().h(38.0).px(d.pad).gap(d.gap).center_y().fill(p().ground).border_sides([0.0, 0.0, 1.0, 0.0], p().line);
    for (i, column) in Column::ALL.into_iter().enumerate() {
        let sorted = sort.column == column;
        let title = if sorted { format!("{} {}", column.title(), if sort.descending { "↓" } else { "↑" }) } else { column.title().to_string() };
        let label = text(title).font(CAPS).color(if sorted { p().brass } else { p().faint }).text_case(rune::TextCase::Uppercase);
        let cell = cell(d, i).child(label).role("columnheader").cursor(Cursor::Pointer).on_press(move |s: &mut App| {
            s.flips.sort = s.flips.sort.clicked(column);
            s.flips.refilter();
        });
        out = out.child(cell);
    }
    out
}

/// The cell for column `i`: the item takes the leftover width, the vendor sits left, and the
/// numbers are fixed and right-aligned.
fn cell(d: Dims, i: usize) -> El {
    match i {
        0 => flexible().items_start(),
        1 => col().w(widths(d)[0]).shrink().min_w(0.0).items_start(),
        _ => col().w(widths(d)[i - 1]).items_end(),
    }
}

fn table_row(s: &App, f: &Stocked, entry: &Row, d: Dims) -> El {
    let (flip, merchant, item) = f.at(entry.flip);
    let key = f.key(entry.flip);
    let selected = s.flips.selected.is_some_and(|(_, item)| item == flip.item);
    let name = item.map_or_else(|| format!("Item #{}", flip.item), |i| i.name.clone());
    let quality = item.map_or(p().text, |i| p().quality(i.quality));
    let n = Money::new(theme::mono(13.0, 400.0), p().text);
    let number = |v: String| text(v).font(theme::mono(13.0, 400.0)).color(p().text);
    let small = |v: &str| text(v).font(theme::sans(11.5, 400.0)).color(p().muted).mt(2.0).truncate();
    let item_cell = row()
        .gap(12.0)
        .center_y()
        .w_full()
        .child(icon_frame(s.icons.item(item.and_then(|i| i.icon)), 32.0, 1.5, 7.0, quality))
        .child(text(&name).font(theme::sans(14.0, 500.0)).color(quality).truncate().shrink().min_w(0.0));
    let place = if s.flips.search.text.trim().is_empty() || merchant.zone == s.flips.zone { merchant.role.clone() } else { format!("{} · {}", merchant.role, merchant.zone) };
    let mut who = row().w_full().gap(6.0).center_y().child(text(&merchant.name).font(theme::sans(13.0, 500.0)).color(p().text).truncate().shrink().min_w(0.0));
    if !entry.others.is_empty() {
        let more = entry.others.len();
        who = who.child(text(format!("+{more}")).font(theme::mono(11.5, 500.0)).color(p().faint).label(format!("and {more} more {}", if more == 1 { "merchant" } else { "merchants" })));
    }
    let vendor = col().w_full().child(who).child(small(&place));
    let paid = flip.paid.map_or_else(|| unknown("no price"), |paid| col().items_end().child(n.show(paid.copper())).child(small(&paid.origin())));
    let cells = [
        item_cell,
        vendor,
        paid,
        flip.sale.map_or_else(|| unknown("no price"), |sale| n.show(sale.unit)),
        flip.deposit.loss.map_or_else(|| unknown("no rate"), |l| n.show(Copper(l.round() as u64))),
        flip.profit.map_or_else(|| unknown("no price"), |pr| Money::new(theme::mono(13.0, 500.0), p().profit).signed(pr)),
        flip.sale_rate.map_or_else(|| unknown("no data"), |r| number(format!("{:.0}%", r * 100.0))),
        flip.sold_per_day.map_or_else(|| unknown("no data"), |v| number(per_day(v))),
        flip.limit.map_or_else(|| unknown("none"), |l| number(l.to_string())),
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
        .label(format!("{name}, {}", merchant.name))
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
    out.on_press(move |s: &mut App| s.flips.selected = Some(key))
}
