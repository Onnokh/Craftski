//! The flip table: a sortable header over virtual rows.

use std::borrow::Cow;

use rune::{Color, Cursor, El, GrowthSpec, ScrollSpec, Span, col, el, row, text};

use super::{Stocked, row_key};
use crate::copper::Copper;
use crate::flip_rows::{Column, Row};
use crate::rows;
use crate::theme::{self, p};
use crate::view::widgets::{Money, column_title, flexible, hidden_hint, icon_frame, selectable_row, sold_per_day, unknown};
use crate::view::{App, Dims};

const ROW: f32 = 32.0;

/// The columns shown, each with its width; the item takes the width left. A narrow window drops
/// the sale rate and the limit, which the detail panel still shows, to leave the item its name.
fn columns(d: Dims) -> Vec<(Column, Option<f32>)> {
    let widths: &[(Column, f32)] = if d.compact {
        &[(Column::Vendor, 120.0), (Column::Paid, 96.0), (Column::Sells, 96.0), (Column::DepositLoss, 80.0), (Column::Profit, 96.0), (Column::SoldPerDay, 56.0)]
    } else {
        &[
            (Column::Vendor, 160.0),
            (Column::Paid, 112.0),
            (Column::Sells, 112.0),
            (Column::DepositLoss, 96.0),
            (Column::Profit, 112.0),
            (Column::SaleRate, 64.0),
            (Column::SoldPerDay, 64.0),
            (Column::Limit, 48.0),
        ]
    };
    std::iter::once((Column::Item, None)).chain(widths.iter().map(|&(c, w)| (c, Some(w)))).collect()
}

pub fn table(s: &App, f: &Stocked, d: Dims) -> El {
    let rows = col().key("flip-rows").w_full().growth(GrowthSpec::default()).virtual_rows(
        f.rows.len(),
        ROW,
        |s: &App, i| s.flips.stocked().map_or(Cow::Borrowed(""), |f| Cow::Owned(row_key(f.flips[f.rows[i].flip].item))),
        move |s: &App, i| s.flips.stocked().map_or_else(el, |f| table_row(s, f, &f.rows[i], d)),
    );
    let body = match hint(s, f, d) {
        Some(hint) if f.rows.is_empty() => col().grow(1.0).center().child(hint),
        None if f.rows.is_empty() => col().grow(1.0).center().child(text("No flips match these filters.").font(theme::BODY).color(p().muted)),
        hint => col().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(rows).children(hint),
    };
    col().grow(1.0).min_h(0.0).w_full().px(d.inset).child(head(s, d)).child(body).role("table").label("Vendor flips")
}

/// What the filters hide that the zone or search shows, if anything.
fn hint(s: &App, f: &Stocked, d: Dims) -> Option<El> {
    let h = f.hidden.as_ref()?;
    let names: Vec<String> = h.by.iter().map(|&k| s.flips.filter_name(k)).collect();
    let line = rows::hint(h.count, &s.flips.search.text, &names, !f.rows.is_empty());
    Some(hidden_hint(line, d.cell_pad(), |s: &mut App| s.flips.show_all()))
}

fn head(s: &App, d: Dims) -> El {
    let sort = s.flips.sort;
    let mut out = row().w_full().h(ROW).px(d.cell_pad()).gap(d.gap).center_y().fill(Color::BLACK.alpha(0.5)).border_sides([0.0, 0.0, 1.0, 0.0], p().bronze);
    for (column, width) in columns(d) {
        let descending = (sort.column == column).then_some(sort.descending);
        out = out.child(cell(column, width).child(column_title(column.title(), descending)).role("columnheader").cursor(Cursor::Pointer).on_press(move |s: &mut App| {
            s.flips.sort = s.flips.sort.clicked(column);
            s.flips.refilter();
        }));
    }
    out
}

/// The cell for a column: the item takes the leftover width, the vendor sits left and may
/// truncate, and the numbers are fixed and right-aligned.
fn cell(column: Column, width: Option<f32>) -> El {
    match (column, width) {
        (_, None) => flexible().items_start(),
        (Column::Vendor, Some(w)) => row().w(w).shrink().min_w(0.0).center_y(),
        (_, Some(w)) => row().w(w).justify_end().center_y(),
    }
}

fn table_row(s: &App, f: &Stocked, entry: &Row, d: Dims) -> El {
    let (flip, merchant, item) = f.at(entry.flip);
    let key = f.key(entry.flip);
    let selected = s.flips.selected.is_some_and(|(_, item)| item == flip.item);
    let name = item.map_or_else(|| format!("Item #{}", flip.item), |i| i.name.clone());
    let quality = item.map_or(p().text, |i| p().quality(i.quality));
    let money = Money::new(theme::BODY, p().text).short();
    let number = |v: String| text(v).font(theme::BODY).color(p().text);
    let mut out = selectable_row(selected)
        .h(ROW)
        .px(d.cell_pad())
        .gap(d.gap)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], p().rule)
        .cursor(Cursor::Pointer)
        .role("row")
        .label(format!("{name}, {}", merchant.name))
        .checked(selected);
    for (column, width) in columns(d) {
        let content = match column {
            Column::Item => row()
                .gap(8.0)
                .center_y()
                .w_full()
                .child(icon_frame(s.icons.item(item.and_then(|i| i.icon)), 26.0, quality))
                .child(text(&name).font(theme::BODY).color(quality).truncate().shrink().min_w(0.0)),
            Column::Vendor => vendor(s, merchant, entry.others.len()),
            Column::Paid => flip.paid.map_or_else(unknown, |paid| money.show(paid.copper())),
            Column::Sells => flip.sale.map_or_else(unknown, |sale| money.show(sale.unit)),
            Column::DepositLoss => flip.deposit.loss.map_or_else(unknown, |l| money.show(Copper(l.round() as u64))),
            Column::Profit => flip.profit.map_or_else(unknown, |pr| Money::new(theme::BODY_STRONG, p().text).short().signed(pr)),
            Column::SaleRate => flip.sale_rate.map_or_else(unknown, |r| number(format!("{:.0}%", r * 100.0))),
            Column::SoldPerDay => sold_per_day(flip.sold_per_day),
            Column::Limit => flip.limit.map_or_else(unknown, |l| number(l.to_string())),
        };
        out = out.child(cell(column, width).child(content));
    }
    out.on_press(move |s: &mut App| s.flips.selected = Some(key))
}

/// The merchant, how many others sell the item, and the zone when a search reaches past the
/// chosen one.
fn vendor(s: &App, merchant: &crate::merchants::Merchant, others: usize) -> El {
    let mut line = merchant.name.clone();
    let start = line.len();
    if others > 0 {
        line.push_str(&format!(" +{others}"));
    }
    if merchant.zone != s.flips.zone {
        line.push_str(&format!(", {}", merchant.zone));
    }
    let end = line.len();
    let more = if others == 0 { String::new() } else { format!(", and {others} more {}", if others == 1 { "merchant" } else { "merchants" }) };
    text(line).font(theme::BODY).color(p().text).spans(vec![Span::new(start, end).color(p().muted)]).truncate().shrink().min_w(0.0).label(format!("{}{more}", merchant.name))
}
