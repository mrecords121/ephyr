pub mod ffmpeg;

use std::{borrow::Cow, marker::PhantomData};

#[derive(Clone, Debug)]
pub struct TeamSpeakSettings {
    pub server_addr: Cow<'static, str>,
    pub channel: Cow<'static, str>,
    pub name_as: Cow<'static, str>,
}
