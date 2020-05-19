pub mod app;
pub mod state;
pub mod streams_bar;

use wasm_bindgen::prelude::wasm_bindgen;

use self::{app::App, state::State};

#[wasm_bindgen(start)]
pub fn main_js() {
    #[cfg(debug_assertions)]
    console_error_panic_hook::set_once();

    dominator::append_dom(
        &dominator::body(),
        App {
            state: State::from_seed(),
        }
        .render(),
    );
}
