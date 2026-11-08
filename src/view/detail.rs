//! The selected recipe: profit, where the sale price goes, reagents, skill range and cautions.

use rune::{El, ScrollSpec, Span, Tx, col, el, row, text};

use super::widgets::{ButtonKind, Money, WARNING, button, caps, flexible, glyph, icon_frame, section, unknown};
use super::{App, Dims, Ready, recipe_at};
use crate::breakdown::{Bar, ReagentLine, breakdown, caution, shopping_list, subtitle, waterfall, wowhead_url};
use crate::copper::{Copper, signed};
use crate::profit::{Evaluation, PriceSource};
use crate::recipes::{Recipe, Skill};
use crate::rows::percent;
use crate::theme::{self, p};

const PAD: f32 = 26.0;

pub fn detail(s: &App, r: &Ready, d: Dims) -> El {
    let panel = col().w(d.detail).h_full().fill(p().surface).border_sides([0.0, 0.0, 0.0, 1.0], p().line);
    let Some((_, index)) = s.picked() else {
        return panel.padding(PAD).child(text("Select a recipe to see where its money goes.").font(theme::sans(13.0, 400.0)).color(p().muted).wrap());
    };
    let (recipe, e) = recipe_at(r, index);
    let b = breakdown(recipe, e, &r.market.book, s.source);
    let inner = d.detail - 2.0 * PAD;
    let mut body = col().w_full().gap(20.0).child(title(s, recipe)).child(profit_card(e, s.source)).child(money_flow(e, b.gross, b.cut, inner)).child(reagents(s, &b.reagents));
    if let Some(skill) = recipe.skill {
        body = body.child(skill_range(skill));
    }
    if let Some(warning) = caution(e.sold_per_day, e.auctions) {
        body = body.child(warn(&warning));
    }
    let list = shopping_list(&b.reagents, e.cost);
    let url = wowhead_url(recipe.item_id);
    let actions = row()
        .w_full()
        .gap(10.0)
        .px(PAD)
        .pt(16.0)
        .pb(22.0)
        .child(button("Copy shopping list", ButtonKind::Primary, move |_: &mut App, tx: &mut Tx| tx.copy(list.clone())))
        .child(button("Open on Wowhead", ButtonKind::Ghost, move |_: &mut App, tx: &mut Tx| {
            if let Err(e) = tx.open_url(&url) {
                eprintln!("{url}: {e}");
            }
        }));
    panel
        .child(col().w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(body.px(PAD).pt(PAD)))
        .child(actions)
}

fn title(s: &App, recipe: &Recipe) -> El {
    let quality = p().quality(recipe.quality);
    row()
        .gap(14.0)
        .center_y()
        .child(icon_frame(s.icons.item(recipe.icon), 56.0, 2.0, 11.0, quality))
        .child(
            flexible()
                .child(text(&recipe.item_name).font(theme::display(24.0).lh(26.4)).color(quality).wrap())
                .child(text(subtitle(recipe)).font(theme::sans(12.5, 400.0)).color(p().muted).mt(5.0)),
        )
}

fn profit_card(e: &Evaluation, source: PriceSource) -> El {
    let card = col().w_full().px(19.0).py(17.0).radius(14.0).fill(p().raised).border(1.0, p().line2).child(caps("Profit per craft", p().faint));
    let Some(profit) = e.profit else {
        let why = if e.unpriced.is_empty() { "The crafted item has no price on the auction house." } else { "A reagent has no price, so the cost is unknown." };
        return card.child(text(why).font(theme::sans(13.0, 400.0)).color(p().muted).mt(10.0).wrap());
    };
    let value = Money::new(theme::mono(30.0, 500.0).lh(30.0), p().profit).coin(13.0, 7.0, 11.0).signed(profit);
    let mut line = String::new();
    let mut bold = Vec::new();
    if let Some(m) = e.margin {
        bold.push((0, percent(m).len()));
        line.push_str(&percent(m));
        line.push_str(" on ");
    } else {
        line.push_str("On ");
    }
    if let Some(cost) = e.cost {
        let c = cost.to_string();
        bold.push((line.len(), line.len() + c.len()));
        line.push_str(&c);
    }
    line.push_str(match source {
        PriceSource::Realm => " of mats · realm prices",
        PriceSource::Region => " of mats · region prices",
    });
    let spans: Vec<Span> = bold.into_iter().map(|(a, b)| Span::new(a, b).weight(500.0).color(p().text)).collect();
    card.child(row().mt(10.0).child(value)).child(text(line).font(theme::sans(12.5, 400.0)).color(p().muted).spans(spans).mt(10.0).wrap())
}

