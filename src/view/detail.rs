//! The selected recipe: its market at a glance, then one craft as a ledger from the sale price
//! down to the profit, and the skill range.

use rune::{Align, Color, Cursor, El, InlineIcon, ScrollSpec, Shadow, Span, Tx, col, el, row, text};

use super::skin;
use super::widgets::{CLIPBOARD_LIST, Money, SHOPPING_BAG, TRIANGLE_ALERT, flexible, glyph, icon_frame, red_button, unknown, wowhead};
use super::{App, Dims, Ready, recipe_at, tooltip};
use crate::breakdown::{Caution, ReagentLine, Sale, breakdown, caution, shopping_list, subtitle, wowhead_url};
use crate::charts;
use crate::profit::{AuctionBasis, Evaluation, PriceOrigin};
use crate::recipes::{Recipe, Skill};
use crate::rows::{per_day, percent};
use crate::skin::Piece;
use crate::theme::{self, p};
use crate::tooltip::Tip;

const PAD: f32 = 24.0;
const ROW: f32 = 32.0;

pub fn detail(s: &App, r: &Ready, d: Dims) -> El {
    let panel = panel(d).children(skin::cover(Piece::BackgroundCard(s.profession), (Align::End, Align::End)));
    let Some((_, index)) = s.picked() else {
        return panel.padding(PAD).child(text("Select a recipe to see where its money goes.").font(theme::BODY).color(p().muted).wrap());
    };
    let (recipe, e) = recipe_at(r, index);
    let costs = r.market.costs();
    let b = breakdown(recipe, e, &r.market.book, costs);
    let warning = caution(e.sold_per_day, e.auctions);
    let money = Money::new(theme::BODY, p().text);
    let mut body = col().w_full().child(identity(s, recipe)).child(facts(r, e, warning)).child(band("One craft"));
    body = body.child(ledger_line(text("Sale price").font(theme::BODY).color(p().text), b.sale.map_or_else(unknown, |s| money.show(s.gross))));
    if let Some(Sale { basis: AuctionBasis::MarketValue, .. }) = b.sale {
        body = body.child(note("No current listings, 14-day average.", false));
    }
    body = body.child(ledger("AH cut, 5%", b.sale.map_or_else(unknown, |s| money.minus(s.cut))));
    for line in &b.reagents {
        body = body.child(reagent(s, line, money));
    }
    body = body.child(total(e));
    if let Some(w) = warning {
        body = body.child(note(w.note(), true));
    }
    if let Some(skill) = recipe.skill {
        body = body.child(band("Skill-up")).child(skill_range(skill));
    }
    let list = shopping_list(recipe, &r.market.recipes, costs);
    let actions = row()
        .w_full()
        .gap(8.0)
        .pt(16.0)
        .child(red_button("Copy shopping list", "Copy shopping list", &CLIPBOARD_LIST, None, 36.0, move |_: &mut App, tx: &mut Tx| tx.copy(list.clone())))
        .child(wowhead::<App>(wowhead_url(recipe.item_id)));
    panel.px(PAD).pb(PAD).child(col().key("detail-body").instant_changes().w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(body)).child(actions.key("actions"))
}

fn identity(s: &App, recipe: &Recipe) -> El {
    let quality = p().quality(recipe.quality);
    row()
        .w_full()
        .pt(20.0)
        .gap(12.0)
        .center_y()
        .child(icon_frame(s.icons.item(recipe.icon), 52.0, quality))
        .child(
            flexible()
                .child(text(&recipe.item_name).font(theme::HEADING).color(quality).truncate())
                .child(text(subtitle(recipe)).font(theme::CAPTION).color(p().muted).mt(4.0).truncate()),
        )
}

