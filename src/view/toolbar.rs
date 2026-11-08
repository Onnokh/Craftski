//! Price source, filters, the priced count and search.

use rune::{Cursor, El, Key, KeyEvent, Tx, row, spacer, text, text_input};

use super::widgets::{SEARCH, Segment, checkbox, glyph, segmented};
use super::{App, Dims, Ready, SOLD_STEPS};
use crate::profit::PriceSource;
use crate::theme::{self, p};

pub fn toolbar(s: &App, r: &Ready, d: Dims) -> El {
    let source = segmented(
        "Price source",
        vec![
            Segment { label: "Realm", on: s.source == PriceSource::Realm, pick: |s: &mut App| s.set_source(PriceSource::Realm) },
            Segment { label: "Region", on: s.source == PriceSource::Region, pick: |s: &mut App| s.set_source(PriceSource::Region) },
        ],
    );
    let unpriced = checkbox("Hide unpriced", s.hide_unpriced, |s: &mut App| {
        s.hide_unpriced = !s.hide_unpriced;
        s.refresh();
    });
    let picks: [fn(&mut App); 4] = [|s| s.set_min_sold(0), |s| s.set_min_sold(1), |s| s.set_min_sold(2), |s| s.set_min_sold(3)];
    let steps = SOLD_STEPS.iter().zip(picks).enumerate().map(|(i, ((label, _), pick))| Segment { label, on: s.min_sold == i, pick }).collect();
    let sold = row().gap(8.0).center_y().child(text("Sold per day").font(theme::BODY).color(p().muted)).child(segmented("Minimum sold per day", steps));
    let mut bar = row().w_full().h(64.0).px(d.pad).gap(d.gap).center_y().child(source).child(unpriced).child(sold).child(spacer());
    if !d.compact {
        let o = &r.overview;
        bar = bar.child(text(format!("{} of {} recipes priced", o.priced, o.total)).font(theme::BODY).color(p().muted));
    }
    bar.child(search(s, d))
}

fn search(s: &App, d: Dims) -> El {
    let field = text_input(&s.search, theme::BODY)
        .key("search")
        .single_line()
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .color(p().text)
        .placeholder(if d.compact { "Search" } else { "Search recipes or reagents" }, p().faint)
        .caret(p().brass, p().brass.alpha(0.3))
        .label("Search recipes or reagents")
        .on_edit(|s: &mut App| &mut s.search)
        .on_input_change(|s: &mut App, _: &mut Tx| s.refresh())
        .on_input_key(|s: &mut App, ev: &KeyEvent, tx: &mut Tx| match ev.key {
            Key::Escape => {
                s.clear_search();
                tx.blur();
                true
            }
            Key::Down => s.step(1, tx),
            Key::Up => s.step(-1, tx),
            _ => false,
        });
    let b = row()
        .h(32.0)
        .gap(8.0)
        .px(13.0)
        .center_y()
        .radius(4.0)
        .border(1.0, if s.search.focused { p().line2 } else { p().line })
        .clip_rounded(4.0)
        .cursor(Cursor::Text)
        .child(glyph(&SEARCH, 16.0, p().faint))
        .child(field);
    if d.compact { b.grow(1.0).shrink().min_w(160.0).max_w(280.0) } else { b.w(280.0) }
}

impl App {
    fn set_source(&mut self, source: PriceSource) {
        self.source = source;
        self.refresh();
    }

    fn set_min_sold(&mut self, step: usize) {
        self.min_sold = step;
        self.refresh();
    }
}
