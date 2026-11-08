//! The selected recipe: profit per craft, where the sale price goes, the market behind it,
//! reagents, where it stands among the others and skill range.

use rune::{Color, Cursor, El, InlineIcon, ScrollSpec, Span, Tx, col, el, row, text};

use super::widgets::{CLIPBOARD_LIST, Money, SHOPPING_BAG, TRIANGLE_ALERT, flexible, glyph, icon_frame, primary, unknown, wowhead};
use super::{App, Dims, Ready, recipe_at};
use crate::breakdown::{Caution, ReagentLine, Sale, breakdown, caution, shopping_list, subtitle, wowhead_url};
use crate::charts::{self, Split, Standing};
use crate::copper::Copper;
use crate::profit::{AuctionBasis, Evaluation, PriceOrigin};
use crate::recipes::{Recipe, Skill};
use crate::rows::{per_day, percent};
use crate::theme::{self, p};

const PAD: f32 = 24.0;
const ROW: f32 = 32.0;
/// The standings' label and rank columns, either side of the strip.
const STANDING_LABEL: f32 = 64.0;
const STANDING_RANK: f32 = 40.0;

pub fn detail(s: &App, r: &Ready, d: Dims) -> El {
    let panel = col().w(d.detail).h_full().fill(p().surface).border_sides([0.0, 0.0, 0.0, 1.0], p().line);
    let Some((_, index)) = s.picked() else {
        return panel.padding(PAD).child(text("Select a recipe to see where its money goes.").font(theme::BODY).color(p().muted).wrap());
    };
    let (recipe, e) = recipe_at(r, index);
    let costs = r.market.costs(s.source);
    let b = breakdown(recipe, e, &r.market.book, costs);
    let split = b.sale.zip(e.cost).and_then(|(sale, cost)| charts::split(sale.gross.0, sale.cut.0, cost.0));
    let warning = caution(e.sold_per_day, e.auctions);
    let money = Money::new(theme::BODY, p().text);
    let sold = e.sold_per_day.map_or_else(|| "none recorded".into(), per_day);
    let listed = e.auctions.map_or_else(unknown, |n| marked(n.to_string(), warning.is_some_and(Caution::on_listings)));
    let mut body = col().w_full().child(identity(s, recipe)).child(hero(e)).child(band("Breakdown").mt(16.0)).children(split.map(stack));
    body = body.child(ledger("Sale price", b.sale.map_or_else(unknown, |s| money.show(s.gross))));
    if let Some(Sale { basis: AuctionBasis::MarketValue, .. }) = b.sale {
        body = body.child(note("No current listings, 14-day average.", false));
    }
    let keyed = |part: Part, value: El| match split {
        Some(sp) => row().gap(12.0).center_y().child(swatch(part, sp.over)).child(value),
        None => value,
    };
    body = body
        .child(ledger("AH cut, 5%", b.sale.map_or_else(unknown, |s| keyed(Part::Cut, money.minus(s.cut)))))
        .child(ledger("Materials", e.cost.map_or_else(unknown, |c| keyed(Part::Materials, money.minus(c)))))
        .child(ledger("Margin on materials", e.margin.map_or_else(unknown, |m| text(percent(m)).font(theme::BODY).color(p().text))))
        .child(band("Market"))
        .child(ledger("Sold per day, region", marked(sold, warning.is_some_and(|w| !w.on_listings()))))
        .child(ledger("Listed on realm", listed));
    if let Some(w) = warning {
        body = body.child(note(w.note(), true));
    }
    body = body.child(band("Reagents"));
    for line in &b.reagents {
        body = body.child(reagent(s, line, money, e.cost));
    }
    body = body.children(standings(r, e, d));
    if let Some(skill) = recipe.skill {
        body = body.child(band("Skill-up")).child(skill_range(skill));
    }
    let list = shopping_list(recipe, &r.market.recipes, costs);
    let actions = row()
        .w_full()
        .gap(8.0)
        .pt(16.0)
        .child(primary("Copy shopping list", &CLIPBOARD_LIST, move |_: &mut App, tx: &mut Tx| tx.copy(list.clone())))
        .child(wowhead::<App>(wowhead_url(recipe.item_id)));
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

/// The profit, keyed to the breakdown bar's profit segment when there is one.
fn hero(e: &Evaluation) -> El {
    let figure = match e.profit {
        None => text("\u{2014}").font(theme::FIGURE).color(p().faint),
        Some(pr) => {
            let money = Money::new(theme::FIGURE, p().text).large().signed(pr);
            if pr > 0 { row().gap(12.0).center_y().child(swatch(Part::Profit, false)).child(money) } else { money }
        }
    };
    row().w_full().h(ROW).mt(20.0).justify_between().center_y().child(text("Profit per craft").font(theme::BODY).color(p().muted)).child(figure)
}

/// A share of the sale price on the breakdown bar.
#[derive(Clone, Copy)]
enum Part {
    Cut,
    Materials,
    Profit,
}

impl Part {
    /// `e` in this part's colour. `over` marks materials that cost more than the cut leaves.
    fn paint(self, e: El, over: bool) -> El {
        match self {
            Part::Cut => e.fill(p().cut).border(1.0, p().cut_edge),
            Part::Materials => e.fill(if over { p().loss } else { p().materials }),
            Part::Profit => e.fill(p().profit),
        }
    }
}

/// The key beside a figure that a breakdown bar segment stands for.
fn swatch(part: Part, over: bool) -> El {
    part.paint(el().w(8.0).h(8.0).radius(2.0).a11y_hidden(), over)
}

/// The sale price as one bar: the cut, the materials, then the profit, each its share. A loss
/// runs the materials to the end in the loss colour.
fn stack(split: Split) -> El {
    let mut bar = row().w_full().h(8.0).gap(1.0).a11y_hidden();
    for (part, share) in [(Part::Cut, split.cut), (Part::Materials, split.materials), (Part::Profit, split.profit)] {
        if share > 0.0 {
            bar = bar.child(part.paint(el().w(0.0).grow(share).h_full().radius(2.0), split.over));
        }
    }
    row().w_full().h(ROW).center_y().border_sides([0.0, 0.0, 1.0, 0.0], p().rule).child(bar)
}

/// A section label over its rows, ruled a step stronger than they are.
pub(super) fn band(label: &str) -> El {
    row().w_full().h(ROW).center_y().border_sides([0.0, 0.0, 1.0, 0.0], p().line).child(text(label).font(theme::LABEL).color(p().muted))
}

pub(super) fn ledger(label: &str, value: El) -> El {
    row()
        .w_full()
        .h(ROW)
        .justify_between()
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], p().rule)
        .child(text(label).font(theme::BODY).color(p().muted))
        .child(value)
}

