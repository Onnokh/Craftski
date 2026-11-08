//! The hover tooltip, drawn as the game draws GameTooltip: its border and dark blue ground, the
//! name in its quality colour, the reagents of a craft, then the card's figures.

use rune::{Align, El, Side, Span, col, el, row, text};

use super::{App, Ready, skin};
use crate::skin::Piece;
use crate::theme::{self, p};
use crate::tooltip::{self, Card, Tip, Value};
use crate::view::widgets::Money;

const WIDTH: f32 = 250.0;
const EDGE: f32 = 7.0;
const FRAME: [Piece; 8] = [Piece::TipTopLeft, Piece::TipTop, Piece::TipTopRight, Piece::TipLeft, Piece::TipRight, Piece::TipBottomLeft, Piece::TipBottom, Piece::TipBottomRight];

/// `el`, opening `tip` while the pointer is over it.
pub fn hover(el: El, tip: Tip) -> El {
    el.on_hover_change(move |s: &mut App, inside, _| {
        if inside {
            s.tip = Some(tip);
        } else if s.tip == Some(tip) {
            s.tip = None;
        }
    })
}

/// The open tooltip beside its anchor, attached at the root.
pub fn overlay(s: &App) -> Option<El> {
    let tip = s.tip.filter(|_| s.menu.is_none())?;
    let card = card(s, s.ready()?, tip)?;
    let panel = col().key("tooltip").instant_changes().w(WIDTH).role("tooltip").label(card.name.clone());
    let panel = match skin::frame(FRAME, EDGE) {
        Some(border) => panel.padding(EDGE + 5.0).child(el().pin(Some(3.0), Some(3.0), Some(3.0), Some(3.0)).fill(p().tooltip)).children(border),
        None => panel.padding(12.0).radius(4.0).fill(p().tooltip).border(1.0, p().line2),
    };
    Some(body(panel, card).anchored(tip.anchor(), Side::Right, Align::Start, 12.0))
}

fn card(s: &App, r: &Ready, tip: Tip) -> Option<Card> {
    let book = &r.market.book;
    match tip {
        Tip::Recipe(spell) => {
            let i = r.market.recipes.iter().position(|x| x.spell_id == spell)?;
            Some(tooltip::recipe(&r.market.recipes[i], &r.evals[i], book))
        }
        Tip::Reagent(item) => {
            let (r, i) = s.picked()?;
            let reagent = r.market.recipes[i].reagents.iter().find(|x| x.item_id == item)?;
            Some(tooltip::reagent(reagent, book))
        }
        Tip::Flip(index) => s.flips.tip(index, book),
    }
}

fn body(panel: El, card: Card) -> El {
    let title = text(&card.name).font(theme::BODY_STRONG).color(p().quality(card.quality)).wrap().mb(4.0);
    let mats = card.mats.iter().fold(col().w_full(), |out, m| {
        let line = format!("{} \u{d7} {}", m.count, m.item_name);
        let count = line.len() - m.item_name.len();
        out.child(text(&line).font(theme::CAPTION).color(p().quality(m.quality)).spans(vec![Span::new(0, count).color(p().muted)]).truncate())
    });
    let panel = match card.mats.is_empty() {
        true => panel.child(title),
        false => panel.child(title).child(mats.mb(8.0)),
    };
    card.lines.into_iter().fold(panel, |out, (label, value)| {
        out.child(row().w_full().h(18.0).gap(12.0).justify_between().center_y().child(text(label).font(theme::CAPTION).color(p().text)).child(figure(value)))
    })
}

fn figure(value: Value) -> El {
    let money = Money::new(theme::CAPTION, p().text);
    match value {
        Value::Money(c) => money.show(c),
        Value::Signed(v) => money.color(p().skill[1]).signed(v),
        Value::Percent(v) => text(format!("{:.0}%", v * 100.0)).font(theme::CAPTION).color(p().text),
    }
}
