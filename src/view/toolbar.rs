//! Search, then one pill per filter: sold per day, the player's skill, and priced only.

use rune::{Cursor, El, Key, KeyEvent, Tx, row, text, text_input};

use super::menu::trigger;
use super::widgets::{TOOLBAR_GAP, checkbox, pill, search_box};
use super::{App, Dims};
use crate::menu::{Menu, SOLD_STEPS, skill_active, sold_active};
use crate::settings;
use crate::theme::{self, p};

pub fn toolbar(s: &App, d: Dims) -> El {
    let sold = trigger(
        s,
        Menu::CraftsSold,
        sold_active(s.min_sold),
        format!("Sold per day: {}", SOLD_STEPS[s.min_sold].value),
        vec![text("Sold per day").font(theme::BODY).color(p().muted), text(SOLD_STEPS[s.min_sold].value).font(theme::BODY).color(p().text)],
    );
    let priced = checkbox("Priced only", s.hide_unpriced, |s: &mut App| {
        s.hide_unpriced = !s.hide_unpriced;
        s.refresh();
    });
    row().w_full().h(64.0).px(d.pad).gap(TOOLBAR_GAP).center_y().child(search(s)).child(sold).child(skill(s)).child(priced)
}

fn skill(s: &App) -> El {
    let field = text_input(&s.skill, theme::BODY)
        .key("skill")
        .single_line()
        .w(28.0)
        .color(p().text)
        .placeholder("Any", p().faint)
        .caret(p().brass, p().brass.alpha(0.3))
        .label("Your skill")
        .on_edit(|s: &mut App| &mut s.skill)
        .on_input_change(|s: &mut App, _: &mut Tx| {
            settings::save_skill(&s.skill.text);
            s.refresh();
        });
    pill(skill_active(&s.skill.text), s.skill.focused)
        .cursor(Cursor::Text)
        .child(text("Your skill").font(theme::BODY).color(p().muted))
        .child(field)
        .on_press(|s: &mut App| s.skill.focused = true)
}

fn search(s: &App) -> El {
    let field = text_input(&s.search, theme::BODY)
        .key("search")
        .single_line()
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .color(p().text)
        .placeholder("Search recipes or reagents", p().faint)
        .caret(p().brass, p().brass.alpha(0.3))
        .label("Search recipes or reagents")
        .on_edit(|s: &mut App| &mut s.search)
        .on_input_change(|s: &mut App, _: &mut Tx| s.refresh())
        .on_input_key(|s: &mut App, ev: &KeyEvent, tx: &mut Tx| match ev.key {
            Key::Escape => {
                s.clear_search();
                tx.blur();
                true
            }
            Key::Down => s.step(1, tx),
            Key::Up => s.step(-1, tx),
            _ => false,
        });
    search_box(field, s.search.focused).min_w(200.0)
}
