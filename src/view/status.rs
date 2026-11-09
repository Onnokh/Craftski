//! The strip along the window's foot: the game, a dropdown of the realm prices come from, the
//! region's sales feed, each with its age, when the app read TSM's file, and a Reload.

use std::time::UNIX_EPOCH;

use rune::time::LocalTime;
use rune::{Color, Cursor, El, Span, col, el, rich, row, text};

use super::menu::opener;
use super::widgets::{ARROW_ROOM, DROPDOWN_GAP, REFRESH, glyph};
use super::{App, Dims, Ready};
use crate::market::PriceSource;
use crate::menu::Menu;
use crate::overview::{Feed, Freshness, checked_at, read_at};
use crate::theme::{self, p};

const GAME: &str = "WoW Forever";
const PRICES: &str = "Prices";
const PUBLIC: &str = "TSM public data";
const TSM_URL: &str = "https://tradeskillmaster.com";
const HEIGHT: f32 = 32.0;
const LEFT: f32 = 16.0;
const RIGHT: f32 = 12.0;
const GAP: f32 = 12.0;
/// The rule between the game and its prices.
const DIVIDER: f32 = 1.0;
const RELOAD: (f32, f32) = (84.0, 24.0);
/// The width a failed reload's message keeps before the region feed goes.
const NOTE_KEPT: f32 = 200.0;
const DOT: f32 = 6.0;
/// Between a dot and its feed's words.
const DOT_GAP: f32 = 6.0;
/// Between "Prices" and the realm it names.
const LABEL_GAP: f32 = 8.0;
/// The realm dropdown, a size down from the toolbar's and framed only while open.
const PILL: f32 = 24.0;
const PILL_LEFT: f32 = 8.0;

/// What the bar shows at a width: the region feed or not, and the read time or not. The read
/// time goes first, then the region feed; the last fit truncates the realm when even it
/// overflows. A failed reload's message takes the read time's place and stays, truncated.
const FITS: [(bool, bool); 3] = [(true, true), (true, false), (false, false)];

pub fn status(s: &App, r: &Ready, d: Dims) -> El {
    let read = r.market.read.at.duration_since(UNIX_EPOCH).map_or(0, |t| t.as_millis() as i64);
    let read = LocalTime::at(read);
    // Under public data the label names the source, which the read time names for AppData.lua.
    let (label, read) = match s.source {
        PriceSource::App => (PRICES, read_at(read.hour, read.minute)),
        PriceSource::Public(_) => (PUBLIC, checked_at(read.hour, read.minute)),
    };
    let error = r.reload_error.clone();
    let (realm, region) = (Feed::realm(&r.market.book, s.now), Feed::region(&r.market.book, s.now));
    let room = d.status.unwrap_or(f32::INFINITY) - LEFT - RIGHT;
    let fits = FITS.map(|(with_region, with_read)| (with_region.then_some(&region), error.as_deref().or(with_read.then_some(read.as_str()))));
    let fit = fits.iter().find(|(region, note)| needed(label, &realm, *region, *note) <= room);
    let (region, note) = fit.unwrap_or(&fits[2]);
    let squeeze = fit.is_none();

    let prices = row().gap(LABEL_GAP).center_y()
        .child(match s.source {
            PriceSource::App => caption(label, p().faint),
            PriceSource::Public(_) => caption(label, p().faint).cursor(Cursor::Pointer).href(TSM_URL),
        })
        .child(realm_pill(s, &realm, squeeze));
    let rule = el().w(DIVIDER).h(14.0).fill(p().rule).a11y_hidden();
    let mut bar = row()
        .key("status")
        .w_full()
        .h(HEIGHT)
        .pl(LEFT)
        .pr(RIGHT)
        .gap(GAP)
        .center_y()
        .child(caption(GAME, p().faint))
        .child(rule)
        .child(if squeeze { prices.shrink().min_w(0.0) } else { prices });
    if let Some(region) = region {
        bar = bar.child(row().gap(DOT_GAP).center_y().label(region.line()).children(feed(region, p().faint, false)));
    }
    bar = bar.child(el().grow(1.0));
    if let Some(note) = note {
        // Squeezed, the error keeps only the width `needed` counted, so the realm keeps the rest.
        let error_note = || caption(note, p().loss).truncate().shrink().min_w(0.0);
        bar = bar.child(match (error.is_some(), squeeze) {
            (true, true) => error_note().max_w(NOTE_KEPT),
            (true, false) => error_note(),
            (false, _) => caption(note, p().faint),
        });
    }
    let [_, right, bottom, left] = d.safe;
    col().w_full().pb(bottom).pl(left).pr(right).fill(p().ground).border_sides([1.0, 0.0, 0.0, 0.0], p().rule).child(bar.child(reload()))
}

