pub mod app;
pub mod mixers_dashboard;
pub mod presets_bar;
pub mod state;
pub mod streams_bar;

use wasm_bindgen::prelude::wasm_bindgen;

use self::{app::App, state::State};

#[wasm_bindgen(start)]
pub fn main_js() {
    #[cfg(debug_assertions)]
    console_error_panic_hook::set_once();

    wasm_logger::init(wasm_logger::Config::default());

    dominator::append_dom(
        &dominator::body(),
        App {
            state: State::from_seed(),
        }
        .render(),
    );
}
