//! Small pieces every region draws: money with coins, framed icons, toolbar pills, labels and
//! buttons.

use rune::{Color, Cursor, El, Icon, Image, TextStyle, Tx, col, el, icon, row, text};

use super::{Dims, skin};
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

fn coin_icon(coin: Coin, size: f32) -> El {
    let [face, rim] = p().coins[coin as usize];
    el().w(size).h(size).radius(size / 2.0).fill(face).border(1.0, rim).a11y_hidden()
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
/// Between a stacked toolbar's rows.
const TOOLBAR_ROW_GAP: f32 = 8.0;
const TOOLBAR_TOP: f32 = 16.0;
const TOOLBAR_BOTTOM: f32 = 8.0;

/// A toolbar: the search and what leads it, then the filters, each with its width. They share
/// one row while it leaves the lead `lead_min`. Else the lead takes a row of its own over the
/// filters: one row of their usual labels, or of their short ones, or as many rows of their
/// usual labels as they need. `lead` gets the width its row has. Returns the toolbar and its
/// height.
pub fn toolbar(d: Dims, lead_min: f32, lead: impl FnOnce(f32) -> Vec<El>, filters: impl Fn(bool) -> Vec<(El, f32)>) -> (El, f32) {
    let room = d.row_room();
    let span = |f: &[(El, f32)]| f.iter().map(|(_, w)| w).sum::<f32>() + TOOLBAR_GAP * f.len().saturating_sub(1) as f32;
    let long = filters(false);
    if lead_min + 2.0 * TOOLBAR_GAP + span(&long) <= room {
        let height = TOOLBAR_TOP + CONTROL + TOOLBAR_BOTTOM;
        let out = row().w_full().h(height).pt(TOOLBAR_TOP).pb(TOOLBAR_BOTTOM).px(d.pad).gap(TOOLBAR_GAP).center_y();
        let lead = lead(room - span(&long) - 2.0 * TOOLBAR_GAP);
        return (out.children(lead).child(el().grow(1.0)).children(long.into_iter().map(|(e, _)| e)), height);
    }
    let filters = if span(&long) <= room { long } else { Some(filters(true)).filter(|short| span(short) <= room).unwrap_or(long) };
    let mut lines: Vec<(Vec<El>, f32)> = Vec::new();
    for (filter, w) in filters {
        match lines.last_mut() {
            Some((line, used)) if *used + TOOLBAR_GAP + w <= room => {
                line.push(filter);
                *used += TOOLBAR_GAP + w;
            }
            _ => lines.push((vec![filter], w)),
        }
    }
    let height = TOOLBAR_TOP + CONTROL + (TOOLBAR_ROW_GAP + CONTROL) * lines.len() as f32 + TOOLBAR_BOTTOM;
    let lead = lead(room);
    let last = lead.len().saturating_sub(1);
    let top = row().w_full().gap(TOOLBAR_GAP).center_y().children(lead.into_iter().enumerate().map(|(i, e)| if i == last { e.grow(1.0) } else { e }));
    let lines = lines.into_iter().map(|(line, _)| row().gap(TOOLBAR_GAP).center_y().children(line));
    (col().w_full().h(height).pt(TOOLBAR_TOP).pb(TOOLBAR_BOTTOM).px(d.pad).gap(TOOLBAR_ROW_GAP).child(top).children(lines), height)
}

/// The width `words` take in the body font.
pub fn body_width(words: &str) -> f32 {
    rune::text::measure(words, &[], theme::BODY, None).0
}

/// The width of a pill holding `words` in the body font, chevron included.
pub fn pill_width(words: &[&str]) -> f32 {
    12.0 + words.iter().map(|w| body_width(w)).sum::<f32>() + DROPDOWN_GAP * words.len().saturating_sub(1) as f32 + ARROW_ROOM
}

/// The width of a checkbox labelled `label`, with the game's box, the wider of its two.
pub fn checkbox_width(label: &str) -> f32 {
    (CONTROL - 22.0) / 2.0 + 22.0 + 7.0 + body_width(label) + 12.0
}

/// The width of a field box holding `label` and a field `field` wide.
pub fn field_box_width(label: &str, field: f32) -> f32 {
    12.0 + body_width(label) + 8.0 + field + 12.0
}

/// The room a dropdown keeps on its right for its chevron.
pub const ARROW_ROOM: f32 = 30.0;

/// Between a dropdown's parts.
pub const DROPDOWN_GAP: f32 = 6.0;

/// A toolbar control's 32pt height, and the height of a checkbox's frame within it.
const CONTROL: f32 = 32.0;

/// A dropdown's body, `height` tall with `left` padding, in the control frame. The opener adds
/// the chevron on its right.
pub fn dropdown(height: f32, left: f32, active: bool, open: bool) -> El {
    frame(row().h(height).pl(left).gap(DROPDOWN_GAP).center_y(), active, open)
}

/// A toolbar dropdown.
pub fn pill(active: bool, open: bool) -> El {
    dropdown(CONTROL, 12.0, active, open)
}

/// The frame every toolbar control shares: a bronze outline on darkened ground. `open` turns
/// the outline brass, for a control whose menu is open or whose field has focus; `active` fills
/// it, for a filter that hides rows a fresh look would show.
fn frame(out: El, active: bool, open: bool) -> El {
    let out = out.radius(4.0).border(1.0, if open { p().brass } else { p().bronze });
    if active { out.fill(p().active) } else { out.fill(Color::BLACK.alpha(0.45)).hover(Color::WHITE.alpha(0.03)) }
}

/// A toolbar text field's frame, filled while `active` says its value narrows the rows.
pub fn field_box(active: bool, focused: bool) -> El {
    frame(row().key("field-box").h(CONTROL).px(12.0).gap(8.0).center_y().cursor(Cursor::Text), active, focused)
}

/// A toolbar's search: a magnifier and `field`, 400pt wide while the pills leave room.
pub fn search_box(field: El, focused: bool) -> El {
    let out = frame(row().key("search-box").h(CONTROL).w(400.0).shrink().gap(8.0).px(12.0).center_y().cursor(Cursor::Text), false, focused).clip_rounded(4.0);
    let magnifier = match skin::art(Piece::Magnifier) {
        Some(px) => el().w(16.0).h(16.0).pixels(px).a11y_hidden(),
        None => glyph(&SEARCH, 16.0, p().faint),
    };
    out.child(magnifier).child(field)
}

/// A checkbox and its label in the control frame; pressing anywhere on it flips the box. The
/// game's box and tick, or without them a brass square, sits as far from the frame's left as
/// from its top and bottom.
pub fn checkbox<S: 'static>(label: &str, on: bool, flip: impl Fn(&mut S) + 'static) -> El {
    let (square, left, gap) = match (skin::art(Piece::Checkbox), skin::art(Piece::Checkmark)) {
        (Some(bx), Some(tick)) => {
            let square = el().w(22.0).h(22.0).pixels(bx).a11y_hidden();
            // The game draws its tick spilling out of the box, up and to the right. Smaller, and
            // with the tick's bounds centred on the box's, it sits inside instead.
            (if on { square.child(el().absolute(0.8, 4.0).w(18.0).h(18.0).pixels(tick)) } else { square }, (CONTROL - 22.0) / 2.0, 7.0)
        }
        _ => {
            let square = row().w(16.0).h(16.0).radius(4.0).a11y_hidden();
            (if on { square.fill(p().brass).child(glyph(&TICK, 16.0, p().on_brass)) } else { square.border(1.0, p().muted) }, (CONTROL - 16.0) / 2.0, 8.0)
        }
    };
    frame(row().h(CONTROL).pl(left).pr(12.0).gap(gap).center_y(), false, false)
        .child(square)
        .child(text(label).font(theme::BODY).color(p().text))
        .key(label)
        .role("checkbox")
        .label(label)
        .checked(on)
        .cursor(Cursor::Pointer)
        .on_press(move |s: &mut S| flip(s))
}

