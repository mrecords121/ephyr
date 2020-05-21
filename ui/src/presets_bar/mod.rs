use dominator::{clone, events, html, Dom};
use futures_signals::{
    signal::{Mutable, SignalExt as _},
    signal_vec::SignalVecExt as _,
};

use crate::state;

pub struct PresetsBar;

impl PresetsBar {
    pub fn render(state: &state::State) -> Dom {
        let streams = state.streams.clone();

        html!("aside", {.class("c-tab").class("c-tab--block")
            .class("presets-bar")
            .child_signal(state.active_stream.signal().dedupe()
                .map(move |n| {
                    let streams = streams.lock_ref();
                    let stream = streams.as_slice().get(n)?;
                    let active = stream.active_preset.clone();

                    Some(html!("ul", {.class("c-tab__list")
                        .attribute("role", "tablist")
                        .children_signal_vec(
                            stream.presets.signal_vec_cloned().enumerate()
                                .map(clone!(active => move |(n, preset)| {
                                    Self::render_item(
                                        n.get().unwrap(),
                                        &preset,
                                        active.clone(),
                                    )
                                }))
                        )}
                    ))
                }))
        })
    }

    fn render_item(
        num: usize,
        preset: &state::Preset,
        active_preset: Mutable<usize>,
    ) -> Dom {
        html!("li", {
            .attribute("aria-controls", &format!("panel{}", num))
            .attribute_signal("aria-expanded", active_preset.signal().dedupe()
                .map(move |n| if n == num { "true" } else { "false" }))
            .attribute_signal("aria-selected", active_preset.signal().dedupe()
                .map(move |n| if n == num { "true" } else { "false" }))
            .class("c-tab__list__item")
            .class_signal("is-selected", active_preset.signal().dedupe()
                .map(move |n| n == num))
            .attribute("id", &format!("preset-{}", num))
            .attribute("role", "tab")
            .attribute("tabindex", "0")
            .event(clone!(active_preset => move |_: events::Click| {
                active_preset.set(num);
            }))
            .text_signal(preset.name.signal_cloned().dedupe_cloned()
                .map(String::from))
        })
    }
}
