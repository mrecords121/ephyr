//! Client [GraphQL] API providing application usage.
//!
//! [GraphQL]: https://graphql.com

use actix_web::http::StatusCode;
use futures::stream::BoxStream;
use futures_signals::signal::SignalExt as _;
use juniper::{graphql_object, graphql_subscription, GraphQLObject, RootNode};
use once_cell::sync::Lazy;
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
        }
    }

    fn state(context: &Context) -> Restreams {
        Restreams {
            restreams: context.state().get_cloned(),
        }
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
        context: &Context,
    ) -> Result<bool, graphql::Error> {
        if !matches!(src.scheme(), "rtmp" | "rtmps") {
            return Err(graphql::Error::new("INVALID_SRC_RTMP_URL")
                .status(StatusCode::BAD_REQUEST)
                .message("Provided `src` is invalid: non-RTMP scheme"));
        }
        if !context.state().add_new_pull_input(src) {
            return Err(graphql::Error::new("DUPLICATE_SRC_RTMP_URL")
                .status(StatusCode::CONFLICT)
                .message("Provided `src` is used already"));
        }
        Ok(true)
    }

    fn add_push_input(
        name: String,
        context: &Context,
    ) -> Result<bool, graphql::Error> {
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
        if !context.state().add_new_push_input(name) {
            return Err(graphql::Error::new("DUPLICATE_INPUT_NAME")
                .status(StatusCode::CONFLICT)
                .message("Provided `name` is used already"));
        }
        Ok(true)
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
        context: &Context,
    ) -> Result<Option<bool>, graphql::Error> {
        if !matches!(dst.scheme(), "rtmp" | "rtmps") {
            return Err(graphql::Error::new("INVALID_DST_RTMP_URL")
                .status(StatusCode::BAD_REQUEST)
                .message("Provided `dst` is invalid: non-RTMP scheme"));
        }
        context
            .state()
            .add_new_output(input_id, dst)
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
}

/// Root of all [GraphQL subscriptions][1] in [`Schema`].
///
/// [1]: https://spec.graphql.org/June2018/#sec-Root-Operation-Types
#[derive(Clone, Copy, Debug)]
pub struct SubscriptionsRoot;

#[graphql_subscription(name = "Subscriptions", context = Context)]
impl SubscriptionsRoot {
    async fn state(context: &Context) -> BoxStream<'static, Restreams> {
        context
            .state()
            .signal_cloned()
            .dedupe_cloned()
            .map(|v| Restreams { restreams: v })
            .to_stream()
            .boxed()
    }
}

#[derive(Clone, Debug, GraphQLObject)]
pub struct Info {
    pub public_host: String,
}

#[derive(Clone, Debug, Eq, GraphQLObject, PartialEq)]
pub struct Restreams {
    pub restreams: Vec<Restream>,
}
