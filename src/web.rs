//! The browser's entry: the desktop's window, drawn in the page's canvas.

use rune::AppSpec;

use crate::{theme, view};

#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    rune::run(AppSpec {
        setup: Some(view::setup),
        poll: Some(view::poll),
        on_key: Some(view::on_key),
        fonts: theme::FONTS,
        families: theme::FAMILIES,
        ..AppSpec::new("Craftski", (1640.0, 1000.0), view::App::new, view::view)
    });
}
