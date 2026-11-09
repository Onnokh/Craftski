//! The hover tooltip, drawn as the game draws GameTooltip beside the cursor: its border and dark
//! blue ground, the name in its quality colour, a craft's reagents with their icons, then the item's
//! own lines.

use std::time::Duration;

use rune::{Align, El, Side, Span, col, el, row, text};

use super::widgets::icon_frame;
use super::{App, Ready, skin};
use crate::recipes::Quality;
use crate::sheets::Tone;
use crate::skin::Piece;
use crate::theme::{self, p};
use crate::tooltip::{Card, Tip};

const WIDTH: f32 = 280.0;
const EDGE: f32 = 7.0;
const FRAME: [Piece; 8] = [Piece::TipTopLeft, Piece::TipTop, Piece::TipTopRight, Piece::TipLeft, Piece::TipRight, Piece::TipBottomLeft, Piece::TipBottom, Piece::TipBottomRight];
const CURSOR: &str = "tooltip-cursor";

/// `el`, opening `tip` while the pointer is over it and keeping the tooltip at the pointer.
pub fn hover(el: El, tip: Tip) -> El {
    el.on_hover_change(move |s: &mut App, inside, _| {
        if inside {
            s.tip = Some(tip);
        } else if s.tip == Some(tip) {
            s.tip = None;
        }
    })
    // A delayed hover never opens under a press, and a finger's pan leaves its press's pause on
    // until a pointer moves, so the rows a finger scrolls past stay shut. The mouse still opens
    // one at once.
    .hover_delays(Duration::from_millis(1), Duration::ZERO)
    .on_pointer_move(|s: &mut App, at| s.pointer = (at.x, at.y))
}

/// The open tooltip beside the pointer, attached at the root: a point at the pointer, then the
/// tooltip anchored to it.
pub fn overlay(s: &App, zoom: f32) -> Vec<El> {
    let Some(card) = s.tip.filter(|_| s.menu.is_none()).and_then(|tip| card(s, s.ready()?, tip)) else { return Vec::new() };
    let (x, y) = (s.pointer.0 / zoom, s.pointer.1 / zoom);
    let cursor = el().key(CURSOR).absolute(x, y).w(1.0).h(1.0).instant_changes().a11y_hidden();
    let panel = col().key("tooltip").instant_changes().w(WIDTH).role("tooltip").label(card.name.clone());
    let panel = match skin::frame(FRAME, EDGE) {
        Some(border) => panel.padding(EDGE + 5.0).child(el().pin(Some(3.0), Some(3.0), Some(3.0), Some(3.0)).fill(p().tooltip)).children(border),
        None => panel.padding(12.0).radius(4.0).fill(p().tooltip).border(1.0, p().bronze),
    };
    vec![cursor, body(s, panel, card).anchored(CURSOR, Side::Right, Align::Start, 18.0)]
}

fn card(s: &App, r: &Ready, tip: Tip) -> Option<Card> {
    let sheets = s.sheets.as_ref();
    match tip {
        Tip::Recipe(spell) => r.market.recipes.iter().find(|x| x.spell_id == spell).map(|recipe| Card::recipe(recipe, sheets)),
        Tip::Reagent(item) => {
            let (r, i) = s.picked()?;
            let reagent = r.market.recipes[i].reagents.iter().find(|x| x.item_id == item)?;
            Some(Card::item(reagent.item_name.clone(), reagent.quality, item, sheets))
        }
        Tip::Flip(index) => s.flips.tip(index, sheets),
    }
}

fn body(s: &App, panel: El, card: Card) -> El {
    let title = text(&card.name).font(theme::BODY_STRONG).color(p().quality(card.quality)).wrap();
    let mats = card.mats.iter().fold(col().w_full().gap(2.0).mt(4.0).mb(6.0), |out, m| {
        let line = format!("{} \u{d7} {}", m.count, m.item_name);
        let count = line.len() - m.item_name.len();
        let name = text(&line).font(theme::MENU).color(p().quality(m.quality)).spans(vec![Span::new(0, count).color(p().text)]).truncate().shrink().min_w(0.0);
        out.child(row().w_full().gap(6.0).center_y().child(icon_frame(s.icons.item(m.icon), 18.0, p().quality(m.quality))).child(name))
    });
    let panel = if card.mats.is_empty() { panel.child(title) } else { panel.child(title).child(mats) };
    card.lines.into_iter().fold(panel, |out, line| {
        let color = match line.tone {
            Tone::Plain => p().text,
            Tone::Gold => p().gold,
            Tone::Green => p().quality(Quality::Uncommon),
            Tone::Red => p().loss,
        };
        let left = text(line.text).font(theme::MENU).color(color).wrap();
        out.child(match line.right {
            Some(right) => row().w_full().gap(12.0).justify_between().child(left.shrink().min_w(0.0)).child(text(right).font(theme::MENU).color(color)),
            None => left,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Cx, Env, Runtime, ScrollSpec, TouchPhase};

    fn rows(_: &App, _: &mut Cx<App>) -> El {
        col().key("list").size(200.0, 300.0).scroll(ScrollSpec { stick_bottom: false })
            .children((0..30).map(|i| hover(el().key(format!("row{i}")).w_full().h(40.0).on_press(|_: &mut App| {}), Tip::Recipe(i))))
    }

    fn rig() -> Runtime<App> {
        let mut rt = Runtime::new(App::new(), rows, (200.0, 300.0), Env::default());
        rt.frame(0.0);
        rt
    }

    #[test]
    fn the_mouse_opens_the_tip_of_the_row_it_rests_on() {
        let mut rt = rig();
        rt.pointer_move(100.0, 100.0);
        rt.frame(0.05);
        assert_eq!(rt.state.tip, Some(Tip::Recipe(2)));
    }

    #[test]
    fn a_finger_scrolling_the_list_opens_no_tip() {
        let mut rt = rig();
        let mut opened = Vec::new();
        rt.touch(1, TouchPhase::Started, 100.0, 250.0, 0.0);
        for i in 1..=20 {
            rt.touch(1, TouchPhase::Moved, 100.0, 250.0 - 10.0 * i as f32, i as f64 * 0.02);
            rt.frame(0.02);
            opened.extend(rt.state.tip);
        }
        rt.touch(1, TouchPhase::Ended, 100.0, 50.0, 0.45);
        for _ in 0..30 {
            rt.frame(1.0 / 60.0);
            opened.extend(rt.state.tip);
        }
        assert_eq!(opened, []);
    }
}
