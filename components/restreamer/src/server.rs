//! HTTP servers.

use std::net::IpAddr;

use ephyr_log::log;
use futures::future;
use tokio::fs;

use crate::{
    cli::{Failure, Opts},
    ffmpeg, srs, State,
};

/// Runs all application's HTTP servers.
///
/// # Errors
///
/// If some [`HttpServer`] cannot run due to already used port, etc.
/// The actual error is witten to logs.
#[actix_web::main]
pub async fn run(mut cfg: Opts) -> Result<(), Failure> {
    let res = {
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

        let state = State::try_new(&cfg.state_path).await.map_err(|e| {
            log::error!("Failed to initialize server state: {}", e)
        })?;

        let callback_http_port = cfg.callback_http_port;
        let ffmpeg_path_str = ffmpeg_path.to_string_lossy().into_owned();
        let srs = srs::Server::try_new(
            &cfg.srs_path,
            &srs::Config {
                callback_port: callback_http_port,
                restreams: state.get_cloned(),
                ffmpeg_path: ffmpeg_path_str.clone(),
            },
        )
        .await
        .map_err(|e| log::error!("Failed to initialize SRS server: {}", e))?;
        state.on_change("refresh_srs_conf", move |restreams| {
            let srs = srs.clone();
            let ffmpeg_path = ffmpeg_path_str.clone();
            async move {
                srs.refresh(&srs::Config {
                    callback_port: callback_http_port,
                    restreams,
                    ffmpeg_path,
                })
                .await
                .map_err(|e| log::error!("Failed to refresh SRS config: {}", e))
            }
        });

        let mut restreamers =
            ffmpeg::RestreamersPool::new(ffmpeg_path, state.clone());
        state.on_change("spawn_restreamers", move |restreams| {
            future::ready(restreamers.apply(restreams))
        });

        future::try_join(
            self::client::run(&cfg, state.clone()),
            self::callback::run(&cfg, state),
        )
        .await
        .map(|_| ())
    };
    crate::await_async_drops().await;
    res
}

/// Client HTTP server responding to client requests.
pub mod client {
    use std::time::Duration;

    use actix_web::{
        get, middleware, route, web, App, Error, HttpRequest, HttpResponse,
        HttpServer,
    };
    use actix_web_static_files::ResourceFiles;
    use ephyr_log::log;
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
        #![allow(unused_results)]
        #![doc(hidden)]

        use std::collections::HashMap;

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
                .data(api::graphql::client::schema())
                .wrap(middleware::Logger::default())
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
        state::{State, Status},
    };

    pub async fn run(cfg: &Opts, state: State) -> Result<(), Failure> {
        Ok(HttpServer::new(move || {
            App::new()
                .app_data(state.clone())
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

    #[post("/")]
    async fn callback(
        req: web::Json<api::srs::callback::Request>,
        state: web::Data<State>,
    ) -> Result<&'static str, Error> {
        use api::srs::callback::Action;
        match req.action {
            Action::OnConnect => on_connect(&req, &*state)?,
            Action::OnPublish => on_publish(&req, &*state)?,
            Action::OnUnpublish => on_unpublish(&req, &*state)?,
        }
        Ok("0")
    }

    fn on_connect(
        req: &api::srs::callback::Request,
        state: &State,
    ) -> Result<(), Error> {
        let restreams = state.get_cloned();
        let restream = restreams
            .iter()
            .find(|r| r.enabled && r.input.uses_srs_app(&req.app))
            .ok_or_else(|| error::ErrorNotFound("Such `app` doesn't exist"))?;

        if restream.input.is_pull() && !req.ip.is_loopback() {
            return Err(error::ErrorForbidden("`app` is allowed only locally"));
        }
        Ok(())
    }

    fn on_publish(
        req: &api::srs::callback::Request,
        state: &State,
    ) -> Result<(), Error> {
        let mut restreams = state.lock_mut();
        let restream = restreams
            .iter_mut()
            .find(|r| r.enabled && r.input.uses_srs_app(&req.app))
            .ok_or_else(|| error::ErrorNotFound("Such `app` doesn't exist"))?;

        restream.input.set_status(Status::Online);
        Ok(())
    }

    fn on_unpublish(
        req: &api::srs::callback::Request,
        state: &State,
    ) -> Result<(), Error> {
        let mut restreams = state.lock_mut();
        let restream = restreams
            .iter_mut()
            .find(|r| r.enabled && r.input.uses_srs_app(&req.app))
            .ok_or_else(|| error::ErrorNotFound("Such `app` doesn't exist"))?;

        restream.input.set_status(Status::Offline);
        Ok(())
    }
}

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
