//! Client [GraphQL] API providing application usage.
//!
//! [GraphQL]: https://graphql.com

use actix_web::http::StatusCode;
use futures::stream::BoxStream;
use futures_signals::signal::SignalExt as _;
use juniper::{graphql_object, graphql_subscription, GraphQLObject, RootNode};
use once_cell::sync::Lazy;
use rand::Rng as _;
use regex::Regex;
use url::Url;

use crate::{
    api::graphql,
    state::{InputId, OutputId, Restream},
};

use super::Context;

/// Full schema of [`api::graphql::client`].
///
/// [`api::graphql::client`]: crate::api::graphql::client
pub type Schema =
    RootNode<'static, QueriesRoot, MutationsRoot, SubscriptionsRoot>;

/// Constructs and returns new [`Schema`], ready for use.
#[inline]
#[must_use]
pub fn schema() -> Schema {
    Schema::new(QueriesRoot, MutationsRoot, SubscriptionsRoot)
}

/// Root of all [GraphQL queries][1] in [`Schema`].
///
/// [1]: https://spec.graphql.org/June2018/#sec-Root-Operation-Types
#[derive(Clone, Copy, Debug)]
pub struct QueriesRoot;

#[graphql_object(name = "Queries", context = Context)]
impl QueriesRoot {
    fn info(context: &Context) -> Info {
        Info {
            public_host: context.config().public_host.clone().unwrap(),
            password_hash: context.state().password_hash.get_cloned(),
        }
    }

    fn restreams(context: &Context) -> Vec<Restream> {
        context.state().restreams.get_cloned()
    }
}

/// Root of all [GraphQL mutations][1] in [`Schema`].
///
/// [1]: https://spec.graphql.org/June2018/#sec-Root-Operation-Types
#[derive(Clone, Copy, Debug)]
pub struct MutationsRoot;

#[graphql_object(name = "Mutations", context = Context)]
impl MutationsRoot {
    fn add_pull_input(
        src: Url,
        replace_id: Option<InputId>,
        context: &Context,
    ) -> Result<Option<bool>, graphql::Error> {
        if !matches!(src.scheme(), "rtmp" | "rtmps") {
            return Err(graphql::Error::new("INVALID_SRC_RTMP_URL")
                .status(StatusCode::BAD_REQUEST)
                .message("Provided `src` is invalid: non-RTMP scheme"));
        }
        match context.state().add_pull_input(src, replace_id) {
            None => Ok(None),
            Some(true) => Ok(Some(true)),
            Some(false) => Err(graphql::Error::new("DUPLICATE_SRC_RTMP_URL")
                .status(StatusCode::CONFLICT)
                .message("Provided `src` is used already")),
        }
    }

    fn add_push_input(
        name: String,
        replace_id: Option<InputId>,
        context: &Context,
    ) -> Result<Option<bool>, graphql::Error> {
        static NAME_REGEX: Lazy<Regex> =
            Lazy::new(|| Regex::new("^[a-z0-9_-]{1,20}$").unwrap());
        if name.starts_with("pull_") {
            return Err(graphql::Error::new("INVALID_INPUT_NAME")
                .status(StatusCode::BAD_REQUEST)
                .message("Provided `name` is invalid: starts with 'pull_'"));
        }
        if !NAME_REGEX.is_match(&name) {
            return Err(graphql::Error::new("INVALID_INPUT_NAME")
                .status(StatusCode::BAD_REQUEST)
                .message("Provided `name` is invalid: not [a-z0-9_-]{1,20}"));
        }
        match context.state().add_push_input(name, replace_id) {
            None => Ok(None),
            Some(true) => Ok(Some(true)),
            Some(false) => Err(graphql::Error::new("DUPLICATE_INPUT_NAME")
                .status(StatusCode::CONFLICT)
                .message("Provided `name` is used already")),
        }
    }

    fn remove_input(id: InputId, context: &Context) -> bool {
        context.state().remove_input(id)
    }

    fn enable_input(id: InputId, context: &Context) -> Option<bool> {
        context.state().enable_input(id)
    }

    fn disable_input(id: InputId, context: &Context) -> Option<bool> {
        context.state().disable_input(id)
    }