/// The waterfall from the sale price down to the profit. `width` is the panel's inner width.
fn money_flow(e: &Evaluation, gross: Option<Copper>, cut: Option<Copper>, width: f32) -> El {
    let out = col().w_full().child(section("Where the money goes"));
    let (Some(gross), Some(cut), Some(cost), Some(profit)) = (gross, cut, e.cost, e.profit) else {
        return out.child(text("Needs prices for the item and every reagent.").font(theme::sans(13.0, 400.0)).color(p().muted));
    };
    let w = waterfall(gross, cut, cost);
    let amount = Money::new(theme::mono(12.5, 400.0), p().text);
    let track = width - 96.0 - 104.0 - 20.0;
    let line = |label: &str, label_color, bar: Bar, color: rune::Color, value: El| {
        row()
            .w_full()
            .h(26.0)
            .gap(10.0)
            .center_y()
            .child(text(label).font(theme::sans(13.0, 400.0)).color(label_color).w(96.0))
            .child(row().w(track).h(8.0).radius(4.0).fill(p().track).child(el().w(bar.start * track)).child(el().w(bar.width * track).h_full().radius(4.0).fill(color)))
            .child(row().w(104.0).justify_end().child(value))
    };
    let minus = |c: Copper| amount.with_sign("\u{2212}", c);
    let (profit_label, profit_color) = if profit < 0 { ("Loss", p().loss) } else { ("Profit", p().profit) };
    out.child(line("Sells for", p().muted, w.sells, p().sells_bar, amount.show(gross)))
        .child(line("AH cut 5%", p().muted, w.cut, p().loss, minus(cut)))
        .child(line("Mats", p().muted, w.mats, p().loss.alpha(0.75), minus(cost)))
        .child(line(profit_label, p().text, w.profit, profit_color, amount.color(profit_color).with_sign(if profit < 0 { "\u{2212}" } else { "" }, Copper(profit.unsigned_abs()))))
        .label(format!("Sells for {gross}, AH cut {cut}, mats {cost}, profit {}", signed(profit)))
}

fn reagents(s: &App, lines: &[ReagentLine]) -> El {
    let mut out = col().w_full().child(section("Reagents"));
    let last = lines.len().saturating_sub(1);
    for (i, l) in lines.iter().enumerate() {
        let each = match l.unit {
            Some(unit) => {
                let mut r = row().mt(2.0).center_y().child(Money::new(theme::sans(12.0, 400.0), p().muted).coin(7.0, 5.0, 7.0).show(unit)).child(text(" each").font(theme::sans(12.0, 400.0)).color(p().muted).ml(3.0));
                if let Some(n) = l.listed {
                    r = r.child(text(format!(" · {n} listed")).font(theme::sans(12.0, 400.0)).color(p().muted));
                }
                r
            }
            None => row().mt(2.0).child(text("No price on the auction house").font(theme::sans(12.0, 400.0)).color(p().loss)),
        };
        let total = l.total.map_or_else(|| unknown("—"), |t| Money::new(theme::mono(13.0, 400.0), p().text).show(t));
        let mut line = row()
            .w_full()
            .gap(12.0)
            .py(10.0)
            .center_y()
            .child(icon_frame(s.icons.item(l.reagent.icon), 34.0, 1.5, 7.0, p().quality(l.reagent.quality)))
            .child(text(format!("{}×", l.reagent.count)).font(theme::mono(13.0, 500.0)).color(p().brass).w(30.0))
            .child(flexible().child(text(&l.reagent.item_name).font(theme::sans(14.0, 500.0)).color(p().text).truncate()).child(each))
            .child(total);
        if i != last {
            line = line.pb(11.0).border_sides([0.0, 0.0, 1.0, 0.0], p().line);
        }
        out = out.child(line);
    }
    out
}

/// Yellow from learning to green, green to grey, then grey: the ranks where a craft still
/// raises the skill.
fn skill_range(skill: Skill) -> El {
    let [still, green, grey] = p().skill;
    let third = |c| el().w_frac(1.0 / 3.0).h_full().fill(c);
    let label = |t: String| text(t).font(theme::mono(11.5, 400.0)).color(p().muted);
    col()
        .w_full()
        .child(section("Skill-up range"))
        .child(row().w_full().h(6.0).radius(3.0).clip_rounded(3.0).child(third(still)).child(third(green)).child(third(grey)))
        .child(
            row()
                .w_full()
                .mt(6.0)
                .justify_between()
                .child(label(format!("{} learn", skill.learn)))
                .child(label(format!("{} green", skill.green())))
                .child(label(format!("{} grey", skill.grey))),
        )
}

fn warn(message: &str) -> El {
    row()
        .w_full()
        .gap(10.0)
        .px(15.0)
        .py(13.0)
        .radius(12.0)
        .fill(p().brass.alpha(0.063))
        .border(1.0, p().brass.alpha(0.2))
        .items_start()
        .child(glyph(&WARNING, 16.0, p().brass).mt(2.0))
        .child(flexible().child(text(message).font(theme::sans(12.5, 400.0).lh(18.1)).color(p().warn_text).wrap()))
        .role("note")
}
