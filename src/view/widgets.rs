//! Small pieces every region draws: money with coins, framed icons, toolbar pills, labels and
//! buttons.

use std::sync::OnceLock;

use rune::{Color, Cursor, El, Icon, Image, ImageCache, TextStyle, Tx, col, el, icon, row, text};

use super::skin;
use crate::charts;
use crate::copper::{Coin, Copper, Lanes};
use crate::rows::per_day;
use crate::skin::Piece;
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

/// An item icon in a slot `size` points square: the game's slot frame around a smaller icon, or
/// without the art a 1px frame of its quality colour. A missing or undecoded image leaves the
/// black backing.
pub fn icon_frame(image: Option<&Image>, size: f32, quality: Color) -> El {
    let Some(frame) = skin::stretch(Piece::SlotFrame) else {
        let inner = el().w_full().h_full().radius(3.0);
        let inner = match image {
            Some(i) => inner.pixels(i.pixels()),
            None => inner,
        };
        return el().w(size).h(size).radius(4.0).fill(Color::BLACK).border(1.0, quality.alpha(0.7)).padding(1.0).child(inner).a11y_hidden();
    };
    let icon = (size * 0.85).round();
    let inset = (size - icon) / 2.0;
    let inner = el().absolute(inset, inset).w(icon).h(icon).fill(Color::BLACK);
    let inner = match image {
        Some(i) => inner.pixels(i.pixels()),
        None => inner,
    };
    el().w(size).h(size).child(inner).child(frame).a11y_hidden()
}

/// Between a toolbar's search and its pills.
pub const TOOLBAR_GAP: f32 = 12.0;
/// The room a dropdown keeps on its right for the arrow its art draws, or the chevron without it.
pub const ARROW_ROOM: f32 = 30.0;

/// Between a dropdown's parts.
pub const DROPDOWN_GAP: f32 = 6.0;

/// A dropdown's body, `height` tall with `left` padding: the game's dropdown button, brighter
/// while `open`, or without the art an outline filled while `active` says its filter narrows
/// the rows. The opener adds the room on its right.
pub fn dropdown(height: f32, left: f32, active: bool, open: bool) -> El {
    let out = row().h(height).pl(left).gap(DROPDOWN_GAP).center_y();
    match skin::nine_slice(if open { Piece::DropdownOpen } else { Piece::Dropdown }, [8.0, 8.0, 28.0, 8.0]) {
        Some(layers) => out.children(layers).hover(Color::WHITE.alpha(0.03)),
        None => outline(out, active, open),
    }
}

/// A toolbar dropdown.
pub fn pill(active: bool, open: bool) -> El {
    dropdown(32.0, 12.0, active, open)
}

/// The flat pill: 32pt tall in the line colour, filled with the stronger border while `active`.
/// `open` strengthens the border alone, for a pill whose menu is open or whose field has focus.
fn outline(out: El, active: bool, open: bool) -> El {
    let out = out.radius(4.0).border(1.0, if active || open { p().line2 } else { p().line });
    if active { out.fill(p().active) } else { out.hover(Color::WHITE.alpha(0.03)) }
}

/// A toolbar text field's frame, 32pt tall: the game's search border, or without it the flat
/// pill, filled while `active` says its value narrows the rows.
pub fn field_box(active: bool, focused: bool) -> El {
    let out = row().key("field-box").h(32.0).px(12.0).gap(8.0).center_y().cursor(Cursor::Text);
    match skin::three_slice([Piece::SearchLeft, Piece::SearchMid, Piece::SearchRight], [12.8, 12.8]) {
        Some(layers) => out.children(layers),
        None => outline(out, active, focused),
    }
}

/// A toolbar's search: a magnifier and `field` in the game's search border, 400pt wide while
/// the pills leave room.
pub fn search_box(field: El, focused: bool) -> El {
    let out = row().key("search-box").h(32.0).w(400.0).shrink().gap(8.0).px(12.0).center_y().cursor(Cursor::Text);
    let out = match skin::three_slice([Piece::SearchLeft, Piece::SearchMid, Piece::SearchRight], [12.8, 12.8]) {
        Some(layers) => out.children(layers),
        None => out.radius(4.0).border(1.0, if focused { p().line2 } else { p().line }).clip_rounded(4.0),
    };
    let magnifier = match skin::art(Piece::Magnifier) {
        Some(px) => el().w(16.0).h(16.0).pixels(px).a11y_hidden(),
        None => glyph(&SEARCH, 16.0, p().faint),
    };
    out.child(magnifier).child(field)
}

/// A checkbox and its label; pressing anywhere on it flips the box. The game's box and tick, or
/// without them a brass square in a pill.
pub fn checkbox<S: 'static>(label: &str, on: bool, flip: fn(&mut S)) -> El {
    let out = match (skin::art(Piece::Checkbox), skin::art(Piece::Checkmark)) {
        (Some(bx), Some(tick)) => {
            let square = el().w(22.0).h(22.0).pixels(bx).a11y_hidden();
            let square = if on { square.child(el().absolute(0.0, -1.0).w(22.0).h(22.0).pixels(tick)) } else { square };
            row().h(32.0).gap(8.0).center_y().child(square)
        }
        _ => {
            let square = row().w(16.0).h(16.0).radius(4.0).a11y_hidden();
            let square = if on { square.fill(p().brass).child(glyph(&TICK, 16.0, p().on_brass)) } else { square.border(1.0, p().muted) };
            outline(row().h(32.0).pl(13.0).pr(11.0).gap(8.0).center_y(), false, false).child(square)
        }
    };
    out.child(text(label).font(theme::BODY).color(p().text))
        .key(label)
        .role("checkbox")
        .label(label)
        .checked(on)
        .cursor(Cursor::Pointer)
        .on_press(move |s: &mut S| flip(s))
}

