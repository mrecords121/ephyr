use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(start)]
pub fn main_js() {
    #[cfg(debug_assertions)]
    console_error_panic_hook::set_once();

    dominator::append_dom(&dominator::body(), dominator::html!("div", {
        .class(".some")
        .text("Hello, ephyr-ui!")
    }));
}
