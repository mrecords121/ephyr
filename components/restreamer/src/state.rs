use derive_more::{Deref, From};
use futures_signals::signal::Mutable;
use juniper::{GraphQLEnum, GraphQLObject, GraphQLUnion};
use url::Url;

#[derive(Clone, Debug, Deref)]
pub struct State(Mutable<Vec<Restream>>);

impl State {
    pub fn new() -> Self {
        Self(Mutable::new(vec![
            Restream {
                enabled: false,
                input: Input::Push(PushInput {
                    name: "en".to_string(),
                    status: Status::Offline,
                }),
                outputs: vec![
                    Output {
                        dst: Url::parse("rtmp://126.3.6.2:1935/app/svgdv")
                            .unwrap(),
                        status: Status::Offline,
                        enabled: true,
                    },
                    Output {
                        dst: Url::parse("rtmp://facecast.io/en/svgdv").unwrap(),
                        status: Status::Initializing,
                        enabled: false,
                    },
                ],
            },
            Restream {
                enabled: true,
                input: Input::Pull(PullInput {
                    src: Url::parse("rtmp://youtube.yt/app/XXXXXX").unwrap(),
                    status: Status::Online,
                }),
                outputs: vec![Output {
                    dst: Url::parse("rtmp://126.3.6.2:1935/app/svgdv").unwrap(),
                    status: Status::Online,
                    enabled: true,
                }],
            },
        ]))
    }

    #[must_use]
    pub fn add_new_pull_input(&self, src: Url) -> bool {
        let mut restreams = self.0.lock_mut();

        if restreams
            .iter_mut()
            .find(|r| r.input.is_pull() && r.input.has_id(src.as_str()))
            .is_some()
        {
            return false;
        }

        restreams.push(Restream {
            input: PullInput {
                src,
                status: Status::Offline,
            }
            .into(),
            outputs: vec![],
            enabled: true,
        });
        true
    }

    #[must_use]
    pub fn add_new_push_input(&self, name: String) -> bool {
        let mut restreams = self.0.lock_mut();

        if restreams
            .iter_mut()
            .find(|r| !r.input.is_pull() && r.input.has_id(&name))
            .is_some()
        {
            return false;
        }

        restreams.push(Restream {
            input: PushInput {
                name,
                status: Status::Offline,
            }
            .into(),
            outputs: vec![],
            enabled: true,
        });
        true
    }

    #[must_use]
    pub fn remove_input(&self, input_id: &str) -> bool {
        let mut restreams = self.0.lock_mut();
        let prev_len = restreams.len();
        restreams.retain(|r| !r.input.has_id(input_id));
        restreams.len() != prev_len
    }

    #[must_use]
    pub fn enable_input(&self, input_id: &str) -> Option<bool> {
        let mut restreams = self.0.lock_mut();
        let input = restreams.iter_mut().find(|r| r.input.has_id(input_id))?;

        if input.enabled {
            return Some(false);
        }

        input.enabled = true;
        Some(true)
    }

    #[must_use]
    pub fn disable_input(&self, input_id: &str) -> Option<bool> {
        let mut restreams = self.0.lock_mut();
        let input = restreams.iter_mut().find(|r| r.input.has_id(input_id))?;

        if !input.enabled {
            return Some(false);
        }

        input.enabled = false;
        Some(true)
    }

    #[must_use]
    pub fn add_new_output(
        &self,
        input_id: &str,
        output_dst: Url,
    ) -> Option<bool> {
        let mut restreams = self.0.lock_mut();
        let outputs = &mut restreams
            .iter_mut()
            .find(|r| r.input.has_id(input_id))?
            .outputs;

        if outputs.iter_mut().find(|o| &o.dst == &output_dst).is_some() {
            return Some(false);
        }

        outputs.push(Output {
            dst: output_dst,
            enabled: false,
            status: Status::Offline,
        });
        Some(true)
    }

    #[must_use]
    pub fn remove_output(
        &self,
        input_id: &str,
        output_dst: &Url,
    ) -> Option<bool> {
        let mut restreams = self.0.lock_mut();
        let outputs = &mut restreams
            .iter_mut()
            .find(|r| r.input.has_id(input_id))?
            .outputs;

        let prev_len = outputs.len();
        outputs.retain(|o| &o.dst != output_dst);
        Some(outputs.len() != prev_len)
    }

    #[must_use]
    pub fn enable_output(
        &self,
        input_id: &str,
        output_dst: &Url,
    ) -> Option<bool> {
        let mut restreams = self.0.lock_mut();
        let output = &mut restreams
            .iter_mut()
            .find(|r| r.input.has_id(input_id))?
            .outputs
            .iter_mut()
            .find(|o| &o.dst == output_dst)?;

        if output.enabled {
            return Some(false);
        }

        output.enabled = true;
        Some(true)
    }

    #[must_use]
    pub fn disable_output(
        &self,
        input_id: &str,
        output_dst: &Url,
    ) -> Option<bool> {
        let mut restreams = self.0.lock_mut();
        let output = &mut restreams
            .iter_mut()
            .find(|r| r.input.has_id(input_id))?
            .outputs
            .iter_mut()
            .find(|o| &o.dst == output_dst)?;

        if !output.enabled {
            return Some(false);
        }

        output.enabled = false;
        Some(true)
    }
}

#[derive(Clone, Debug, Eq, GraphQLObject, PartialEq)]
pub struct Restream {
    pub input: Input,
    pub outputs: Vec<Output>,
    pub enabled: bool,
}

#[derive(Clone, Debug, Eq, From, GraphQLUnion, PartialEq)]
pub enum Input {
    Push(PushInput),
    Pull(PullInput),
}

impl Input {
    #[inline]
    #[must_use]
    pub fn is_pull(&self) -> bool {
        matches!(self, Input::Pull(_))
    }

    #[inline]
    #[must_use]
    pub fn has_id(&self, id: &str) -> bool {
        match self {
            Self::Pull(i) => i.src.as_str() == id,
            Self::Push(i) => i.name.as_str() == id,
        }
    }

    #[inline]
    #[must_use]
    pub fn is(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Pull(a), Self::Pull(b)) => a.is(b),
            (Self::Push(a), Self::Push(b)) => a.is(b),
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Eq, GraphQLObject, PartialEq)]
pub struct PullInput {
    pub src: Url,
    pub status: Status,
}

impl PullInput {
    #[inline]
    #[must_use]
    pub fn is(&self, other: &Self) -> bool {
        &self.src == &other.src
    }
}

#[derive(Clone, Debug, Eq, GraphQLObject, PartialEq)]
pub struct PushInput {
    pub name: String,
    pub status: Status,
}

impl PushInput {
    #[inline]
    #[must_use]
    pub fn is(&self, other: &Self) -> bool {
        &self.name == &other.name
    }
}

#[derive(Clone, Debug, Eq, GraphQLObject, PartialEq)]
pub struct Output {
    pub dst: Url,
    pub enabled: bool,
    pub status: Status,
}

impl Output {
    #[inline]
    #[must_use]
    pub fn is(&self, other: &Self) -> bool {
        &self.dst == &other.dst
    }
}

#[derive(Clone, Copy, Debug, Eq, GraphQLEnum, PartialEq)]
pub enum Status {
    Offline,
    Initializing,
    Online,
}
