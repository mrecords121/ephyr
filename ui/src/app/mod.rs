use dominator::{html, Dom};

use crate::{
    mixers_dashboard::MixersDashboard, presets_bar::PresetsBar, state::State,
    streams_bar::StreamsBar,
};

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
                html!("main", {
                    .class("content")
                    .children(&mut [
                        PresetsBar::render(&self.state),
                        MixersDashboard::render(&self.state),
                    ])
                }),
            ])
        })
    }
}
