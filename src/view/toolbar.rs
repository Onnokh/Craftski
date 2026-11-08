//! Price source, filters and search.

use rune::{Cursor, El, Key, KeyEvent, Tx, el, row, spacer, text, text_input};

use super::widgets::{SEARCH, Segment, caps, glyph, segmented, toggle};
use super::{App, Dims, SOLD_STEPS};
use crate::profit::PriceSource;
use crate::theme::{self, p};

pub fn toolbar(s: &App, d: Dims) -> El {
    let source = segmented(
        "Price source",
        vec![
            Segment { label: "Realm", on: s.source == PriceSource::Realm, brass: true, pick: Some(|s: &mut App| s.set_source(PriceSource::Realm)) },
            Segment { label: "Region", on: s.source == PriceSource::Region, brass: true, pick: Some(|s: &mut App| s.set_source(PriceSource::Region)) },
        ],
    );
    let unpriced = toggle("Hide unpriced", s.hide_unpriced, |s: &mut App| {
        s.hide_unpriced = !s.hide_unpriced;
        s.refresh();
    });
    let picks: [fn(&mut App); 4] = [|s| s.set_min_sold(0), |s| s.set_min_sold(1), |s| s.set_min_sold(2), |s| s.set_min_sold(3)];
    let mut steps: Vec<Segment<App>> = SOLD_STEPS.iter().zip(picks).enumerate().map(|(i, ((label, _), pick))| Segment { label, on: s.min_sold == i, brass: false, pick: Some(pick) }).collect();
    steps.push(Segment { label: "/ day", on: false, brass: false, pick: None });
    row()
        .w_full()
        .px(d.pad)
        .pt(14.0)
        .pb(15.0)
        .gap(12.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], p().line)
        .child(source)
        .child(unpriced.ml(8.0))
        .child(caps("Sells", p().faint).font(theme::sans(12.0, 500.0).tracking(0.72)).ml(8.0))
        .child(segmented("Minimum sold per day", steps))
        .child(spacer())
        .child(search(s, d))
}

fn search(s: &App, d: Dims) -> El {
    let field = text_input(&s.search, theme::sans(13.0, 400.0))
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
    let mut b = row()
        .gap(8.0)
        .px(13.0)
        .py(9.0)
        .center_y()
        .radius(10.0)
        .fill(p().surface)
        .border(1.0, if s.search.focused { p().line2 } else { p().line })
        .clip_rounded(10.0)
        .cursor(Cursor::Text)
        .child(glyph(&SEARCH, 14.0, p().faint))
        .child(field);
    b = if d.compact { b.grow(1.0).shrink().min_w(120.0).max_w(300.0) } else { b.w(300.0) };
    if !d.compact {
        b = b.child(el().px(6.0).py(2.0).radius(5.0).border(1.0, p().line2).child(text("Ctrl K").font(theme::mono(11.0, 500.0)).color(p().faint)).a11y_hidden());
    }
    b
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