/// The market in three cells: region sales, realm listings and the profit rank among every
/// recipe priced under the current source, whatever the filters show. A figure the caution
/// below is about turns amber.
fn facts(r: &Ready, e: &Evaluation, warning: Option<Caution>) -> El {
    let sold = e.sold_per_day.map_or_else(|| "None".into(), per_day);
    let listed = e.auctions.map_or_else(|| "\u{2014}".into(), |n| n.to_string());
    let rank = e.profit.map_or_else(
        || "\u{2014}".into(),
        |mine| {
            let profits: Vec<f64> = r.evals.iter().filter_map(|e| e.profit).map(|v| v as f64).collect();
            format!("{} of {}", charts::ordinal(charts::rank(&profits, mine as f64)), profits.len())
        },
    );
    let cells = [
        ("Sold per day", sold, 1.0, warning.is_some_and(|w| !w.on_listings())),
        ("Listed here", listed, 0.85, warning.is_some_and(Caution::on_listings)),
        ("Profit rank", rank, 1.15, false),
    ];
    let mut out = row().w_full().mt(16.0).fill(Color::BLACK.alpha(0.4)).border(1.0, Color::BLACK);
    for (i, (label, value, share, flagged)) in cells.into_iter().enumerate() {
        let cell = col()
            .w(0.0)
            .grow(share)
            .px(10.0)
            .py(8.0)
            .gap(2.0)
            .label(format!("{label}: {value}"))
            .child(text(label).font(theme::CAPTION).color(p().muted))
            .child(text(value).font(theme::BODY_MEDIUM).color(if flagged { p().warn } else { p().text }).truncate());
        out = out.child(if i == 0 { cell } else { cell.border_sides([0.0, 0.0, 0.0, 1.0], p().bronze.alpha(0.5)) });
    }
    out
}

/// The sum under the ledger: a double rule, the profit, and the materials it was made on with
/// the margin they earn.
fn total(e: &Evaluation) -> El {
    let figure = match e.profit {
        None => text("\u{2014}").font(theme::HEADING).color(p().faint),
        Some(pr) => Money::new(theme::HEADING, p().text).large().signed(pr),
    };
    let rule = || el().w_full().h(1.0).fill(p().bronze);
    let mut out = col()
        .w_full()
        .child(col().w_full().gap(2.0).a11y_hidden().child(rule()).child(rule()))
        .child(row().w_full().h(44.0).justify_between().center_y().child(text("Profit").font(theme::BODY_MEDIUM).color(p().gold)).child(figure));
    if let (Some(cost), Some(margin)) = (e.cost, e.margin) {
        out = out.child(
            row()
                .w_full()
                .h(20.0)
                .mt(-6.0)
                .justify_between()
                .center_y()
                .child(text(format!("On {cost} of materials")).font(theme::CAPTION).color(p().muted))
                .child(text(format!("{} margin", percent(margin))).font(theme::CAPTION).color(p().muted)),
        );
    }
    out
}

/// The detail panel's box: a drawer over the table, edged in bronze and casting its shadow
/// onto the rows beside it. The edge takes room, so the body and the art lie inside it.
pub(super) fn panel(d: Dims) -> El {
    col()
        .w(d.detail)
        .h_full()
        .fill(p().surface)
        .border_sides([0.0, 0.0, 0.0, 2.0], p().bronze)
        .border_layout()
        .box_shadow(Shadow { x: -8.0, y: 0.0, blur: 40.0, spread: 0.0, color: Color::BLACK.alpha(0.9), inset: false })
}

/// A gold section label over its rows, ruled in bronze.
pub(super) fn band(label: &str) -> El {
    row().w_full().h(ROW).mt(12.0).center_y().border_sides([0.0, 0.0, 1.0, 0.0], p().bronze).child(text(label).font(theme::LABEL).color(p().gold))
}

pub(super) fn ledger(label: &str, value: El) -> El {
    ledger_line(text(label).font(theme::BODY).color(p().muted), value)
}

/// A ruled ledger row: `what` on the left, its amount on the right.
fn ledger_line(what: El, value: El) -> El {
    row().w_full().h(ROW).gap(8.0).justify_between().center_y().border_sides([0.0, 0.0, 1.0, 0.0], p().rule).child(what).child(value)
}

fn caution_icon() -> El {
    glyph(&TRIANGLE_ALERT, 14.0, p().warn)
}

/// A caption line under the rows it explains; `mark` leads it with the caution icon.
pub(super) fn note(message: &str, mark: bool) -> El {
    let line = text(message).font(theme::CAPTION).color(p().muted).truncate().shrink();
    let out = row().w_full().h(ROW).gap(8.0).center_y().role("note");
    if mark { out.child(caution_icon()).child(line) } else { out.child(line) }
}

