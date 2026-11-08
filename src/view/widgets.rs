//! Small pieces every region draws: money with coins, framed icons, segmented controls, the
//! toggle, labels and buttons.

use rune::{Color, Cursor, El, Icon, Image, Shadow, TextCase, TextStyle, Tx, col, el, icon, row, text};

use crate::copper::{Coin, Copper};
use crate::theme::{self, CAPS, p};

// Rune borders take no layout space, so boxes that CSS sizes around a 1pt border pad 1pt more.

/// How an amount draws: its digits' font and colour and its coins' size and spacing.
#[derive(Clone, Copy)]
pub struct Money {
    pub font: TextStyle,
    pub color: Color,
    pub coin: f32,
    /// Space between digits and their coin, and between a coin and the next digits.
    pub before: f32,
    pub after: f32,
    /// Space between a sign and the digits.
    pub sign_gap: f32,
}

impl Money {
    pub fn new(font: TextStyle, color: Color) -> Money {
        Money { font, color, coin: 9.0, before: 5.0, after: 7.0, sign_gap: 0.0 }
    }

    pub fn coin(self, size: f32, before: f32, after: f32) -> Money {
        Money { coin: size, before, after, ..self }
    }

    pub fn sign_gap(self, sign_gap: f32) -> Money {
        Money { sign_gap, ..self }
    }

    pub fn color(self, color: Color) -> Money {
        Money { color, ..self }
    }

    pub fn show(self, amount: Copper) -> El {
        self.with_sign("", amount)
    }

    /// `sign` drawn tight against the first digits, as in `+3g` or `−25s`.
    pub fn with_sign(self, sign: &str, amount: Copper) -> El {
        let segments = amount.parts().segments();
        let last = segments.len() - 1;
        let mut out = row().center_y();
        let spaced = self.sign_gap > 0.0 && !sign.is_empty();
        if spaced {
            out = out.child(text(sign).font(self.font).color(self.color).mr(self.sign_gap));
        }
        for (i, (digits, coin)) in segments.into_iter().enumerate() {
            let digits = if i == 0 && !spaced { format!("{sign}{digits}") } else { digits };
            let color = if coin == Coin::Copper { self.color.alpha(self.color.a * 0.5) } else { self.color };
            out = out.child(text(digits).font(self.font).color(color)).child(coin_dot(coin, self.coin).ml(self.before).mr(if i == last { 0.0 } else { self.after }));
        }
        out.label(amount.to_string())
    }

    /// A profit or loss: `+` in the profit colour, `−` in the loss colour.
    pub fn signed(self, amount: i64) -> El {
        let (sign, color) = if amount < 0 { ("\u{2212}", p().loss) } else { ("+", p().profit) };
        self.color(color).with_sign(sign, Copper(amount.unsigned_abs()))
    }
}

/// A coin lit from the top left, as the game's money frames draw them.
pub fn coin_dot(coin: Coin, size: f32) -> El {
    let [hi, body, rim] = p().coin(coin);
    // CSS `circle at 35% 30%` reaches the farthest corner: 0.955 of the side.
    el().w(size)
        .h(size)
        .radius(size / 2.0)
        .fill(body)
        .radial_gradient([0.35, 0.30], size * 0.955, [(0.0, hi), (0.55, body), (1.0, rim)])
        .box_shadow(Shadow { spread: 1.0, color: Color::BLACK.alpha(0.33), inset: true, ..Shadow::default() })
        .a11y_hidden()
}

/// An item icon in a frame of its quality colour. A missing or undecoded image leaves the
/// black backing.
pub fn icon_frame(image: Option<&Image>, size: f32, border: f32, radius: f32, quality: Color) -> El {
    let inner = el().w_full().h_full().radius(radius - border);
    let inner = match image {
        Some(i) => inner.pixels(i.pixels()),
        None => inner,
    };
    el().w(size)
        .h(size)
        .radius(radius)
        .fill(Color::BLACK)
        .border(border, quality)
        .box_shadow(Shadow { spread: 1.0, color: Color::BLACK, ..Shadow::default() })
        .padding(border)
        .child(inner)
        .a11y_hidden()
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
    /// Brass instead of plain text when chosen, for the control's primary choice.
    pub brass: bool,
    pub pick: Option<fn(&mut S)>,
}

pub fn segmented<S: 'static>(name: &str, segments: Vec<Segment<S>>) -> El {
    let mut out = row().fill(p().surface).border(1.0, p().line).radius(10.0).padding(4.0).role("radiogroup").label(name);
    for s in segments {
        let color = match (s.on, s.brass) {
            (true, true) => p().brass,
            (true, false) => p().text,
            (false, _) => p().muted,
        };
        let mut item = el().px(14.0).py(6.0).radius(7.0).child(text(s.label).font(theme::sans(13.0, 500.0)).color(color));
        if s.on {
            item = item.fill(p().raised).box_shadow(Shadow { spread: 1.0, color: p().line2, inset: true, ..Shadow::default() });
        }
        out = out.child(match s.pick {
            Some(pick) => item.role("radio").label(s.label).checked(s.on).cursor(Cursor::Pointer).hover(Color::WHITE.alpha(0.03)).on_press(move |st: &mut S| pick(st)),
            None => item.px(10.0).a11y_hidden(),
        });
    }
    out
}

pub fn toggle<S: 'static>(label: &str, on: bool, flip: fn(&mut S)) -> El {
    let knob = el().w(14.0).h(14.0).radius(7.0).fill(if on { p().knob } else { p().muted });
    let track = row().w(32.0).h(18.0).radius(9.0).padding(2.0).center_y().fill(if on { p().brass } else { p().line2 });
    let track = if on { track.justify_end() } else { track };
    row()
        .gap(8.0)
        .center_y()
        .child(track.child(knob))
        .child(text(label).font(theme::sans(13.0, 400.0)).color(p().muted))
        .role("switch")
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

/// Muted text for what the data does not know.
pub fn unknown(label: &str) -> El {
    text(label).font(theme::mono(12.0, 400.0)).color(p().faint)
}

/// A column that takes the leftover width and lets its text truncate.
pub fn flexible() -> El {
    col().grow(1.0).shrink().min_w(0.0)
}
