//! The strip under a tab's table: the game, each price feed with its age, when the app read
//! TSM's file, and a Reload.

use std::time::UNIX_EPOCH;

use rune::time::LocalTime;
use rune::{Color, Cursor, El, Span, el, rich, row, text};

use super::widgets::{REFRESH, glyph};
use super::{App, Dims, Ready};
use crate::market;
use crate::overview::{Feed, Freshness, read_at};
use crate::theme::{self, p};

const GAME: &str = "WoW Forever";
const RELOAD: &str = "Reload";
const RELOAD_ICON: f32 = 12.0;
/// The width a failed reload's message keeps before the feeds give up their kind words.
const NOTE_KEPT: f32 = 200.0;
const DOT: f32 = 6.0;
/// Between a dot and its feed's words.
const DOT_GAP: f32 = 6.0;

/// What the bar shows at a width: the feeds' names short or whole, and the read time or not.
/// The read time goes first, then the feeds' kind words; the last fit truncates the feeds when
/// even it overflows. A failed reload's message takes the read time's place and stays,
/// truncated.
const FITS: [(bool, bool); 3] = [(false, true), (false, false), (true, false)];

pub fn status(s: &App, r: &Ready, d: Dims) -> El {
    let read = r.market.read.at.duration_since(UNIX_EPOCH).map_or(0, |t| t.as_millis() as i64);
    let read = LocalTime::at(read);
    let read = read_at(read.hour, read.minute);
    let error = r.reload_error.as_ref().map(|e| format!("Reload failed: {e}"));
    let room = d.status.unwrap_or(f32::INFINITY) - 2.0 * d.pad;
    let fits = FITS.map(|(short, with_read)| (Feed::both(&r.market.book, s.now, short), error.as_deref().or(with_read.then_some(read.as_str()))));
    let fit = fits.iter().find(|(feeds, note)| needed(feeds, *note, d.gap) <= room);
    let (feeds, note) = fit.unwrap_or(&fits[2]);

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
        .child(el().w(1.0).h(16.0).fill(p().line).a11y_hidden());
    for f in feeds {
        bar = bar.child(feed(f, fit.is_none()));
    }
    bar = bar.child(el().grow(1.0));
    if let Some(note) = note {
        bar = bar.child(if error.is_some() { caption(note, p().loss).truncate().shrink().min_w(0.0) } else { caption(note, p().faint) });
    }
    bar.child(reload())
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

/// A dot and the feed's words, which truncate only when `squeeze` says nothing else fits.
fn feed(f: &Feed, squeeze: bool) -> El {
    let line = f.line();
    let words = match &f.age {
        // Past fresh, the age words take the dot's colour, so the warning reads without the dot.
        Some((ago, fresh)) if *fresh != Freshness::Fresh => rich(line.clone(), vec![Span::new(line.len() - ago.len(), line.len()).color(tint(*fresh))]),
        _ => text(line),
    };
    let words = words.font(theme::CAPTION).color(p().muted).truncate();
    let out = row().gap(DOT_GAP).center_y();
    let (words, out) = if squeeze { (words.shrink().min_w(0.0), out.shrink().min_w(0.0)) } else { (words, out) };
    match f.age {
        Some((_, fresh)) => out.child(el().w(DOT).h(DOT).radius(DOT / 2.0).fill(tint(fresh)).a11y_hidden()).child(words),
        None => out.child(words),
    }
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
/// flexible space between the feeds and the note counts as a part of no width; a long error
/// note counts only the part it keeps when truncated.
fn needed(feeds: &[Feed; 2], note: Option<&str>, gap: f32) -> f32 {
    let w = |words: &str| rune::text::measure(words, &[], theme::CAPTION, None).0;
    let feed = |f: &Feed| w(&f.line()) + if f.age.is_some() { DOT + DOT_GAP } else { 0.0 };
    let mut parts = vec![w(GAME), 1.0, feed(&feeds[0]), feed(&feeds[1]), 0.0];
    parts.extend(note.map(|n| w(n).min(NOTE_KEPT)));
    parts.push(RELOAD_ICON + DOT_GAP + w(RELOAD));
    parts.iter().sum::<f32>() + gap * (parts.len() - 1) as f32
}
