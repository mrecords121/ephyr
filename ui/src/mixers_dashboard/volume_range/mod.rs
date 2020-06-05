use std::str::FromStr as _;

use decimal::Decimal;
use dominator::{clone, events, html, Dom};
use futures_signals::signal::{Mutable, SignalExt as _};
use web_sys::HtmlInputElement;

use crate::state::{self, Volume};

#[derive(Clone, Copy, Debug)]
pub struct VolumeRange;

impl VolumeRange {
    pub fn render(id: usize, src: &state::Source) -> Dom {
        let id = format!("volume_range-{}", id);
        let value = src.volume.clone();

        html!("div", {
            .class("volume_range")
            .class("c-range").class("c-range--inline")
            .children(&mut [
                html!("label", {
                    .class("c-range__label")
                    .attribute("for", &id)
                    .text_signal(src.name.signal_cloned().dedupe_cloned()
                        .map(|name| name.to_string()))
                }),
                html!("small", {
                    .class("url")
                    .class("c-range__message")
                    .text(&src.url)
                }),
                html!("small", {
                    .class("delay")
                    .class("c-range__message")
                    .text(&format!(
                        "Delay: {}", humantime::format_duration(src.delay),
                    ))
                }),
                html!("input", {.attribute("type", "range")
                    .class("c-range__input")
                    .attribute("id", &id)
                    .attribute("max", "200")
                    .attribute("step", "1")
                    .attribute_signal("value", value.signal().dedupe()
                        .map(|v| format!("{:.2}", Decimal::from(v) * Decimal::from(100))))
                    .event(clone!(value => move |ev: events::Change| {
                        let new = ev.dyn_target::<HtmlInputElement>()
                            .expect("VolumeRange is not HtmlInputElement")
                            .value()
                            .parse::<Decimal>()
                            .expect(
                                "Cannot parse VolumeRange::value as Decimal",
                            );
                        value.set(Volume::new(new / Decimal::from(100)).expect("Not volume!!"));
                    }))
                }),
                html!("small", {
                    .class("c-range__message")
                    .text_signal(value.signal().dedupe()
                        .map(|v| v.to_string()))
                }),
            ])
        })
    }
}
