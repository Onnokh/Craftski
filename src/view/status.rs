//! The strip under a tab's table: the game, the realm prices come from and the region's sales
//! feed with their ages, when the app read TSM's file, and a Reload.

use std::time::UNIX_EPOCH;

use rune::time::LocalTime;
use rune::{Color, Cursor, El, Span, el, rich, row, text};

use super::widgets::{REFRESH, glyph};
use super::{App, Dims, Ready};
use crate::market;
use crate::overview::{Feed, Freshness, read_at};
use crate::theme::{self, p};

const GAME: &str = "WoW Forever";
const PRICES: &str = "Prices";
const RELOAD: &str = "Reload";
const RELOAD_ICON: f32 = 12.0;
/// The width a failed reload's message keeps before the region feed goes.
const NOTE_KEPT: f32 = 200.0;
const DOT: f32 = 6.0;
/// Between a dot and its feed's words.
const DOT_GAP: f32 = 6.0;
/// Between "Prices" and the realm it names.
const LABEL_GAP: f32 = 8.0;

/// What the bar shows at a width: the region feed or not, and the read time or not. The read
/// time goes first, then the region feed; the last fit truncates the realm when even it
/// overflows. A failed reload's message takes the read time's place and stays, truncated.
const FITS: [(bool, bool); 3] = [(true, true), (true, false), (false, false)];

pub fn status(s: &App, r: &Ready, d: Dims) -> El {
    let read = r.market.read.at.duration_since(UNIX_EPOCH).map_or(0, |t| t.as_millis() as i64);
    let read = LocalTime::at(read);
    let read = read_at(read.hour, read.minute);
    let error = r.reload_error.as_ref().map(|e| format!("Reload failed: {e}"));
    let (realm, region) = (Feed::realm(&r.market.book, s.now), Feed::region(&r.market.book, s.now));
    let room = d.status.unwrap_or(f32::INFINITY) - 2.0 * d.pad;
    let fits = FITS.map(|(with_region, with_read)| (with_region.then_some(&region), error.as_deref().or(with_read.then_some(read.as_str()))));
    let fit = fits.iter().find(|(region, note)| needed(&realm, *region, *note, d.gap) <= room);
    let (region, note) = fit.unwrap_or(&fits[2]);

    let mut bar = row()
        .key("status")
        .w_full()
        .h(32.0)
        .pt(1.0)
        .px(d.pad)
        .gap(d.gap)
        .center_y()
        .fill(p().rail)
        .border_sides([1.0, 0.0, 0.0, 0.0], p().line)
        .child(caption(GAME, p().text))
        .child(el().w(1.0).h(16.0).fill(p().line).a11y_hidden())
        .child(row().gap(LABEL_GAP).center_y().child(caption(PRICES, p().faint)).child(feed(&realm, p().text, fit.is_none())));
    if let Some(region) = region {
        bar = bar.child(feed(region, p().muted, false));
    }
    bar = bar.child(el().grow(1.0));
    if let Some(note) = note {
        bar = bar.child(if error.is_some() { caption(note, p().loss).truncate().shrink().min_w(0.0) } else { caption(note, p().faint) });
    }
    bar.child(reload())
}

/// A dot and the feed's words, which truncate only when `squeeze` says nothing else fits.
fn feed(f: &Feed, color: Color, squeeze: bool) -> El {
    let line = f.line();
    let words = match &f.age {
        // Past fresh, the age words take the dot's colour, so the warning reads without the dot.
        Some((ago, fresh)) if *fresh != Freshness::Fresh => rich(line.clone(), vec![Span::new(line.len() - ago.len(), line.len()).color(tint(*fresh))]),
        _ => text(line.clone()),
    };
    let words = words.font(theme::CAPTION).color(color).truncate();
    let out = row().gap(DOT_GAP).center_y().label(line);
    let (words, out) = if squeeze { (words.shrink().min_w(0.0), out.shrink().min_w(0.0)) } else { (words, out) };
    let out = match f.age {
        Some((_, fresh)) => out.child(el().w(DOT).h(DOT).radius(DOT / 2.0).fill(tint(fresh)).a11y_hidden()),
        None => out,
    };
    out.child(words)
}

fn reload() -> El {
    row()
        .key("reload")
        .gap(DOT_GAP)
        .center_y()
        .child(glyph(&REFRESH, RELOAD_ICON, p().text))
        .child(caption(RELOAD, p().text))
        .role("button")
        .label("Reload TSM prices")
        .cursor(Cursor::Pointer)
        .on_press(|s: &mut App| s.reload(&market::appdata_path()))
}

fn tint(f: Freshness) -> Color {
    match f {
        Freshness::Fresh => p().muted,
        Freshness::Aging => p().warn,
        Freshness::Stale => p().loss,
    }
}

fn caption(words: &str, color: Color) -> El {
    text(words).font(theme::CAPTION).color(color)
}

/// The width the bar's parts take at their natural size, with `gap` between each two. The
/// flexible space before the note counts as a part of no width; a long error note counts only
/// the part it keeps when truncated.
fn needed(realm: &Feed, region: Option<&Feed>, note: Option<&str>, gap: f32) -> f32 {
    let w = |words: &str| rune::text::measure(words, &[], theme::CAPTION, None).0;
    let feed = |f: &Feed| w(&f.line()) + if f.age.is_some() { DOT + DOT_GAP } else { 0.0 };
    let mut parts = vec![w(GAME), 1.0, w(PRICES) + LABEL_GAP + feed(realm)];
    parts.extend(region.map(feed));
    parts.push(0.0);
    parts.extend(note.map(|n| w(n).min(NOTE_KEPT)));
    parts.push(RELOAD_ICON + DOT_GAP + w(RELOAD));
    parts.iter().sum::<f32>() + gap * (parts.len() - 1) as f32
}
