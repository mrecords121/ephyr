//! Implementation of [`cli::ServeCommand::VodMeta`].
//!
//! [`cli::ServeCommand::VodMeta`]: crate::cli::ServeCommand::VodMeta

use actix_web::{error, middleware, web, App, HttpServer};
use actix_web_httpauth::extractors::bearer::{self, BearerAuth};
use slog_scope as log;

use crate::{
    api::{nginx, vod},
    cli,
    vod::meta::{schedule_nginx_vod_module_set, state, State},
};

/// Runs [`cli::ServeCommand::VodMeta`].
///
/// # Errors
///
/// If running has failed and could not be performed. The appropriate error
/// is logged.
#[actix_rt::main]
pub async fn run(opts: cli::VodMetaOpts) -> Result<(), cli::Failure> {
    let state = state::Manager::try_new(&opts.state).await.map_err(|e| {
        log::error!("Failed to initialize vod::meta::State: {}", e);
        cli::Failure
    })?;
    let auth_token_hash = AuthTokenHash(opts.auth_token_hash);

    let _ = HttpServer::new(move || {
        App::new()
            .data(state.clone())
            .data(auth_token_hash.clone())
            .data(bearer::Config::default().realm("Restricted area"))
            .wrap(middleware::Logger::default())
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

    let playlist = state.playlist(&slug).await.ok_or_else(|| {
        error::ErrorNotFound(format!("Unknown playlist '{}'", slug))
    })?;

    Ok(web::Json(schedule_nginx_vod_module_set(&playlist)))
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

    let new = State::parse_request(req.0)
        .await
        .map_err(error::ErrorBadRequest)?;

    state
        .set_state(new)
        .await
        .map_err(error::ErrorInternalServerError)?;

    Ok("Ok")
}

/// Helper wrapper for extracting [`cli::VodMetaOpts::auth_token_hash`] in
/// [`actix_web`] handlers.
#[derive(Clone, Debug)]
struct AuthTokenHash(String);
