//! The selected recipe: profit per craft, where the sale price goes, the market behind it,
//! reagents and skill range.

use rune::{El, ScrollSpec, Span, Tx, col, el, row, text};

use super::widgets::{ButtonKind, Money, button, flexible, icon_frame, unknown};
use super::{App, Dims, Ready, recipe_at};
use crate::breakdown::{Caution, ReagentLine, breakdown, caution, shopping_list, subtitle, wowhead_url};
use crate::profit::Evaluation;
use crate::recipes::{Recipe, Skill};
use crate::rows::{per_day, percent};
use crate::theme::{self, p};

const PAD: f32 = 24.0;
const ROW: f32 = 32.0;
const DAGGER: &str = "\u{2020}";

pub fn detail(s: &App, r: &Ready, d: Dims) -> El {
    let panel = col().w(d.detail).h_full().fill(p().surface).border_sides([0.0, 0.0, 0.0, 1.0], p().line);
    let Some((_, index)) = s.picked() else {
        return panel.padding(PAD).child(text("Select a recipe to see where its money goes.").font(theme::BODY).color(p().muted).wrap());
    };
    let (recipe, e) = recipe_at(r, index);
    let b = breakdown(recipe, e, &r.market.book, s.source);
    let warning = caution(e.sold_per_day, e.auctions);
    let money = Money::new(theme::BODY, p().text);
    let sold = e.sold_per_day.map_or_else(|| "none recorded".into(), per_day);
    let listed = e.auctions.map_or_else(unknown, |n| marked(n.to_string(), warning.is_some_and(Caution::on_listings)));
    let mut body = col()
        .w_full()
        .child(identity(s, recipe))
        .child(hero(e))
        .child(band("Breakdown").mt(16.0))
        .child(ledger("Sale price", b.gross.map_or_else(unknown, |c| money.show(c))))
        .child(ledger("AH cut, 5%", b.cut.map_or_else(unknown, |c| money.minus(c))))
        .child(ledger("Materials", e.cost.map_or_else(unknown, |c| money.minus(c))))
        .child(ledger("Margin on materials", e.margin.map_or_else(unknown, |m| text(percent(m)).font(theme::BODY).color(p().text))))
        .child(band("Market"))
        .child(ledger("Sold per day, region", marked(sold, warning.is_some_and(|w| !w.on_listings()))))
        .child(ledger("Listed on realm", listed));
    if let Some(w) = warning {
        body = body.child(note(w.note()));
    }
    body = body.child(band("Reagents"));
    for line in &b.reagents {
        body = body.child(reagent(s, line, money));
    }
    if let Some(skill) = recipe.skill {
        body = body.child(band("Skill-up")).child(skill_range(skill));
    }
    let list = shopping_list(&b.reagents, e.cost);
    let url = wowhead_url(recipe.item_id);
    let actions = row()
        .w_full()
        .gap(8.0)
        .pt(16.0)
        .child(button("Copy shopping list", ButtonKind::Primary, move |_: &mut App, tx: &mut Tx| tx.copy(list.clone())))
        .child(button("Open on Wowhead", ButtonKind::Ghost, move |_: &mut App, tx: &mut Tx| {
            if let Err(e) = tx.open_url(&url) {
                eprintln!("{url}: {e}");
            }
        }));
    panel.pl(PAD + 1.0).pr(PAD).pb(PAD).child(col().w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(body)).child(actions)
}

fn identity(s: &App, recipe: &Recipe) -> El {
    let quality = p().quality(recipe.quality);
    row()
        .w_full()
        .pt(20.0)
        .gap(16.0)
        .child(icon_frame(s.icons.item(recipe.icon), 48.0, quality))
        .child(
            flexible()
                .child(text(&recipe.item_name).font(theme::HEADING).color(quality).truncate())
                .child(text(subtitle(recipe)).font(theme::CAPTION).color(p().muted).mt(4.0).truncate()),
        )
}

fn hero(e: &Evaluation) -> El {
    let figure = e.profit.map_or_else(|| text("\u{2014}").font(theme::FIGURE).color(p().faint), |pr| Money::new(theme::FIGURE, p().text).large().signed(pr));
    row().w_full().h(ROW).mt(20.0).justify_between().center_y().child(text("Profit per craft").font(theme::BODY).color(p().muted)).child(figure)
}

/// A section label over its rows, ruled a step stronger than they are.
fn band(label: &str) -> El {
    row().w_full().h(ROW).center_y().border_sides([0.0, 0.0, 1.0, 0.0], p().line).child(text(label).font(theme::LABEL).color(p().muted))
}

fn ledger(label: &str, value: El) -> El {
    row()
        .w_full()
        .h(ROW)
        .justify_between()
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], p().rule)
        .child(text(label).font(theme::BODY).color(p().muted))
        .child(value)
}

/// A market figure, with a brass dagger when the note below is about it.
fn marked(value: String, mark: bool) -> El {
    if !mark {
        return text(value).font(theme::BODY).color(p().text);
    }
    let start = value.len() + 1;
    text(format!("{value} {DAGGER}")).font(theme::BODY).color(p().text).spans(vec![Span::new(start, start + DAGGER.len()).color(p().brass)])
}

fn note(message: &str) -> El {
    let line = format!("{DAGGER} {message}");
    row().w_full().h(ROW).center_y().role("note").child(text(line).font(theme::CAPTION).color(p().muted).spans(vec![Span::new(0, DAGGER.len()).color(p().brass)]).truncate())
}

fn reagent(s: &App, l: &ReagentLine, money: Money) -> El {
    let name = format!("{} \u{d7} {}", l.reagent.count, l.reagent.item_name);
    let at = match l.unit {
        Some(unit) => format!(" at {unit}"),
        None => " unpriced".into(),
    };
    let line = format!("{name}{at}");
    let end = line.len();
    let what = text(line).font(theme::BODY).color(p().text).spans(vec![Span::new(name.len(), end).color(p().muted)]).truncate();
    row()
        .w_full()
        .h(ROW)
        .gap(8.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], p().rule)
        .child(icon_frame(s.icons.item(l.reagent.icon), 24.0, p().quality(l.reagent.quality)))
        .child(flexible().child(what))
        .child(l.total.map_or_else(unknown, |t| money.show(t)))
}

/// Yellow from learning to green, green to grey, then grey: the ranks where a craft still
/// raises the skill.
fn skill_range(skill: Skill) -> El {
    let [yellow, green, grey] = p().skill;
    let third = |color, label: String| col().w(0.0).grow(1.0).child(el().w_full().h(4.0).radius(2.0).fill(color)).child(text(label).font(theme::CAPTION).color(p().muted).mt(8.0));
    row()
        .w_full()
        .h(ROW)
        .gap(2.0)
        .center_y()
        .child(third(yellow, format!("Yellow {}", skill.learn)))
        .child(third(green, format!("Green {}", skill.green())))
        .child(third(grey, format!("Grey {}", skill.grey)))
}
