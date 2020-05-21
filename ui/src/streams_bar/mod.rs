use dominator::{clone, events, html, Dom};
use futures_signals::{
    signal::{Mutable, SignalExt as _},
    signal_vec::SignalVecExt as _,
};

use crate::state::{self, State};

pub struct StreamsBar;

impl StreamsBar {
    pub fn render(state: &State) -> Dom {
        let active = state.active_stream.clone();

        html!("nav", {.class("c-tab")
            .children(&mut [
                html!("ul", {.class("c-tab__list").attribute("role", "tablist")
                    .children_signal_vec(
                        state.streams.signal_vec_cloned().enumerate()
                            .map(clone!(active => move |(n, stream)| {
                                Self::render_item(
                                    n.get().unwrap(),
                                    &stream,
                                    active.clone(),
                                )
                            }))
                    )}),
            ])
        })
    }

    fn render_item(
        num: usize,
        stream: &state::Stream,
        active_stream: Mutable<usize>,
    ) -> Dom {
        html!("li", {
            .attribute("aria-controls", &format!("panel{}", num))
            .attribute_signal("aria-expanded", active_stream.signal().dedupe()
                .map(move |n| if n == num { "true" } else { "false" }))
            .attribute_signal("aria-selected", active_stream.signal().dedupe()
                .map(move |n| if n == num { "true" } else { "false" }))
            .class("c-tab__list__item")
            .class_signal("is-selected", active_stream.signal().dedupe()
                .map(move |n| n == num))
            .attribute("id", &format!("tab-{}", num))
            .attribute("role", "tab")
            .attribute_signal("tabindex", active_stream.signal().dedupe()
                .map(move |n| if n == num { "0" } else { "-1" }))
            .event(clone!(active_stream => move |_: events::Click| {
                active_stream.set(num);
            }))
            .text_signal(stream.name.signal_cloned().dedupe_cloned()
                .map(String::from))
        })
    }
}
