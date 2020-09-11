//! Implementation of [`cli::ServeCommand::VodMeta`].
//!
//! [`cli::ServeCommand::VodMeta`]: crate::cli::ServeCommand::VodMeta

use std::{
    convert::TryInto as _, panic::AssertUnwindSafe, sync::Arc, time::Duration,
};

use actix_web::{error, middleware, web, App, FromRequest as _, HttpServer};
use actix_web_httpauth::extractors::bearer::{self, BearerAuth};
use futures::{sink, FutureExt as _, StreamExt as _};
use slog_scope as log;
use tokio::time;

use crate::{
    api::{nginx, vod},
    cli,
    util::display_panic,
    vod::{
        file,
        meta::{state, State},
    },
};

/// Runs [`cli::ServeCommand::VodMeta`].
///
/// # Errors
///
/// If running has failed and could not be performed. The appropriate error
/// is logged.
#[actix_rt::main]
pub async fn run(opts: cli::VodMetaOpts) -> Result<(), cli::Failure> {
    let request_max_size =
        opts.request_max_size.get_bytes().try_into().map_err(|e| {
            log::error!("Maximum request size has too big value: {}", e);
            cli::Failure
        })?;

    let state = state::Manager::try_new(&opts.state).await.map_err(|e| {
        log::error!("Failed to initialize vod::meta::State: {}", e);
        cli::Failure
    })?;
    state.refresh_playlists_positions().await.map_err(|e| {
        log::error!(
            "Failed to refresh vod::meta::State initial positions: {}",
            e,
        );
        cli::Failure
    })?;

    let cache = Arc::new(
        file::cache::Manager::try_new(opts.cache_dir).map_err(|e| {
            log::error!("Failed to initialize vod::file::cache: {}", e);
            cli::Failure
        })?,
    );

    tokio::spawn(refill_state_with_cache_files(
        state.clone(),
        cache.clone(),
        Duration::from_secs(10),
    ));

    tokio::spawn(refresh_initial_positions(
        state.clone(),
        Duration::from_secs(60),
    ));

    let auth_token_hash = AuthTokenHash(opts.auth_token_hash);

    let _ = HttpServer::new(move || {
        App::new()
            .data(state.clone())
            .data(cache.clone())
            .data(auth_token_hash.clone())
            .data(bearer::Config::default().realm("Restricted area"))
            .wrap(middleware::Logger::default())
            .app_data(web::Json::<vod::meta::Request>::configure(|cfg| {
                cfg.limit(request_max_size).error_handler(|err, _| {
                    error::ErrorBadRequest(format!(
                        "Invalid request body: {}",
                        err,
                    ))
                })
            }))
            .route(
                "/{location}/{playlist}/{filename}",
                web::get().to(produce_meta),
            )
            .route("/", web::put().to(renew_state))
    })
    .bind((opts.http_ip, opts.http_port))
    .map_err(|e| {
        log::error!("Failed to bind web server: {}", e);
        cli::Failure
    })?
    .run()
    .await;

    Ok(())
}

/// Responses with the [`nginx-vod-module` mapping][1] containing the playlist
/// which should be played, starting from now and on.
///
/// [1]: https://github.com/kaltura/nginx-vod-module#mapping-response-format
async fn produce_meta(
    state: web::Data<state::Manager>,
    path: web::Path<(String, String, String)>,
) -> Result<web::Json<nginx::vod_module::mapping::Set>, error::Error> {
    let slug = state::PlaylistSlug::new(&path.1).ok_or_else(|| {
        error::ErrorBadRequest(format!("Invalid playlist slug '{}'", path.1))
    })?;
    Ok(web::Json(
        state
            .playlist(&slug)
            .await
            .ok_or_else(|| {
                error::ErrorNotFound(format!("Unknown playlist '{}'", slug))
            })?
            .schedule_nginx_vod_module_set(),
    ))
}

