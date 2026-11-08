//! Small pieces every region draws: money with coins, framed icons, segmented controls, the
//! toggle, labels and buttons.

use std::sync::OnceLock;

use rune::{Color, Cursor, El, Icon, Image, ImageCache, TextStyle, Tx, col, el, icon, row, text};

use crate::charts;
use crate::copper::{Coin, Copper, Lanes};
use crate::rows::per_day;
use crate::theme::{self, p};

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
        Money { font, color, coin: 12.0, lane: 36.0, units: Copper::lanes }
    }

    /// The hero figure's bigger coins and wider lanes.
    pub fn large(self) -> Money {
        Money { coin: 16.0, lane: 56.0, ..self }
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
                Some(digits) => lane.child(text(digits).font(self.font).color(self.color)).child(coin_icon(coin, self.coin).ml(4.0)),
                None => lane,
            });
        }
        out.label(format!("{sign}{amount}"))
    }
}

static COINS: OnceLock<[Image; 3]> = OnceLock::new();

/// The game's 13 px money icons, gold first, decoded once per process.
pub fn load_coins(cache: &ImageCache) -> &'static [Image; 3] {
    let files: [&'static [u8]; 3] = [include_bytes!("../../assets/coins/gold.png"), include_bytes!("../../assets/coins/silver.png"), include_bytes!("../../assets/coins/copper.png")];
    COINS.get_or_init(|| files.map(|bytes| cache.load(bytes)))
}

fn coin_icon(coin: Coin, size: f32) -> El {
    let slot = el().w(size).h(size).a11y_hidden();
    match COINS.get() {
        Some(coins) => slot.pixels(coins[coin as usize].pixels()),
        None => slot,
    }
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
    let b = row().h(40.0).radius(4.0).center_y().justify_center().role("button").label(label).cursor(Cursor::Pointer);
    let (b, color) = match kind {
        ButtonKind::Primary => (b.grow(1.0).px(16.0).fill(p().brass).hover(Color::WHITE.alpha(0.10)).press(Color::BLACK.alpha(0.10)), p().on_brass),
        ButtonKind::Ghost => (b.px(17.0).border(1.0, p().line).hover(Color::WHITE.alpha(0.04)).press(Color::WHITE.alpha(0.07)), p().text),
    };
    b.child(text(label).font(theme::BODY_MEDIUM).color(color)).on_press_tx(act)
}

/// A Rune Icons outline glyph from `assets/icons`, drawn in its element's colour: the files'
/// `stroke="black"` names no hex colour, so rune draws them as a tintable coverage mask.
macro_rules! rune_icon {
    ($file:literal) => {
        Icon { name: concat!("rune-", $file), view: 24.0, body: include_str!(concat!("../../assets/icons/", $file, ".svg")) }
    };
}

pub static SEARCH: Icon = rune_icon!("search");
pub static CHEVRON_DOWN: Icon = rune_icon!("chevron-down");
static ARROW_DOWN: Icon = rune_icon!("arrow-down");
static ARROW_UP: Icon = rune_icon!("arrow-up");

pub fn glyph(i: &'static Icon, size: f32, color: Color) -> El {
    icon(i, size).color(color).a11y_hidden()
}

/// A faint dash for what the data does not know.
pub fn unknown() -> El {
    text("\u{2014}").font(theme::BODY).color(p().faint)
}

/// A column head's title; `descending` is Some on the sorted column, which reads brighter and
/// ends in an arrow the way it sorts.
pub fn column_title(title: &str, descending: Option<bool>) -> El {
    let label = text(title).font(theme::LABEL).color(if descending.is_some() { p().text } else { p().muted });
    match descending {
        Some(down) => row().gap(4.0).center_y().child(label).child(glyph(if down { &ARROW_DOWN } else { &ARROW_UP }, 12.0, p().text)),
        None => label,
    }
}

/// Sales a day as three pips that light at 1, 5 and 20, then the figure; a dash with none.
pub fn sold_per_day(sold: Option<f64>) -> El {
    let Some(sold) = sold.filter(|&s| s > 0.0) else { return unknown() };
    let lit = charts::pips_lit(sold);
    let mut pips = row().h(12.0).gap(2.0).items_end().a11y_hidden();
    for (i, height) in [6.0, 9.0, 12.0].into_iter().enumerate() {
        pips = pips.child(el().w(4.0).h(height).radius(1.0).fill(p().text.alpha(if i < lit { 0.7 } else { 0.12 })));
    }
    row().gap(8.0).center_y().child(pips).child(row().min_w(28.0).justify_end().child(text(per_day(sold)).font(theme::BODY).color(p().muted)))
}

/// A horizontal bar `width` points long, brighter on the selected row; nothing without a width.
pub fn bar(width: Option<f32>, selected: bool) -> El {
    match width {
        Some(w) => el().w(w).h(8.0).radius(2.0).fill(p().text.alpha(if selected { 0.5 } else { 0.3 })).a11y_hidden(),
        None => el(),
    }
}

/// The quiet line under a table that says what its filters hide, and a brass Show all that
/// turns those filters off.
pub fn hidden_hint<S: 'static>(line: String, pad: f32, show_all: impl Fn(&mut S) + 'static) -> El {
    let action = text("Show all")
        .font(theme::LABEL)
        .color(p().brass)
        .key("show-all")
        .role("button")
        .label("Show all")
        .cursor(Cursor::Pointer)
        .on_press(show_all);
    row()
        .key("hidden-hint")
        .w_full()
        .px(pad)
        .py(12.0)
        .gap(8.0)
        .center_y()
        .child(text(line).font(theme::CAPTION).color(p().muted).truncate().shrink().min_w(0.0))
        .child(text("·").font(theme::CAPTION).color(p().faint).a11y_hidden())
        .child(action)
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
