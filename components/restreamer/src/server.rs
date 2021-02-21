//! HTTP servers.

use std::net::IpAddr;

use ephyr_log::log;
use futures::future;
use tokio::fs;

use crate::{
    cli::{Failure, Opts},
    ffmpeg, srs, teamspeak, State,
};

/// Initializes and runs all application's HTTP servers.
///
/// # Errors
///
/// If some [`HttpServer`] cannot run due to already used port, etc.
/// The actual error is witten to logs.
///
/// [`HttpServer`]: actix_web::HttpServer
#[actix_web::main]
pub async fn run(mut cfg: Opts) -> Result<(), Failure> {
    if cfg.public_host.is_none() {
        cfg.public_host = Some(
            detect_public_ip()
                .await
                .ok_or_else(|| {
                    log::error!("Cannot detect server's public IP address")
                })?
                .to_string(),
        );
    }

    let ffmpeg_path =
        fs::canonicalize(&cfg.ffmpeg_path).await.map_err(|e| {
            log::error!("Failed to resolve FFmpeg binary path: {}", e)
        })?;

    let state = State::try_new(&cfg.state_path)
        .await
        .map_err(|e| log::error!("Failed to initialize server state: {}", e))?;

    let _srs = srs::Server::try_new(
        &cfg.srs_path,
        &srs::Config {
            callback_port: cfg.callback_http_port,
            log_level: cfg.verbose.map(Into::into).unwrap_or_default(),
        },
    )
    .await
    .map_err(|e| log::error!("Failed to initialize SRS server: {}", e))?;

    let mut restreamers =
        ffmpeg::RestreamersPool::new(ffmpeg_path, state.clone());
    State::on_change("spawn_restreamers", &state.restreams, move |restreams| {
        restreamers.apply(&restreams);
        future::ready(())
    });

    future::try_join(
        self::client::run(&cfg, state.clone()),
        self::callback::run(&cfg, state),
    )
    .await?;

    teamspeak::finish_all_disconnects().await;

    Ok(())
}

/// Client HTTP server responding to client requests.
pub mod client {
    use std::time::Duration;

    use actix_service::Service as _;
    use actix_web::{
        dev::ServiceRequest, get, middleware, route, web, App, Error,
        HttpRequest, HttpResponse, HttpServer,
    };
    use actix_web_httpauth::extractors::{
        basic::{self, BasicAuth},
        AuthExtractor as _, AuthExtractorConfig, AuthenticationError,
    };
    use actix_web_static_files::ResourceFiles;
    use ephyr_log::log;
    use futures::{future, FutureExt as _};
    use juniper::http::playground::playground_source;
    use juniper_actix::{
        graphql_handler, subscriptions::subscriptions_handler,
    };
    use juniper_graphql_ws::ConnectionConfig;

    use crate::{
        api,
        cli::{Failure, Opts},
        State,
    };

    pub mod public_dir {
        #![allow(clippy::must_use_candidate, unused_results)]
        #![doc(hidden)]

        include!(concat!(env!("OUT_DIR"), "/generated.rs"));
    }

    /// Runs client HTTP server.
    ///
    /// Client HTTP server serves [`api::graphql::client`] on `/` endpoint.
    ///
    /// # Playground
    ///
    /// If [`cli::Opts::debug`] is specified then additionally serves
    /// [GraphQL Playground][2] on `/playground` endpoint with no authorization
    /// required.
    ///
    /// # Errors
    ///
    /// If [`HttpServer`] cannot run due to already used port, etc.
    /// The actual error is logged.
    ///
    /// [`cli::Opts::debug`]: crate::cli::Opts::debug
    /// [2]: https://github.com/graphql/graphql-playground
    pub async fn run(cfg: &Opts, state: State) -> Result<(), Failure> {
        let in_debug_mode = cfg.debug;

        let stored_cfg = cfg.clone();

        Ok(HttpServer::new(move || {
            let public_dir_files = public_dir::generate();
            let mut app = App::new()
                .app_data(stored_cfg.clone())
                .app_data(state.clone())
                .app_data(
                    basic::Config::default().realm("Any login is allowed"),
                )
                .data(api::graphql::client::schema())
                .wrap(middleware::Logger::default())
                .wrap_fn(|req, srv| match authorize(req) {
                    Ok(req) => srv.call(req).left_future(),
                    Err(e) => future::err(e).right_future(),
                })
                .service(graphql);
            if in_debug_mode {
                app = app.service(playground);
            }
            app.service(ResourceFiles::new("/", public_dir_files))
        })
        .bind((cfg.client_http_ip, cfg.client_http_port))
        .map_err(|e| log::error!("Failed to bind client HTTP server: {}", e))?
        .run()
        .await
        .map_err(|e| log::error!("Failed to run client HTTP server: {}", e))?)
    }

