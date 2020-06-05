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
            .class("presets_bar")
            .child_signal(state.active_stream.signal().dedupe()
                .map(move |n| {
                    let streams = streams.lock_ref();
                    let stream = streams.as_slice().get(n)?;
                    let active = stream.active_preset.clone();

                    Some(html!("ul", {.class("c-tab__list")
                        .attribute("role", "tablist")
                        .children_signal_vec(
                            stream.presets.signal_vec_cloned().enumerate()
                                .map(clone!(active, stream => move |(n, prst)| {
                                    Self::render_item(
                                        n.get().unwrap(),
                                        &prst,
                                        active.clone(),
                                        stream.clone(),
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
        stream: state::Stream,
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
            .event(clone!(active_preset, stream => move |_: events::Click| {
                active_preset.set(num);

                let presets = stream.presets.lock_ref();
                let active_preset = presets.as_slice().get(num).unwrap();

                for (mixer_name, srcs) in active_preset.volume.iter() {
                    let mixers = stream.mixers.lock_ref();
                    if let Some(mixer) = mixers.iter()
                        .find(|m| &*m.name.lock_ref() == mixer_name) {

                        for (src_name, volume) in srcs.iter() {
                            let sources = mixer.sources.lock_ref();
                            if let Some(src) = sources.iter()
                                .find(|s| &*s.name.lock_ref() == src_name) {
                                src.volume.set(volume.get());
                            }
                        }
                    }
                }

            }))
            .text_signal(preset.name.signal_cloned().dedupe_cloned()
                .map(String::from))
        })
    }
}