/// Renews the `vod-meta` server [`State`] with the new one provided in
/// [`vod::meta::Request`].
///
/// # Authorization
///
/// __Mandatory.__ The [`vod::meta::Request`] must be authorized with
/// [Bearer HTTP token][1], which value is verified against
/// [`cli::VodMetaOpts::auth_token_hash`].
///
/// [1]: https://tools.ietf.org/html/rfc6750#section-2.1
async fn renew_state(
    state: web::Data<state::Manager>,
    cache: web::Data<Arc<file::cache::Manager>>,
    req: web::Json<vod::meta::Request>,
    auth_token_hash: web::Data<AuthTokenHash>,
    auth: BearerAuth,
) -> Result<&'static str, error::Error> {
    web::block(move || {
        argon2::verify_encoded(
            &auth_token_hash.as_ref().0,
            auth.token().as_bytes(),
        )
    })
    .await
    .map_err(error::ErrorInternalServerError)
    .and_then(|ok| {
        if ok {
            Ok(())
        } else {
            Err(error::ErrorUnauthorized("Invalid Bearer token provided"))
        }
    })?;

    let mut new = State::parse_request(req.0)
        .await
        .map_err(error::ErrorBadRequest)?;

    new.fill_with_cache_files(&cache)
        .await
        .map_err(error::ErrorInternalServerError)?;

    state
        .set_state(new, None)
        .await
        .map_err(error::ErrorInternalServerError)?;

    Ok("Ok")
}

/// Runs job, which periodically (with the given `period`) refills the given
/// `state` with information about files available in the given `cache`.
async fn refill_state_with_cache_files(
    state: state::Manager,
    cache: Arc<file::cache::Manager>,
    period: Duration,
) {
    async fn refill(
        state: state::Manager,
        cache: Arc<file::cache::Manager>,
    ) -> Result<(), anyhow::Error> {
        let (mut curr, ver) = state.state_and_version().await;
        curr.fill_with_cache_files(&cache).await?;
        state.set_state(curr, Some(ver)).await?;
        Ok(())
    }

    let _ = time::interval(period)
        .then(move |_| {
            log::debug!(
                "Refilling vod::meta::State with vod::file::cache triggered",
            );
            let (state, cache) = (state.clone(), cache.clone());
            async move {
                AssertUnwindSafe(refill(state, cache))
                    .catch_unwind()
                    .await
                    .map_err(|p| {
                        log::error!(
                            "Panicked while refilling vod::meta::State with \
                             vod::file::cache: {}",
                            display_panic(&p),
                        )
                    })?
                    .map_err(|e| {
                        log::error!(
                            "Failed to refill vod::meta::State with \
                             vod::file::cache: {}",
                            e,
                        )
                    })
            }
        })
        .map(Ok)
        .forward(sink::drain())
        .await;
}

/// Runs job, which periodically (with the given `period`) refreshes
/// [`state::Playlist::initial`] positions in the given `state`.
async fn refresh_initial_positions(state: state::Manager, period: Duration) {
    let _ = time::interval(period)
        .then(move |_| {
            log::debug!(
                "Refreshing vod::meta::state::Playlist::initial positions",
            );
            let state = state.clone();
            async move {
                AssertUnwindSafe(state.refresh_playlists_positions())
                    .catch_unwind()
                    .await
                    .map_err(|p| {
                        log::error!(
                            "Panicked while refreshing vod::meta::State \
                             initial positions: {}",
                            display_panic(&p),
                        )
                    })?
                    .map_err(|e| {
                        log::error!(
                            "Failed to refresh vod::meta::State initial \
                             positions: {}",
                            e,
                        )
                    })
            }
        })
        .map(Ok)
        .forward(sink::drain())
        .await;
}

/// Helper wrapper for extracting [`cli::VodMetaOpts::auth_token_hash`] in
/// [`actix_web`] handlers.
#[derive(Clone, Debug)]
struct AuthTokenHash(String);