    /// Endpoint serving [`api::graphql::client`] directly.
    ///
    /// # Errors
    ///
    /// If GraphQL operation execution errors or fails.
    #[route("/api", method = "GET", method = "POST")]
    async fn graphql(
        req: HttpRequest,
        payload: web::Payload,
        schema: web::Data<api::graphql::client::Schema>,
    ) -> Result<HttpResponse, Error> {
        let ctx = api::graphql::Context::new(req.clone());
        if req.head().upgrade() {
            let cfg = ConnectionConfig::new(ctx)
                .with_keep_alive_interval(Duration::from_secs(5));
            subscriptions_handler(req, payload, schema.into_inner(), cfg).await
        } else {
            graphql_handler(&schema, &ctx, req, payload).await
        }
    }

    /// Endpoint serving [GraphQL Playground][1] for exploring
    /// [`api::graphql::client`].
    ///
    /// [1]: https://github.com/graphql/graphql-playground
    #[get("/api/playground")]
    async fn playground() -> HttpResponse {
        // Constructs API URL relatively to the current HTTP request's scheme
        // and authority.
        let html = playground_source("__API_URL__", None).replace(
            "'__API_URL__'",
            r"document.URL.replace(/\/playground$/, '')",
        );
        HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(html)
    }

    /// Performs [`HttpRequest`] [Basic authorization][1] as middleware against
    /// [`State::password_hash`]. Doesn't consider username anyhow.
    ///
    /// No-op if [`State::password_hash`] is [`None`].
    ///
    /// [1]: https://en.wikipedia.org/wiki/Basic_access_authentication
    fn authorize(req: ServiceRequest) -> Result<ServiceRequest, Error> {
        let hash =
            match req.app_data::<State>().unwrap().password_hash.get_cloned() {
                Some(h) => h,
                None => return Ok(req),
            };

        let err = || {
            AuthenticationError::new(
                req.app_data::<basic::Config>()
                    .unwrap()
                    .clone()
                    .into_inner(),
            )
        };

        let auth = BasicAuth::from_service_request(&req).into_inner()?;
        let pass = auth.password().ok_or_else(err)?;
        if argon2::verify_encoded(hash.as_str(), pass.as_bytes()) != Ok(true) {
            return Err(err().into());
        }

        Ok(req)
    }
}

/// Callback HTTP server responding to [SRS] HTTP callbacks.
///
/// [SRS]: https://github.com/ossrs/srs
pub mod callback {
    use actix_web::{error, middleware, post, web, App, Error, HttpServer};
    use ephyr_log::log;

    use crate::{
        api,
        cli::{Failure, Opts},
        state::{Input, State, Status},
    };

    /// Runs HTTP server for exposing [SRS] [HTTP Callback API][1] on `/`
    /// endpoint for responding to [SRS] HTTP callbacks.
    ///
    /// # Errors
    ///
    /// If [`HttpServer`] cannot run due to already used port, etc.
    /// The actual error is logged.
    ///
    /// [SRS]: https://github.com/ossrs/srs
    /// [1]: https://github.com/ossrs/srs/wiki/v3_EN_HTTPCallback
    pub async fn run(cfg: &Opts, state: State) -> Result<(), Failure> {
        Ok(HttpServer::new(move || {
            App::new()
                .data(state.clone())
                .wrap(middleware::Logger::default())
                .service(callback)
        })
        .bind((cfg.callback_http_ip, cfg.callback_http_port))
        .map_err(|e| log::error!("Failed to bind callback HTTP server: {}", e))?
        .run()
        .await
        .map_err(|e| {
            log::error!("Failed to run callback HTTP server: {}", e)
        })?)
    }

