use dominator::{clone, events, html, Dom};
use futures_signals::{
    signal::{Mutable, SignalExt as _},
    signal_vec::SignalVecExt as _,
};

use crate::state;

pub struct MixersDashboard;

impl MixersDashboard {
    pub fn render(state: &state::State) -> Dom {
        let streams = state.streams.clone();

        html!("div", {
            .class("c-tab__panel")
            .class("u-p")
            .attribute_signal("aria-labeledby", state.active_stream.signal()
                .dedupe()
                .map(|n| format!("stream-{}", n)))
            .attribute("role", "tabpanel")
            .child_signal(state.active_stream.signal().dedupe()
                .map(move |n| {
                    let streams = streams.lock_ref();
                    let stream = streams.as_slice().get(n)?;

                    Some(Self::render_header(stream))
                }))
        })
    }

    pub fn render_header(stream: &state::Stream) -> Dom {
        html!("div", {
            .text_signal(stream.name.signal_cloned().dedupe_cloned()
                .map(|n| format!("rtmp://127.0.0.1/{}/????", n)))
        })
    }
}