fn caution_icon() -> El {
    glyph(&TRIANGLE_ALERT, 14.0, p().warn)
}

/// A market figure, followed by the caution icon when the note below is about it.
fn marked(value: String, mark: bool) -> El {
    let figure = text(value).font(theme::BODY).color(p().text);
    if mark { row().gap(6.0).center_y().child(figure).child(caution_icon()) } else { figure }
}

/// A caption line under the rows it explains; `mark` leads it with the caution icon its figure
/// carries.
pub(super) fn note(message: &str, mark: bool) -> El {
    let line = text(message).font(theme::CAPTION).color(p().muted).truncate().shrink();
    let out = row().w_full().h(ROW).gap(8.0).center_y().role("note");
    if mark { out.child(caution_icon()).child(line) } else { out.child(line) }
}

/// "2 × Rugged Leather at 1g 20s", "4 × Silken Thread [bag] 5s 78c" when a vendor sells it, or
/// "…, crafted at" when this profession makes it. The price follows the name so a long name
/// truncates the price first. A crafted reagent's line opens its recipe. `cost` is the whole
/// recipe's, which the share bar measures this reagent against.
fn reagent(s: &App, l: &ReagentLine, money: Money, cost: Option<Copper>) -> El {
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
    let share = l.total.zip(cost).map(|(t, c)| charts::share(t.0, c.0));
    let out = row()
        .w_full()
        .h(ROW)
        .gap(8.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], p().rule)
        .child(icon_frame(s.icons.item(l.reagent.icon), 24.0, p().quality(l.reagent.quality)))
        .child(flexible().child(what))
        .child(share_bar(share))
        .child(l.total.map_or_else(unknown, |t| money.show(t)));
    match recipe {
        Some(spell_id) => out.cursor(Cursor::Pointer).hover(Color::WHITE.alpha(0.025)).label(format!("Show the {} recipe", l.reagent.item_name)).on_press_tx(move |s: &mut App, tx: &mut Tx| {
            s.selected = Some(spell_id);
            tx.scroll_into_view(spell_id.to_string());
        }),
        None => out,
    }
}

/// A reagent's share of the materials cost as a short bar; an empty track when unknown.
fn share_bar(share: Option<f32>) -> El {
    let track = row().w(40.0).h(4.0).mr(4.0).radius(2.0).fill(p().text.alpha(0.07)).a11y_hidden();
    match share.filter(|&f| f > 0.0) {
        Some(f) => track.child(el().w(40.0 * f.min(1.0)).h_full().radius(2.0).fill(p().text.alpha(0.42))),
        None => track,
    }
}

/// Where the recipe's profit and margin stand among every recipe priced under the current
/// source, whatever the filters show, so the rank holds still while searching. None for an
/// unpriced recipe.
fn standings(r: &Ready, e: &Evaluation, d: Dims) -> Option<El> {
    let mine = e.profit? as f64;
    let width = d.detail - 2.0 * PAD - 1.0 - STANDING_LABEL - STANDING_RANK - 16.0;
    let profits: Vec<f64> = r.evals.iter().filter_map(|e| e.profit).map(|v| v as f64).collect();
    let mut out = col().w_full().child(band("Among priced recipes")).child(standing("Profit", charts::standing(&profits, mine, width), profits.len(), width));
    if let Some(m) = e.margin {
        let margins: Vec<f64> = r.evals.iter().filter_map(|e| e.margin).map(f64::from).collect();
        out = out.child(standing("Margin", charts::standing(&margins, f64::from(m), width), margins.len(), width));
    }
    Some(out)
}

/// One measure's strip: a tick per recipe on a rule, this recipe's in brass, then its rank of
/// `of`.
fn standing(label: &str, s: Standing, of: usize, width: f32) -> El {
    let mut strip = el().w(width).h(16.0).a11y_hidden().child(el().absolute(0.0, 8.0).w(width).h(1.0).fill(p().rule));
    for x in s.ticks {
        strip = strip.child(el().absolute(x - 0.5, 2.0).w(1.0).h(12.0).fill(p().text.alpha(0.30)));
    }
    strip = strip.child(el().absolute(s.at - 1.0, 0.0).w(2.0).h(16.0).fill(p().brass));
    let rank = charts::ordinal(s.rank);
    row()
        .w_full()
        .h(ROW)
        .gap(8.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], p().rule)
        .label(format!("{label} ranks {rank} of {of}"))
        .child(row().w(STANDING_LABEL).child(text(label).font(theme::BODY).color(p().muted)))
        .child(strip)
        .child(row().w(STANDING_RANK).justify_end().child(text(rank).font(theme::BODY).color(p().text)))
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