    fn add_output(
        input_id: InputId,
        dst: Url,
        label: Option<String>,
        context: &Context,
    ) -> Result<Option<bool>, graphql::Error> {
        if !matches!(dst.scheme(), "rtmp" | "rtmps") {
            return Err(graphql::Error::new("INVALID_DST_RTMP_URL")
                .status(StatusCode::BAD_REQUEST)
                .message("Provided `dst` is invalid: non-RTMP scheme"));
        }

        static LABEL_REGEX: Lazy<Regex> =
            Lazy::new(|| Regex::new("^[a-zA-Z0-9_-]{1,40}$").unwrap());
        if let Some(label) = &label {
            if !LABEL_REGEX.is_match(label) {
                return Err(graphql::Error::new("INVALID_OUTPUT_LABEL")
                    .status(StatusCode::BAD_REQUEST)
                    .message(
                        "Provided `label` is invalid: not [a-zA-Z0-9_-]{1,40}",
                    ));
            }
        }

        context
            .state()
            .add_new_output(input_id, dst, label)
            .map(|added| {
                if added {
                    Ok(added)
                } else {
                    Err(graphql::Error::new("DUPLICATE_DST_RTMP_URL")
                        .status(StatusCode::CONFLICT)
                        .message(
                            "Provided `dst` is used already for this input",
                        ))
                }
            })
            .transpose()
    }

    fn remove_output(
        input_id: InputId,
        output_id: OutputId,
        context: &Context,
    ) -> Option<bool> {
        context.state().remove_output(input_id, output_id)
    }

    fn enable_output(
        input_id: InputId,
        output_id: OutputId,
        context: &Context,
    ) -> Option<bool> {
        context.state().enable_output(input_id, output_id)
    }

    fn disable_output(
        input_id: InputId,
        output_id: OutputId,
        context: &Context,
    ) -> Option<bool> {
        context.state().disable_output(input_id, output_id)
    }

    fn enable_all_outputs(
        input_id: InputId,
        context: &Context,
    ) -> Option<bool> {
        context.state().enable_all_outputs(input_id)
    }

    fn disable_all_outputs(
        input_id: InputId,
        context: &Context,
    ) -> Option<bool> {
        context.state().disable_all_outputs(input_id)
    }

    fn set_password(
        new: Option<String>,
        old: Option<String>,
        context: &Context,
    ) -> Result<bool, graphql::Error> {
        let mut current = context.state().password_hash.lock_mut();

        if let Some(hash) = &*current {
            match old {
                None => {
                    return Err(graphql::Error::new("NO_OLD_PASSWORD")
                        .status(StatusCode::FORBIDDEN)
                        .message("Old password required for this action"))
                }
                Some(pass) => {
                    if !argon2::verify_encoded(hash, pass.as_bytes()).unwrap() {
                        return Err(graphql::Error::new("WRONG_OLD_PASSWORD")
                            .status(StatusCode::FORBIDDEN)
                            .message("Wrong old password specified"));
                    }
                }
            }
        }

        if current.is_none() && new.is_none() {
            return Ok(false);
        }

        static HASH_CFG: Lazy<argon2::Config<'static>> =
            Lazy::new(argon2::Config::default);
        *current = new.map(|v| {
            argon2::hash_encoded(
                v.as_bytes(),
                &rand::thread_rng().gen::<[u8; 32]>(),
                &*HASH_CFG,
            )
            .unwrap()
        });
        Ok(true)
    }
}

/// Root of all [GraphQL subscriptions][1] in [`Schema`].
///
/// [1]: https://spec.graphql.org/June2018/#sec-Root-Operation-Types
#[derive(Clone, Copy, Debug)]
pub struct SubscriptionsRoot;

#[graphql_subscription(name = "Subscriptions", context = Context)]
impl SubscriptionsRoot {
    async fn info(context: &Context) -> BoxStream<'static, Info> {
        let public_host = context.config().public_host.clone().unwrap();
        context
            .state()
            .password_hash
            .signal_cloned()
            .dedupe_cloned()
            .map(move |h| Info {
                public_host: public_host.clone(),
                password_hash: h,
            })
            .to_stream()
            .boxed()
    }

    async fn restreams(context: &Context) -> BoxStream<'static, Vec<Restream>> {
        context
            .state()
            .restreams
            .signal_cloned()
            .dedupe_cloned()
            .to_stream()
            .boxed()
    }
}

#[derive(Clone, Debug, GraphQLObject)]
pub struct Info {
    pub public_host: String,
    pub password_hash: Option<String>,
}
