//! Small pieces every region draws: money with coins, framed icons, segmented controls, the
//! toggle, labels and buttons.

use rune::{Color, Cursor, El, Icon, Image, Shadow, TextCase, TextStyle, Tx, col, el, icon, row, text};

use crate::copper::{Coin, Copper, Lanes};
use crate::theme::{self, CAPS, p};

// Rune borders take no layout space, so boxes that CSS sizes around a 1pt border pad 1pt more.

/// How an amount draws: its digits' font and colour, its coins' size, the width of the silver
/// and copper lanes, and which units it shows.
#[derive(Clone, Copy)]
pub struct Money {
    font: TextStyle,
    color: Color,
    coin: f32,
    lane: f32,
    units: fn(Copper) -> Lanes,
}

impl Money {
    pub fn new(font: TextStyle, color: Color) -> Money {
        Money { font, color, coin: 6.0, lane: 32.0, units: Copper::lanes }
    }

    /// The hero figure's bigger coins and wider lanes.
    pub fn large(self) -> Money {
        Money { coin: 8.0, lane: 52.0, ..self }
    }

    /// The two largest units only, as table cells show them.
    pub fn short(self) -> Money {
        Money { units: Copper::short_lanes, ..self }
    }

    pub fn color(self, color: Color) -> Money {
        Money { color, ..self }
    }

    pub fn show(self, amount: Copper) -> El {
        self.lanes("", amount)
    }

    /// A deduction, drawn with a minus in the usual colour.
    pub fn minus(self, amount: Copper) -> El {
        self.lanes("\u{2212}", amount)
    }

    /// A profit as is, or a loss with a minus in the loss colour.
    pub fn signed(self, amount: i64) -> El {
        if amount < 0 { self.color(p().loss).minus(Copper(amount.unsigned_abs())) } else { self.show(Copper(amount.unsigned_abs())) }
    }

    fn lanes(self, sign: &str, amount: Copper) -> El {
        let mut lanes = (self.units)(amount);
        if let Some(first) = lanes.iter_mut().flatten().next() {
            first.insert_str(0, sign);
        }
        let mut out = row().center_y();
        for (digits, coin) in lanes.into_iter().zip(Coin::ALL) {
            let lane = if coin == Coin::Gold { row() } else { row().w(self.lane) }.justify_end().center_y();
            out = out.child(match digits {
                Some(digits) => lane.child(text(digits).font(self.font).color(self.color)).child(coin_dot(coin, self.coin).ml(4.0)),
                None => lane,
            });
        }
        out.label(format!("{sign}{amount}"))
    }
}

pub fn coin_dot(coin: Coin, size: f32) -> El {
    el().w(size).h(size).radius(size / 2.0).fill(p().coin(coin)).a11y_hidden()
}

/// An item icon in a 1px frame of its quality colour. A missing or undecoded image leaves the
/// black backing.
pub fn icon_frame(image: Option<&Image>, size: f32, quality: Color) -> El {
    let inner = el().w_full().h_full().radius(3.0);
    let inner = match image {
        Some(i) => inner.pixels(i.pixels()),
        None => inner,
    };
    el().w(size).h(size).radius(4.0).fill(Color::BLACK).border(1.0, quality.alpha(0.7)).padding(1.0).child(inner).a11y_hidden()
}

pub fn caps(label: &str, color: Color) -> El {
    text(label).font(CAPS).color(color).text_case(TextCase::Uppercase)
}

pub fn section(label: &str) -> El {
    caps(label, p().faint).mb(10.0)
}

/// One choice of a segmented control.
pub struct Segment<S> {
    pub label: &'static str,
    pub on: bool,
    pub pick: fn(&mut S),
}

pub fn segmented<S: 'static>(name: &str, segments: Vec<Segment<S>>) -> El {
    let mut out = row().h(32.0).padding(1.0).radius(4.0).border(1.0, p().line).clip_rounded(4.0).role("radiogroup").label(name);
    for (i, s) in segments.into_iter().enumerate() {
        if i > 0 {
            out = out.child(el().w(1.0).h_full().fill(p().line));
        }
        let item = row().h_full().px(12.0).center_y().child(text(s.label).font(theme::BODY).color(if s.on { p().text } else { p().muted }));
        let item = if s.on { item.fill(p().active) } else { item.hover(Color::WHITE.alpha(0.03)) };
        let pick = s.pick;
        out = out.child(item.role("radio").label(s.label).checked(s.on).cursor(Cursor::Pointer).on_press(move |st: &mut S| pick(st)));
    }
    out
}

pub fn checkbox<S: 'static>(label: &str, on: bool, flip: fn(&mut S)) -> El {
    let square = row().w(16.0).h(16.0).radius(4.0).a11y_hidden();
    let square = if on { square.fill(p().brass).child(glyph(&CHECK, 16.0, p().on_brass)) } else { square.border(1.0, p().muted) };
    row()
        .gap(8.0)
        .center_y()
        .child(square)
        .child(text(label).font(theme::BODY).color(p().text))
        .role("checkbox")
        .label(label)
        .checked(on)
        .cursor(Cursor::Pointer)
        .on_press(move |s: &mut S| flip(s))
}

pub enum ButtonKind {
    Primary,
    Ghost,
}

pub fn button<S: 'static>(label: &str, kind: ButtonKind, act: impl Fn(&mut S, &mut Tx) + 'static) -> El {
    let b = row().grow(1.0).py(11.0).radius(10.0).justify_center().role("button").label(label).cursor(Cursor::Pointer);
    let (b, color) = match kind {
        ButtonKind::Primary => (b.fill(p().brass).hover(Color::WHITE.alpha(0.10)).press(Color::BLACK.alpha(0.10)), p().on_brass),
        ButtonKind::Ghost => (b.border(1.0, p().line2).hover(Color::WHITE.alpha(0.04)).press(Color::WHITE.alpha(0.07)), p().text),
    };
    b.child(text(label).font(theme::sans(13.0, 600.0)).color(color)).on_press_tx(act)
}

pub static SEARCH: Icon = Icon {
    name: "tsm-search",
    view: 24.0,
    body: r#"<circle cx="11" cy="11" r="7" fill="none" stroke="currentColor" stroke-width="2"/><path d="m20 20-3.5-3.5" fill="none" stroke="currentColor" stroke-width="2"/>"#,
};

pub static WARNING: Icon = Icon {
    name: "tsm-warning",
    view: 24.0,
    body: r#"<path d="M12 9v4m0 4h.01M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub fn glyph(i: &'static Icon, size: f32, color: Color) -> El {
    icon(i, size).color(color).a11y_hidden()
}

/// A faint dash for what the data does not know.
pub fn unknown() -> El {
    text("\u{2014}").font(theme::BODY).color(p().faint)
}

/// A column that takes the leftover width and lets its text truncate.
pub fn flexible() -> El {
    col().grow(1.0).shrink().min_w(0.0)
}

pub static CHECK: Icon = Icon {
    name: "tsm-check",
    view: 16.0,
    body: r#"<path d="m4.5 8.5 2.5 2.5 4.5-5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>"#,
};
