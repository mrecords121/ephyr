use dominator::{html, Dom};

use crate::{state::State, streams_bar::StreamsBar};

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
            ])
        })
    }
}
