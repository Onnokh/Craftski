//! The selected flip: profit, where the sale price goes, the merchant and the auction house.

use rune::{El, ScrollSpec, Tx, col, row, text};

use super::Stocked;
use crate::breakdown::wowhead_url;
use crate::copper::Copper;
use crate::flips::{Flip, ListSource, Paid, RULES};
use crate::merchants::Merchant;
use crate::profit::{AuctionBasis, PriceSource, SELLER_PERCENT};
use crate::rows::per_day;
use crate::theme::{self, p};
use crate::view::widgets::{ButtonKind, Money, button, caps, flexible, icon_frame, section, unknown};
use crate::view::{App, Dims};

const PAD: f32 = 26.0;

pub fn detail(s: &App, f: &Stocked, d: Dims) -> El {
    let panel = col().w(d.detail).h_full().fill(p().surface).border_sides([0.0, 0.0, 0.0, 1.0], p().line);
    let Some((_, index)) = s.flips.picked() else {
        return panel.padding(PAD).child(text("Select a flip to see where its money goes.").font(theme::sans(13.0, 400.0)).color(p().muted).wrap());
    };
    let (flip, merchant, item) = f.at(index);
    let name = item.map_or_else(|| format!("Item #{}", flip.item), |i| i.name.clone());
    let quality = item.map_or(p().text, |i| p().quality(i.quality));
    let title = row()
        .gap(14.0)
        .center_y()
        .child(icon_frame(s.icons.item(item.and_then(|i| i.icon)), 56.0, 2.0, 11.0, quality))
        .child(
            flexible()
                .child(text(&name).font(theme::display(24.0).lh(26.4)).color(quality).wrap())
                .child(text(format!("{} · {}", merchant.name, merchant.role)).font(theme::sans(12.5, 400.0)).color(p().muted).mt(5.0)),
        );
    let body = col().w_full().gap(20.0).child(title).child(profit_card(flip, s.source)).child(money_flow(flip)).child(merchant_facts(flip, merchant)).child(auction_facts(flip));
    let url = wowhead_url(flip.item);
    let actions = row().w_full().px(PAD).pt(16.0).pb(22.0).child(button("Open on Wowhead", ButtonKind::Ghost, move |_: &mut App, tx: &mut Tx| {
        if let Err(e) = tx.open_url(&url) {
            eprintln!("{url}: {e}");
        }
    }));
    panel
        .child(col().w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(body.px(PAD).pt(PAD).pb(8.0)))
        .child(actions)
}

fn profit_card(flip: &Flip, source: PriceSource) -> El {
    let card = col().w_full().px(19.0).py(17.0).radius(14.0).fill(p().raised).border(1.0, p().line2).child(caps("Profit per item", p().faint));
    let (Some(profit), Some(paid)) = (flip.profit, flip.paid) else {
        let why = if flip.paid.is_none() { "Neither TSM, the client nor foreverdb knows what the merchant charges." } else { "It has no price on the auction house." };
        return card.child(text(why).font(theme::sans(13.0, 400.0)).color(p().muted).mt(10.0).wrap());
    };
    let value = Money::new(theme::mono(30.0, 500.0).lh(30.0), p().profit).coin(13.0, 7.0, 11.0).signed(profit);
    let prices = match source {
        PriceSource::Realm => "realm prices",
        PriceSource::Region => "region prices",
    };
    let line = format!("On {} paid · {prices}", paid.copper());
    card.child(row().mt(10.0).child(value)).child(text(line).font(theme::sans(12.5, 400.0)).color(p().muted).mt(10.0).wrap())
}

fn line(label: &str, value: El) -> El {
    row().w_full().h(26.0).center_y().child(text(label).font(theme::sans(13.0, 400.0)).color(p().muted)).child(rune::spacer()).child(value)
}

fn money_flow(flip: &Flip) -> El {
    let out = col().w_full().child(section("Where the money goes"));
    let (Some(sale), Some(resale), Some(paid), Some(profit)) = (flip.sale, flip.resale, flip.paid, flip.profit) else {
        return out.child(text("Needs a merchant price and an auction price.").font(theme::sans(13.0, 400.0)).color(p().muted));
    };
    let amount = Money::new(theme::mono(12.5, 400.0), p().text);
    let minus = |c: Copper| amount.with_sign(if c.0 > 0 { "\u{2212}" } else { "" }, c);
    let loss = flip.deposit.loss.map_or_else(|| unknown("no sale rate"), |l| minus(Copper(l.round() as u64)));
    let (label, color) = if profit < 0 { ("Loss", p().loss) } else { ("Profit", p().profit) };
    out.child(line("Sells for", amount.show(sale.unit)))
        .child(line(&format!("AH cut {}%", 100.0 - SELLER_PERCENT), minus(Copper(sale.unit.0 - resale.0))))
        .child(line(&format!("Paid · {}", paid.origin()), minus(paid.copper())))
        .child(line("Deposit loss", loss))
        .child(line(label, amount.color(color).with_sign(if profit < 0 { "\u{2212}" } else { "" }, Copper(profit.unsigned_abs()))).mt(4.0).border_sides([1.0, 0.0, 0.0, 0.0], p().line))
}

fn fact(label: &str, value: String) -> El {
    row()
        .w_full()
        .gap(12.0)
        .py(5.0)
        .items_start()
        .child(text(label).font(theme::sans(13.0, 400.0)).color(p().muted).w(104.0))
        .child(flexible().child(text(value).font(theme::sans(13.0, 400.0)).color(p().text).wrap()))
}

fn merchant_facts(flip: &Flip, m: &Merchant) -> El {
    let price = match flip.paid {
        None => "unknown".to_string(),
        Some(Paid::Tsm(_)) => "TSM's vendor price, as a merchant last charged you".to_string(),
        Some(Paid::List { from, discount_percent, .. }) => {
            let list = match from {
                ListSource::Client => "The client's list price",
                ListSource::ForeverDb => "foreverdb's list price",
            };
            match (discount_percent, &m.rep) {
                (0, _) | (_, None) => list.to_string(),
                (pct, Some(rep)) => format!("{list}, {pct}% off for Honored with {rep}"),
            }
        }
    };
    col()
        .w_full()
        .child(section("Merchant"))
        .child(fact("Name", m.name.clone()))
        .child(fact("Where", format!("{} {:.1}, {:.1}", m.zone, m.at.0, m.at.1)))
        .child(fact("Stock from", m.source.label().to_string()))
        .child(fact("Supply", flip.limit.map_or_else(|| "Endless".to_string(), |l| format!("{l} until the merchant restocks"))))
        .child(fact("Price", price))
}

fn auction_facts(flip: &Flip) -> El {
    let basis = match flip.sale.map(|s| s.basis) {
        Some(AuctionBasis::MinBuyout) => "The lowest listing on the realm now",
        Some(AuctionBasis::MarketValue) => "Nothing listed now · 14-day average",
        Some(AuctionBasis::Region) => "The region's market value",
        None => "No auction price",
    };
    let dep = flip.deposit;
    col()
        .w_full()
        .child(section("Auction house"))
        .child(fact("Sale price", basis.to_string()))
        .child(fact("Sale rate", flip.sale_rate.map_or_else(|| "No region data".to_string(), |r| format!("{:.0}% of listings sell", r * 100.0))))
        .child(fact("Sold per day", flip.sold_per_day.map_or_else(|| "No region data".to_string(), |v| format!("{} across the region", per_day(v)))))
        .child(fact("Deposit", format!("{} per listing of {}, {} hours", dep.per_listing, dep.post_stack, RULES.listing_hours)))
}