/// A button in the toolbar controls' bronze frame: `leading` in gold before its label.
pub fn button<S: 'static>(label: &str, spoken: &str, leading: &'static Icon, act: impl Fn(&mut S, &mut Tx) + 'static) -> El {
    frame(row().h(CONTROL).px(12.0).gap(8.0).center_y().justify_center(), false, false)
        .key(spoken)
        .press(Color::BLACK.alpha(0.2))
        .child(glyph(leading, 16.0, p().gold))
        .child(text(label).font(theme::BODY).color(p().text))
        .role("button")
        .label(spoken)
        .cursor(Cursor::Pointer)
        .on_press_tx(act)
}

/// A "Wowhead" button that opens `url` in the browser, marked as leaving the app.
pub fn wowhead<S: 'static>(url: String) -> El {
    link::<S>("Wowhead", "Open on Wowhead", url)
}

/// A button that opens `url` in the browser, marked as leaving the app.
pub fn link<S: 'static>(label: &str, spoken: &str, url: String) -> El {
    button(label, spoken, &EXTERNAL, move |_: &mut S, tx: &mut Tx| {
        if let Err(e) = tx.open_url(&url) {
            eprintln!("{url}: {e}");
        }
    })
}

/// A table row's box: hovered faintly, and when `selected` tinted warm under the game's
/// selection glow, or without the art a brass edge. Every row has the mark, as its first
/// child, and only a selected one shows it: the selection moving fades it in place, where a
/// mark that left the tree would fade out over the row's text, and the cells after it keep
/// their places so each animates as itself.
pub fn selectable_row(selected: bool) -> El {
    let out = row().w_full().hover(Color::WHITE.alpha(0.025));
    let shown = if selected { 1.0 } else { 0.0 };
    match skin::stretch(Piece::RowSelected) {
        Some(glow) => out.fill(p().gold.alpha(0.05 * shown)).child(glow.dim(0.5 * shown)),
        None => out.fill(p().selected.alpha(shown)).child(el().pin(Some(0.0), Some(0.0), None, Some(0.0)).w(2.0).fill(p().brass).dim(shown)),
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

/// An open folder, drawn to the Rune Icons' 24pt grid and stroke.
pub static FOLDER_OPEN: Icon = Icon {
    name: "tsm-folder-open",
    view: 24.0,
    body: r#"<path d="M3 19V5h6l2 2h8v4M3 19l3-8h16l-3 8z" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

/// A cloud with a downward arrow, drawn to the Rune Icons' 24pt grid and stroke.
pub static CLOUD_DOWNLOAD: Icon = Icon {
    name: "tsm-cloud-download",
    view: 24.0,
    body: r#"<path d="M12 13v8M8 17l4 4 4-4M4 14.9A7 7 0 1 1 15.7 8h1.8a4.5 4.5 0 0 1 2.5 8.2" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>"#,
};