    /// Endpoint serving the whole [HTTP Callback API][1] for [SRS].
    ///
    /// # Errors
    ///
    /// If [SRS] HTTP callback doesn't succeed.
    ///
    /// [SRS]: https://github.com/ossrs/srs
    /// [1]: https://github.com/ossrs/srs/wiki/v3_EN_HTTPCallback
    #[post("/")]
    async fn callback(
        req: web::Json<api::srs::callback::Request>,
        state: web::Data<State>,
    ) -> Result<&'static str, Error> {
        use api::srs::callback::Event;
        match req.action {
            Event::OnConnect => on_connect(&req, &*state)?,
            Event::OnPublish => on_publish(&req, &*state)?,
            Event::OnUnpublish => on_unpublish(&req, &*state)?,
        }
        Ok("0")
    }

    /// Handles [`api::srs::callback::Event::OnConnect`].
    ///
    /// Only checks whether the appropriate [`state::Restream::input`] exists.
    ///
    /// # Errors
    ///
    /// If [`api::srs::callback::Request::app`] matches no existing
    /// [`state::Restream`].
    ///
    /// [`state::Restream`]: crate::state::Restream
    fn on_connect(
        req: &api::srs::callback::Request,
        state: &State,
    ) -> Result<(), Error> {
        let restreams = state.restreams.get_cloned();
        let _ = restreams
            .iter()
            .find(|r| r.enabled && r.uses_srs_app(&req.app))
            .ok_or_else(|| error::ErrorNotFound("Such `app` doesn't exist"))?;
        Ok(())
    }

    /// Handles [`api::srs::callback::Event::OnPublish`].
    ///
    /// Updates the appropriate [`state::Restream::input`] to
    /// [`Status::Online`].
    ///
    /// # Errors
    ///
    /// - If [`api::srs::callback::Request::app`] or
    ///   [`api::srs::callback::Request::stream`] matches no existing
    ///   [`state::Restream`].
    /// - If [`state::Restream`] with [`PullInput`] is tried to be published
    ///   by external client.
    ///
    /// [`PullInput`]: crate::state::PullInput
    /// [`state::Restream`]: crate::state::Restream
    fn on_publish(
        req: &api::srs::callback::Request,
        state: &State,
    ) -> Result<(), Error> {
        let endpoint = req.stream.as_deref().unwrap_or_default();
        if !matches!(endpoint, "in" | "main" | "backup") {
            return Err(error::ErrorNotFound("Such `stream` doesn't exist"));
        }

        let mut restreams = state.restreams.lock_mut();
        let restream = restreams
            .iter_mut()
            .find(|r| r.enabled && r.uses_srs_app(&req.app))
            .ok_or_else(|| error::ErrorNotFound("Such `app` doesn't exist"))?;

        if !restream.input.is_failover() && endpoint != "in" {
            return Err(error::ErrorNotFound("Such `stream` doesn't exist"));
        }

        if restream.input.is_pull() && !req.ip.is_loopback() {
            return Err(error::ErrorForbidden("`app` is allowed only locally"));
        }

        if let Input::FailoverPush(input) = &mut restream.input {
            match endpoint {
                "main" => {
                    if input.main_srs_publisher_id.as_ref().map(|id| **id)
                        != Some(req.client_id)
                    {
                        input.main_srs_publisher_id =
                            Some(req.client_id.into());
                    }
                    input.main_status = Status::Online;
                    return Ok(());
                }
                "backup" => {
                    if input.backup_srs_publisher_id.as_ref().map(|id| **id)
                        != Some(req.client_id)
                    {
                        input.backup_srs_publisher_id =
                            Some(req.client_id.into());
                    }
                    input.backup_status = Status::Online;
                    return Ok(());
                }
                "in" if !req.ip.is_loopback() => {
                    return Err(error::ErrorForbidden(
                        "`app` is allowed only locally",
                    ));
                }
                _ => (),
            }
        }

        if restream.srs_publisher_id.as_ref().map(|id| **id)
            != Some(req.client_id)
        {
            restream.srs_publisher_id = Some(req.client_id.into());
        }
        restream.input.set_status(Status::Online);
        Ok(())
    }

    /// Handles [`api::srs::callback::Event::OnUnpublish`].
    ///
    /// Updates the appropriate [`state::Restream::input`] to
    /// [`Status::Offline`].
    ///
    /// # Errors
    ///
    /// If [`api::srs::callback::Request::app`] matches no existing
    /// [`state::Restream`].
    ///
    /// [`PullInput`]: crate::state::PullInput
    /// [`state::Restream`]: crate::state::Restream
    fn on_unpublish(
        req: &api::srs::callback::Request,
        state: &State,
    ) -> Result<(), Error> {
        let endpoint = req.stream.as_deref().unwrap_or_default();

        let mut restreams = state.restreams.lock_mut();
        let restream = restreams
            .iter_mut()
            .find(|r| r.uses_srs_app(&req.app))
            .ok_or_else(|| error::ErrorNotFound("Such `app` doesn't exist"))?;

        if restream.input.is_pull() {
            // For `PullInput` `Status::Offline` is managed by its FFmpeg
            // process.
            restream.srs_publisher_id = None;
            return Ok(());
        }

        if let Input::FailoverPush(input) = &mut restream.input {
            match endpoint {
                "main" => {
                    input.main_srs_publisher_id = None;
                    input.main_status = Status::Offline;
                    return Ok(());
                }
                "backup" => {
                    input.backup_srs_publisher_id = None;
                    input.backup_status = Status::Offline;
                    return Ok(());
                }
                _ => (),
            }
        }

        restream.srs_publisher_id = None;
        // For `FailoverPushInput` `Status::Offline` is managed by its FFmpeg
        // process.
        if !restream.input.is_failover() {
            restream.input.set_status(Status::Offline);
        }
        Ok(())
    }
}

/// Tries to detect public IP address of the machine where this application
/// runs.
///
/// See [`public_ip`] crate for details.
pub async fn detect_public_ip() -> Option<IpAddr> {
    use public_ip::{dns, http, BoxToResolver, ToResolver as _};

    public_ip::resolve_address(
        vec![
            BoxToResolver::new(dns::OPENDNS_RESOLVER),
            BoxToResolver::new(http::HTTP_IPIFY_ORG_RESOLVER),
        ]
        .to_resolver(),
    )
    .await
}