/// A reagent as a ledger deduction: "2 × Rugged Leather at 1g 20s", "4 × Silken Thread [bag]
/// 5s 78c" when a vendor sells it, or "…, crafted at" when this profession makes it, then its
/// total. The price follows the name so a long name truncates the price first. A crafted
/// reagent's line opens its recipe.
fn reagent(s: &App, l: &ReagentLine, money: Money) -> El {
    let name = format!("{} \u{d7} {}", l.reagent.count, l.reagent.item_name);
    let mut vendor = false;
    let mut recipe = None;
    let line = match l.price {
        None => format!("{name} unpriced"),
        Some(price) => match price.origin {
            PriceOrigin::Vendor => {
                vendor = true;
                format!("{name} {}", price.unit)
            }
            PriceOrigin::Crafted { spell_id } => {
                recipe = Some(spell_id);
                format!("{name}, crafted at {}", price.unit)
            }
            _ => format!("{name} at {}", price.unit),
        },
    };
    let spans = match recipe {
        Some(_) => {
            let word = name.len() + 2..name.len() + ", crafted".len();
            vec![Span::new(name.len(), word.start).color(p().muted), Span::new(word.start, word.end).color(p().brass), Span::new(word.end, line.len()).color(p().muted)]
        }
        None => vec![Span::new(name.len(), line.len()).color(p().muted)],
    };
    let mut what = text(line).font(theme::BODY).color(p().text).spans(spans).truncate();
    if vendor {
        what = what.inline_icon(name.len() + 1, InlineIcon::glyph(&SHOPPING_BAG).size(12.0).gap(4.0).color(p().muted));
    }
    let out = row()
        .w_full()
        .h(ROW)
        .gap(8.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], p().rule)
        .child(icon_frame(s.icons.item(l.reagent.icon), 22.0, p().quality(l.reagent.quality)))
        .child(flexible().child(what))
        .child(l.total.map_or_else(unknown, |t| money.minus(t)));
    let out = tooltip::hover(out, Tip::Reagent(l.reagent.item_id));
    match recipe {
        Some(spell_id) => out.cursor(Cursor::Pointer).hover(Color::WHITE.alpha(0.025)).label(format!("Show the {} recipe", l.reagent.item_name)).on_press_tx(move |s: &mut App, tx: &mut Tx| {
            s.selected = Some(spell_id);
            tx.scroll_into_view(spell_id.to_string());
        }),
        None => out,
    }
}

/// Yellow from learning to green, green to grey, then grey: the ranks where a craft still
/// raises the skill.
fn skill_range(skill: Skill) -> El {
    let labels = [format!("Yellow {}", skill.learn), format!("Green {}", skill.green()), format!("Grey {}", skill.grey)];
    let (Some(track), Some(frame)) = (skin::stretch(Piece::ProgressBg), skin::stretch(Piece::ProgressFrame)) else { return flat_skill_range(labels) };
    let segments = p().skill_bar.into_iter().fold(row().pin(Some(5.0), Some(5.0), Some(5.0), Some(5.0)).gap(2.0), |out, [top, bottom]| {
        out.child(el().w(0.0).grow(1.0).h_full().fill(bottom).linear_gradient(std::f32::consts::PI, [(0.0, top), (1.0, bottom)]))
    });
    let bar = el().w_full().h(22.0).mt(12.0).a11y_hidden().child(track).child(segments).child(frame);
    let labels = labels.into_iter().fold(row().w_full().mt(6.0).justify_between(), |out, l| out.child(text(l).font(theme::CAPTION).color(p().muted)));
    col().w_full().child(bar).child(labels)
}

/// The skill range as three thin bars, each over its label, where the game's bar is missing.
fn flat_skill_range([yellow_at, green_at, grey_at]: [String; 3]) -> El {
    let [yellow, green, grey] = p().skill;
    let third = |color, label: String| col().w(0.0).grow(1.0).child(el().w_full().h(4.0).radius(2.0).fill(color)).child(text(label).font(theme::CAPTION).color(p().muted).mt(8.0));
    row()
        .w_full()
        .h(ROW)
        .gap(2.0)
        .center_y()
        .child(third(yellow, yellow_at))
        .child(third(green, green_at))
        .child(third(grey, grey_at))
}
