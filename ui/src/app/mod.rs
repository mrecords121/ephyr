use dominator::{html, Dom};

use crate::{presets_bar::PresetsBar, state::State, streams_bar::StreamsBar, mixers_dashboard::MixersDashboard};

#[derive(Clone, Debug)]
pub struct App {
    pub state: State,
}

impl App {
    pub fn render(&self) -> Dom {
        html!("div", {
            .class("ephyr-ui")
            .children(&mut [
                StreamsBar::render(&self.state),
                PresetsBar::render(&self.state),
                MixersDashboard::render(&self.state),
            ])
        })
    }
}
