//! The recipe table: a sortable header over virtual rows.

use std::borrow::Cow;

use rune::{Color, Cursor, El, GrowthSpec, ScrollSpec, col, el, row, text};

use super::widgets::{Money, bar, column_title, flexible, hidden_hint, icon_frame, selectable_row, sold_per_day, unknown};
use super::{App, Dims, Ready, Recipes, recipe_at, tooltip};
use crate::charts;
use crate::menu::SOLD_STEPS;
use crate::rows::{self, Column, FilterKind, percent};
use crate::settings;
use crate::theme::{self, p};
use crate::tooltip::Tip;

const ROW: f32 = 32.0;

/// A cell after the recipe's.
#[derive(Clone, Copy)]
enum Slot {
    Skill,
    Profit,
    ProfitBar,
    Margin,
    SoldPerDay,
    Listed,
}

impl Slot {
    /// The column a click on its head sorts by; the bar has none.
    fn column(self) -> Option<Column> {
        match self {
            Slot::Skill => Some(Column::Skill),
            Slot::Profit => Some(Column::Profit),
            Slot::ProfitBar => None,
            Slot::Margin => Some(Column::Margin),
            Slot::SoldPerDay => Some(Column::SoldPerDay),
            Slot::Listed => Some(Column::Auctions),
        }
    }

    /// Its cell, `width` wide: figures sit right, the bar grows from the left.
    fn cell(self, width: f32) -> El {
        let cell = row().w(width).center_y();
        if self.column().is_some() { cell.justify_end() } else { cell }
    }
}

/// The cells after the recipe's, each with its width; the recipe takes the width left. The
/// narrow window keeps every cell, only narrower.
fn slots(d: Dims) -> [(Slot, f32); 6] {
    use Slot::*;
    if d.compact {
        [(Skill, 40.0), (Profit, 104.0), (ProfitBar, 96.0), (Margin, 56.0), (SoldPerDay, 56.0), (Listed, 40.0)]
    } else {
        [(Skill, 56.0), (Profit, 112.0), (ProfitBar, 160.0), (Margin, 72.0), (SoldPerDay, 80.0), (Listed, 64.0)]
    }
}

pub fn table(s: &App, r: &Ready, d: Dims) -> El {
    let top = r.rows.iter().filter_map(|&i| r.evals[i].profit).max().unwrap_or(0);
    let rows = col().key("rows").w_full().growth(GrowthSpec::default()).virtual_rows(
        r.rows.len(),
        ROW,
        |s: &App, i| s.ready().map_or(Cow::Borrowed(""), |r| Cow::Owned(r.market.recipes[r.rows[i]].spell_id.to_string())),
        move |s: &App, i| s.ready().map_or_else(el, |r| table_row(s, r, r.rows[i], d, top)),
    );
    let name = s.profession.name();
    let say = |m: String| col().grow(1.0).center().padding(24.0).child(text(m).font(theme::BODY).color(p().muted).wrap());
    let body = match (&r.recipes, hint(s, r, d)) {
        (Recipes::Loading, _) => say(format!("Loading {name} recipes...")),
        (Recipes::Failed(e), _) => say(format!("Could not load {name} recipes: {e}")),
        (Recipes::Shown, Some(hint)) if r.rows.is_empty() => col().grow(1.0).center().child(hint),
        (Recipes::Shown, None) if r.rows.is_empty() => say("No recipes match these filters.".into()),
        (Recipes::Shown, hint) => col().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(rows).children(hint),
    };
    col().grow(1.0).min_h(0.0).w_full().child(head(s, d)).child(body).role("table").label(format!("{name} recipes"))
}

/// What the filters hide that the search finds, if anything.
fn hint(s: &App, r: &Ready, d: Dims) -> Option<El> {
    let h = r.hidden.as_ref()?;
    let names: Vec<String> = h
        .by
        .iter()
        .map(|k| match k {
            FilterKind::Sells => SOLD_STEPS[s.min_sold].named(),
            FilterKind::HideUnpriced => "Priced only".to_string(),
            FilterKind::Skill => format!("Skill {}", settings::parse_skill(&s.skill.text).unwrap_or_default()),
        })
        .collect();
    let line = rows::hint(h.count, &s.search.text, &names, !r.rows.is_empty());
    Some(hidden_hint(line, d.pad, |s: &mut App| s.show_all()))
}

fn head(s: &App, d: Dims) -> El {
    let title = |column: Column, cell: El| {
        let descending = (s.sort.column == column).then_some(s.sort.descending);
        cell.child(column_title(column.title(), descending)).role("columnheader").cursor(Cursor::Pointer).on_press(move |s: &mut App| {
            s.sort = s.sort.clicked(column);
            s.refresh();
        })
    };
    let mut out = row().w_full().h(ROW).px(d.pad).gap(d.gap).center_y().fill(Color::BLACK.alpha(0.5)).border_sides([0.0, 0.0, 1.0, 0.0], p().bronze).child(title(Column::Item, flexible().items_start()));
    for (slot, width) in slots(d) {
        let cell = slot.cell(width);
        out = out.child(match slot.column() {
            Some(column) => title(column, cell),
            None => cell,
        });
    }
    out
}

/// `top` is the largest profit among the rows shown, which a full bar stands for.
fn table_row(s: &App, r: &Ready, index: usize, d: Dims, top: i64) -> El {
    let (recipe, e) = recipe_at(r, index);
    let selected = s.selected == Some(recipe.spell_id);
    let quality = p().quality(recipe.quality);
    let number = |v: String| text(v).font(theme::BODY).color(p().text);
    let item = row()
        .gap(8.0)
        .center_y()
        .w_full()
        .child(icon_frame(s.icons.item(recipe.icon), 26.0, quality))
        .child(text(&recipe.item_name).font(theme::BODY).color(quality).truncate().shrink().min_w(0.0));
    let mut out = selectable_row(selected)
        .h(ROW)
        .px(d.pad)
        .gap(d.gap)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], p().rule)
        .cursor(Cursor::Pointer)
        .role("row")
        .label(recipe.item_name.clone())
        .checked(selected)
        .child(flexible().items_start().child(item));
    for (slot, width) in slots(d) {
        let content = match slot {
            Slot::Skill => recipe.skill.map_or_else(unknown, |k| number(k.learn.to_string())),
            Slot::Profit => e.profit.map_or_else(unknown, |pr| Money::new(theme::BODY_STRONG, p().text).short().signed(pr)),
            Slot::ProfitBar => bar(e.profit.and_then(|pr| charts::bar(pr, top, width)), selected),
            Slot::Margin => e.margin.map_or_else(unknown, |m| number(percent(m)).color(if m < 0.0 { p().loss } else { p().text })),
            Slot::SoldPerDay => sold_per_day(e.sold_per_day),
            Slot::Listed => e.auctions.map_or_else(unknown, |v| number(v.to_string())),
        };
        out = out.child(slot.cell(width).child(content));
    }
    let id = recipe.spell_id;
    tooltip::hover(out, Tip::Recipe(id)).on_press(move |s: &mut App| s.selected = Some(id))
}
