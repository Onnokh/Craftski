//! The strip along the window's foot: the game, a dropdown of the realm prices come from, the
//! region's sales feed, each with its age, when the app read TSM's file, and a Reload.

use std::time::UNIX_EPOCH;

use rune::time::LocalTime;
use rune::{Color, El, Span, el, rich, row, text};

use super::menu::opener;
use super::widgets::{ARROW_ROOM, REFRESH, dropdown, red_button};
use super::{App, Dims, Ready};
use crate::market;
use crate::menu::Menu;
use crate::overview::{Feed, Freshness, read_at};
use crate::theme::{self, p};

const GAME: &str = "WoW Forever";
const PRICES: &str = "Prices";
const HEIGHT: f32 = 40.0;
const LEFT: f32 = 16.0;
const RIGHT: f32 = 12.0;
const GAP: f32 = 12.0;
/// The two-line groove between the game and its prices.
const DIVIDER: f32 = 2.0;
const RELOAD: (f32, f32) = (104.0, 28.0);
/// The width a failed reload's message keeps before the region feed goes.
const NOTE_KEPT: f32 = 200.0;
const DOT: f32 = 6.0;
/// Between a dot and its feed's words.
const DOT_GAP: f32 = 6.0;
/// Between "Prices" and the realm it names.
const LABEL_GAP: f32 = 8.0;
/// The realm dropdown, a size down from the toolbar's.
const PILL: f32 = 28.0;
const PILL_LEFT: f32 = 10.0;

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
    let room = d.status.unwrap_or(f32::INFINITY) - LEFT - RIGHT;
    let fits = FITS.map(|(with_region, with_read)| (with_region.then_some(&region), error.as_deref().or(with_read.then_some(read.as_str()))));
    let fit = fits.iter().find(|(region, note)| needed(&realm, *region, *note) <= room);
    let (region, note) = fit.unwrap_or(&fits[2]);
    let squeeze = fit.is_none();

    let prices = row().gap(LABEL_GAP).center_y().child(caption(PRICES, p().muted)).child(realm_pill(s, &realm, squeeze));
    let groove = row().h(20.0).a11y_hidden().child(el().w(1.0).h_full().fill(Color::BLACK)).child(el().w(1.0).h_full().fill(p().bevel.alpha(0.14)));
    let mut bar = row()
        .key("status")
        .w_full()
        .h(HEIGHT)
        .pl(LEFT)
        .pr(RIGHT)
        .gap(GAP)
        .center_y()
        .fill(p().status[1])
        .linear_gradient(std::f32::consts::PI, [(0.0, p().status[0]), (1.0, p().status[1])])
        .border_sides([1.0, 0.0, 0.0, 0.0], Color::BLACK)
        .child(el().pin(Some(0.0), Some(1.0), Some(0.0), None).h(1.0).fill(p().bevel.alpha(0.12)).a11y_hidden())
        .child(caption(GAME, p().gold))
        .child(groove)
        .child(if squeeze { prices.shrink().min_w(0.0) } else { prices });
    if let Some(region) = region {
        bar = bar.child(row().gap(DOT_GAP).center_y().label(region.line()).children(feed(region, p().muted, false)));
    }
    bar = bar.child(el().grow(1.0));
    if let Some(note) = note {
        // Squeezed, the error keeps only the width `needed` counted, so the realm keeps the rest.
        let error_note = || caption(note, p().loss).truncate().shrink().min_w(0.0);
        bar = bar.child(match (error.is_some(), squeeze) {
            (true, true) => error_note().max_w(NOTE_KEPT),
            (true, false) => error_note(),
            (false, _) => caption(note, p().muted),
        });
    }
    bar.child(reload())
}

/// The realm prices come from, as a small dropdown that opens the list of realms in AppData.lua.
fn realm_pill(s: &App, realm: &Feed, squeeze: bool) -> El {
    let base = dropdown(PILL, PILL_LEFT, false, s.menu == Some(Menu::Realm));
    let base = if squeeze { base.shrink().min_w(0.0) } else { base };
    opener(base, Menu::Realm, format!("Prices from {}", realm.line()), feed(realm, p().text, squeeze))
}

/// A dot and the feed's words, which truncate only when `squeeze` says nothing else fits.
fn feed(f: &Feed, color: Color, squeeze: bool) -> Vec<El> {
    let line = f.line();
    let words = match &f.age {
        // Past fresh, the age words take the dot's colour, so the warning reads without the dot.
        Some((ago, fresh)) if *fresh != Freshness::Fresh => rich(line.clone(), vec![Span::new(line.len() - ago.len(), line.len()).color(tint(*fresh))]),
        _ => text(line),
    };
    let words = words.font(theme::CAPTION).color(color).truncate();
    let dot = f.age.as_ref().map(|(_, fresh)| el().w(DOT).h(DOT).radius(DOT / 2.0).fill(tint(*fresh)).a11y_hidden());
    dot.into_iter().chain([if squeeze { words.shrink().min_w(0.0) } else { words }]).collect()
}

fn reload() -> El {
    red_button("Reload", "Reload TSM prices", &REFRESH, Some(RELOAD.0), RELOAD.1, |s: &mut App, _| s.reload(&market::appdata_path())).key("reload")
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
fn needed(realm: &Feed, region: Option<&Feed>, note: Option<&str>) -> f32 {
    let w = |words: &str| rune::text::measure(words, &[], theme::CAPTION, None).0;
    let feed = |f: &Feed| w(&f.line()) + if f.age.is_some() { DOT + DOT_GAP } else { 0.0 };
    let pill = PILL_LEFT + feed(realm) + ARROW_ROOM;
    let mut parts = vec![w(GAME), DIVIDER, w(PRICES) + LABEL_GAP + pill];
    parts.extend(region.map(feed));
    parts.push(0.0);
    parts.extend(note.map(|n| w(n).min(NOTE_KEPT)));
    parts.push(RELOAD.0);
    parts.iter().sum::<f32>() + GAP * (parts.len() - 1) as f32
}