/// The game's red button, `height` tall and `width` wide or taking the width its row leaves,
/// with `leading` before a gold label; without the art, a dark red one.
pub fn red_button<S: 'static>(label: &str, spoken: &str, leading: &'static Icon, width: Option<f32>, height: f32, act: impl Fn(&mut S, &mut Tx) + 'static) -> El {
    let out = row().h(height).px(12.0).gap(if height < 32.0 { 6.0 } else { 8.0 }).center_y().justify_center();
    let out = match width {
        Some(w) => out.w(w),
        None => out.grow(1.0),
    };
    // The art's caps keep their shape at any height; a button too narrow for both shrinks them.
    let caps = [114.0 / 128.0 * height, 292.0 / 128.0 * height];
    let fit = width.map_or(1.0, |w| (w / (caps[0] + caps[1])).min(1.0));
    let out = match skin::three_slice([Piece::RedLeft, Piece::RedMid, Piece::RedRight], caps.map(|c| c * fit)) {
        Some(layers) => out.children(layers),
        None => out.radius(4.0).fill(p().red).border(1.0, p().bronze),
    };
    let (icon, font) = if height < 32.0 { (12.0, theme::LABEL) } else { (16.0, theme::BODY_MEDIUM) };
    out.key(spoken)
        .hover(Color::WHITE.alpha(0.06))
        .press(Color::BLACK.alpha(0.12))
        .child(glyph(leading, icon, p().gold))
        .child(text(label).font(font).color(p().gold))
        .role("button")
        .label(spoken)
        .cursor(Cursor::Pointer)
        .on_press_tx(act)
}

/// A "Wowhead" button that opens `url` in the browser, marked as leaving the app.
pub fn wowhead<S: 'static>(url: String) -> El {
    red_button("Wowhead", "Open on Wowhead", &EXTERNAL, Some(112.0), 36.0, move |_: &mut S, tx: &mut Tx| {
        if let Err(e) = tx.open_url(&url) {
            eprintln!("{url}: {e}");
        }
    })
}

/// A table row's box: hovered faintly, and when `selected` tinted warm under the game's
/// selection glow, or without the art a brass edge. The mark is always the first child, an
/// empty one when unselected, so the cells after it keep their places as the selection moves
/// and each animates as itself.
pub fn selectable_row(selected: bool) -> El {
    let out = row().w_full().hover(Color::WHITE.alpha(0.025));
    let edge = || el().pin(Some(0.0), Some(0.0), None, Some(0.0)).w(2.0);
    if !selected {
        return out.child(edge());
    }
    match skin::stretch(Piece::RowSelected) {
        Some(glow) => out.fill(p().gold.alpha(0.05)).child(glow.dim(0.4)),
        None => out.fill(p().selected).child(edge().fill(p().brass)),
    }
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
pub static CHECK: Icon = rune_icon!("check");
pub static MAP_PIN: Icon = rune_icon!("map-pin");
static ARROW_DOWN: Icon = rune_icon!("arrow-down");
static ARROW_UP: Icon = rune_icon!("arrow-up");
pub static CLIPBOARD_LIST: Icon = rune_icon!("clipboard-list");
pub static SHOPPING_BAG: Icon = rune_icon!("shopping-bag");
pub static TRIANGLE_ALERT: Icon = rune_icon!("triangle-alert");
pub static REFRESH: Icon = rune_icon!("refresh-cw");
static EXTERNAL: Icon = rune_icon!("square-arrow-out-up-right");

pub fn glyph(i: &'static Icon, size: f32, color: Color) -> El {
    icon(i, size).color(color).a11y_hidden()
}

/// A faint dash for what the data does not know.
pub fn unknown() -> El {
    text("\u{2014}").font(theme::BODY).color(p().faint)
}

/// A column head's title in gold; `descending` is Some on the sorted column, which reads white
/// and ends in an arrow the way it sorts.
pub fn column_title(title: &str, descending: Option<bool>) -> El {
    let label = text(title).font(theme::LABEL).color(if descending.is_some() { p().text } else { p().gold });
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
        pips = pips.child(el().w(4.0).h(height).radius(1.0).fill(if i < lit { p().pip } else { Color::WHITE.alpha(0.12) }));
    }
    row().gap(8.0).center_y().child(pips).child(row().min_w(28.0).justify_end().child(text(per_day(sold)).font(theme::BODY).color(p().muted)))
}

/// A gold bar `width` points long in a black ring, brighter on the selected row; nothing without
/// a width.
pub fn bar(width: Option<f32>, selected: bool) -> El {
    let [top, bottom] = if selected { p().bar_selected } else { p().bar };
    match width {
        Some(w) => el().w(w).h(6.0).fill(bottom).linear_gradient(std::f32::consts::PI, [(0.0, top), (1.0, bottom)]).border(1.0, Color::BLACK).a11y_hidden(),
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

/// The checkbox's tick, drawn to its 16pt square.
static TICK: Icon = Icon {
    name: "tsm-check",
    view: 16.0,
    body: r#"<path d="m4.5 8.5 2.5 2.5 4.5-5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>"#,
};