/// The realm prices come from, as a small dropdown that opens the list of realms in AppData.lua.
fn realm_pill(s: &App, realm: &Feed, squeeze: bool) -> El {
    let open = s.menu == Some(Menu::Realm);
    let base = quiet(row().h(PILL).pl(PILL_LEFT).gap(DROPDOWN_GAP), open);
    let base = if squeeze { base.shrink().min_w(0.0) } else { base };
    opener(base, Menu::Realm, p().faint, format!("Prices from {}", realm.line()), feed(realm, p().muted, squeeze))
}

/// A dot and the feed's words, which truncate only when `squeeze` says nothing else fits.
fn feed(f: &Feed, color: Color, squeeze: bool) -> Vec<El> {
    let line = f.line();
    let words = match &f.age {
        // Stale, the age words take the dot's colour too, so the warning reads without the dot.
        // Aging shows in the dot alone: region prices update daily, so they are aging most hours.
        Some((ago, fresh)) if *fresh == Freshness::Stale => rich(line.clone(), vec![Span::new(line.len() - ago.len(), line.len()).color(tint(*fresh))]),
        _ => text(line),
    };
    let words = words.font(theme::CAPTION).color(color).truncate();
    let dot = f.age.as_ref().map(|(_, fresh)| el().w(DOT).h(DOT).radius(DOT / 2.0).fill(tint(*fresh)).a11y_hidden());
    dot.into_iter().chain([if squeeze { words.shrink().min_w(0.0) } else { words }]).collect()
}

fn reload() -> El {
    quiet(row().w(RELOAD.0).h(RELOAD.1).gap(6.0).justify_center(), false)
        .key("reload")
        .child(glyph(&REFRESH, 12.0, p().faint))
        .child(caption("Reload", p().muted))
        .role("button")
        .label("Reload TSM prices")
        .cursor(Cursor::Pointer)
        .on_press(|s: &mut App| s.reload_prices())
}

/// A control that stays bare until hovered, framed as the toolbar's while `open`.
fn quiet(out: El, open: bool) -> El {
    let out = out.radius(4.0).center_y().hover(Color::WHITE.alpha(0.05));
    if open { out.fill(Color::BLACK.alpha(0.45)).border(1.0, p().brass) } else { out }
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

/// The width the bar's parts take at their natural size, with the gap between each two. The
/// flexible space before the note counts as a part of no width; a long error note counts only
/// the part it keeps when truncated.
fn needed(label: &str, realm: &Feed, region: Option<&Feed>, note: Option<&str>) -> f32 {
    let w = |words: &str| rune::text::measure(words, &[], theme::CAPTION, None).0;
    let feed = |f: &Feed| w(&f.line()) + if f.age.is_some() { DOT + DOT_GAP } else { 0.0 };
    let pill = PILL_LEFT + feed(realm) + ARROW_ROOM;
    let mut parts = vec![w(GAME), DIVIDER, w(label) + LABEL_GAP + pill];
    parts.extend(region.map(feed));
    parts.push(0.0);
    parts.extend(note.map(|n| w(n).min(NOTE_KEPT)));
    parts.push(RELOAD.0);
    parts.iter().sum::<f32>() + GAP * (parts.len() - 1) as f32
}
