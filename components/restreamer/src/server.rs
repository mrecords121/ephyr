//! HTTP servers.

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
pub async fn run(cfg: Opts) -> Result<(), Failure> {
    let res = {
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

        future::try_join(self::client::run(&cfg, state), future::ok(()))
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

        Ok(HttpServer::new(move || {
            let mut app = App::new()
                .app_data(state.clone())
                .data(api::graphql::client::schema())
                .wrap(middleware::Logger::default())
                .service(graphql);
            if in_debug_mode {
                app = app.service(playground);
            }
            app
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
    #[route("/", method = "GET", method = "POST")]
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
    #[get("/playground")]
    async fn playground() -> HttpResponse {
        // Constructs API URL relatively to the current HTTP request's scheme
        // and authority.
        let html = playground_source("__API_URL__", None).replace(
            "'__API_URL__'",
            r"document.URL.replace(/\/playground$/, '/')",
        );
        HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(html)
    }
}
