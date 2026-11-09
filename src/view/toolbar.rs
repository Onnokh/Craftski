//! Search, then one pill per filter: sold per day, the player's skill, and priced only.

use rune::{El, Key, KeyEvent, Tx, text, text_input};

use super::menu::trigger;
use super::widgets::{self, checkbox, checkbox_width, field_box, field_box_width, pill_width, search_box};
use super::{App, Dims};
use crate::menu::{Menu, SOLD_STEPS, skill_active, sold_active};
use crate::settings;
use crate::theme::{self, p};

/// The narrowest the search gets beside the filters before it takes a row of its own.
const SEARCH_MIN: f32 = 200.0;
const SKILL_FIELD: f32 = 28.0;

/// The toolbar and its height.
pub fn toolbar(s: &App, d: Dims) -> (El, f32) {
    widgets::toolbar(d, SEARCH_MIN, |_| vec![search(s)], |short| vec![sold(s, short), skill(s, short), priced(s, short)])
}

fn sold(s: &App, short: bool) -> (El, f32) {
    let (label, value) = (if short { "Sold/day" } else { "Sold per day" }, SOLD_STEPS[s.min_sold].value);
    let pill = trigger(s, Menu::CraftsSold, sold_active(s.min_sold), format!("Sold per day: {value}"), vec![text(label).font(theme::BODY).color(p().muted), text(value).font(theme::BODY).color(p().text)]);
    (pill, pill_width(&[label, value]))
}

fn priced(s: &App, short: bool) -> (El, f32) {
    let label = if short { "Priced" } else { "Priced only" };
    let flip = |s: &mut App| {
        s.hide_unpriced = !s.hide_unpriced;
        s.refresh();
    };
    (checkbox(label, s.hide_unpriced, flip).key("Priced only").label("Priced only"), checkbox_width(label))
}

fn skill(s: &App, short: bool) -> (El, f32) {
    let label = if short { "Skill" } else { "Your skill" };
    let field = text_input(&s.skill, theme::BODY)
        .key("skill")
        .single_line()
        .w(SKILL_FIELD)
        .color(p().text)
        .placeholder("Any", p().faint)
        .caret(p().brass, p().brass.alpha(0.3))
        .label("Your skill")
        .on_edit(|s: &mut App| &mut s.skill)
        .on_input_change(|s: &mut App, _: &mut Tx| {
            settings::save_skill(s.profession, &s.skill.text);
            s.refresh();
        });
    let field = field_box(skill_active(&s.skill.text), s.skill.focused)
        .child(text(label).font(theme::BODY).color(p().muted))
        .child(field)
        .on_press(|s: &mut App| s.skill.focused = true);
    (field, field_box_width(label, SKILL_FIELD))
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
