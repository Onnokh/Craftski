//! The selected flip: profit per item, where the sale price goes, the merchant and the auction
//! house.

use rune::{El, ScrollSpec, Span, Tx, col, row, text};

use super::Stocked;
use crate::breakdown::wowhead_url;
use crate::copper::Copper;
use crate::flips::{Flip, ListSource, Paid, RULES};
use crate::merchants::Merchant;
use crate::profit::{AuctionBasis, SELLER_PERCENT};
use crate::rows::per_day;
use crate::theme::{self, p};
use crate::view::detail::{band, ledger, note};
use crate::view::widgets::{ButtonKind, Money, button, flexible, icon_frame, unknown};
use crate::view::{App, Dims};

const PAD: f32 = 24.0;
const ROW: f32 = 32.0;

pub fn detail(s: &App, f: &Stocked, d: Dims) -> El {
    let panel = col().w(d.detail).h_full().fill(p().surface).border_sides([0.0, 0.0, 0.0, 1.0], p().line);
    let Some((_, index, others)) = s.flips.picked() else {
        return panel.padding(PAD).child(text("Select a flip to see where its money goes.").font(theme::BODY).color(p().muted).wrap());
    };
    let (flip, merchant, item) = f.at(index);
    let name = item.map_or_else(|| format!("Item #{}", flip.item), |i| i.name.clone());
    let quality = item.map_or(p().text, |i| p().quality(i.quality));
    let identity = row()
        .w_full()
        .pt(20.0)
        .gap(16.0)
        .child(icon_frame(s.icons.item(item.and_then(|i| i.icon)), 48.0, quality))
        .child(
            flexible()
                .child(text(&name).font(theme::HEADING).color(quality).truncate())
                .child(text(format!("{}, {}.", merchant.name, merchant.role)).font(theme::CAPTION).color(p().muted).mt(4.0).truncate()),
        );
    let mut body = breakdown(col().w_full().child(identity).child(hero(flip)).child(band("Breakdown").mt(16.0)), flip);
    body = merchant_facts(body.child(band("Merchant")), flip, merchant);
    if !others.is_empty() {
        body = body.child(band("Other merchants"));
        for &i in others {
            body = body.child(other_merchant(f, i));
        }
    }
    let body = auction_facts(body.child(band("Auction house")), flip);
    let url = wowhead_url(flip.item);
    let actions = row().w_full().pt(16.0).child(button("Open on Wowhead", ButtonKind::Ghost, move |_: &mut App, tx: &mut Tx| {
        if let Err(e) = tx.open_url(&url) {
            eprintln!("{url}: {e}");
        }
    }));
    panel.pl(PAD + 1.0).pr(PAD).pb(PAD).child(col().w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(body)).child(actions)
}

/// The profit per item, or why there is none.
fn hero(flip: &Flip) -> El {
    let figure = flip.profit.map_or_else(|| text("\u{2014}").font(theme::FIGURE).color(p().faint), |pr| Money::new(theme::FIGURE, p().text).large().signed(pr));
    let out = col().w_full().mt(20.0).child(row().w_full().h(ROW).justify_between().center_y().child(text("Profit per item").font(theme::BODY).color(p().muted)).child(figure));
    match (flip.paid, flip.sale) {
        (None, _) => out.child(note("Neither TSM, the client nor foreverdb knows the merchant's price.", false)),
        (_, None) => out.child(note("It has no price on the auction house.", false)),
        _ => out,
    }
}

/// The sale price down to the profit: the auction house cut, the price paid and the deposit
/// expected to be lost.
fn breakdown(body: El, flip: &Flip) -> El {
    let money = Money::new(theme::BODY, p().text);
    let mut body = body.child(ledger("Sale price", flip.sale.map_or_else(unknown, |s| money.show(s.unit))));
    if flip.sale.is_some_and(|s| s.basis == AuctionBasis::MarketValue) {
        body = body.child(note("No current listings, 14-day average.", false));
    }
    let cut = flip.sale.zip(flip.resale).map(|(sale, resale)| Copper(sale.unit.0 - resale.0));
    let paid = flip.paid.map_or_else(|| "Paid".to_string(), |paid| format!("Paid, {}", paid.origin()));
    body.child(ledger(&format!("AH cut, {}%", 100.0 - SELLER_PERCENT), cut.map_or_else(unknown, |c| money.minus(c))))
        .child(ledger(&paid, flip.paid.map_or_else(unknown, |p| money.minus(p.copper()))))
        .child(ledger("Deposit loss", flip.deposit.loss.map_or_else(unknown, |l| money.minus(Copper(l.round() as u64)))))
}

/// A ledger row whose value is a phrase, cut short when the panel is too narrow for it.
fn fact(label: &str, value: String) -> El {
    ledger(label, text(value).font(theme::BODY).color(p().text).truncate().shrink().min_w(0.0)).gap(16.0)
}

fn merchant_facts(body: El, flip: &Flip, m: &Merchant) -> El {
    let price = match flip.paid {
        None => "Unknown",
        Some(Paid::Tsm(_)) => "TSM, as last charged",
        Some(Paid::List { from: ListSource::Client, .. }) => "Client list price",
        Some(Paid::List { from: ListSource::ForeverDb, .. }) => "foreverdb list price",
    };
    let body = body
        .child(fact("Where", format!("{} {:.1}, {:.1}", m.zone, m.at.0, m.at.1)))
        .child(fact("Stock from", m.source.label().to_string()))
        .child(fact("Supply", flip.limit.map_or_else(|| "Endless".to_string(), |l| format!("{l} until restock"))))
        .child(fact("Price", price.to_string()));
    match (flip.paid, &m.rep) {
        (Some(Paid::List { discount_percent, .. }), Some(rep)) if discount_percent > 0 => body.child(fact("Discount", format!("{discount_percent}%, Honored with {rep}"))),
        _ => body,
    }
}

/// Another merchant selling the item: name, zone, stock and price.
fn other_merchant(f: &Stocked, i: usize) -> El {
    let (flip, m, _) = f.at(i);
    let supply = flip.limit.map_or_else(|| "endless".to_string(), |l| format!("{l} in stock"));
    let line = format!("{}, {}, {supply}", m.name, m.zone);
    let what = text(&line).font(theme::BODY).color(p().text).spans(vec![Span::new(m.name.len(), line.len()).color(p().muted)]).truncate();
    row()
        .w_full()
        .h(ROW)
        .gap(16.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], p().rule)
        .child(flexible().child(what))
        .child(flip.paid.map_or_else(unknown, |paid| Money::new(theme::BODY, p().text).show(paid.copper())))
}

fn auction_facts(body: El, flip: &Flip) -> El {
    let dep = flip.deposit;
    let number = |v: String| text(v).font(theme::BODY).color(p().text);
    body.child(ledger("Sale rate", flip.sale_rate.map_or_else(unknown, |r| number(format!("{:.0}%", r * 100.0)))))
        .child(ledger("Sold per day, region", flip.sold_per_day.map_or_else(unknown, |v| number(per_day(v)))))
        .child(fact("Deposit", format!("{} per {}, {} hours", dep.per_listing, dep.post_stack, RULES.listing_hours)))
}
