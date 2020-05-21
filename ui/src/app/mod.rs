use dominator::{html, Dom};

use crate::{presets_bar::PresetsBar, state::State, streams_bar::StreamsBar};

#[derive(Clone, Debug)]
pub struct App {
    pub state: State,
}

impl App {
    pub fn render(&self) -> Dom {
        let streams_bar = StreamsBar::render(&self.state);

        let presets_bar = PresetsBar::render(&self.state);

        html!("div", {
            .class("ephyr-ui")
            .children(&mut [
                streams_bar,
                presets_bar,
            ])
        })
    }
}
