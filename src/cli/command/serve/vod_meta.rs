//! Implementation of [`cli::ServeCommand::VodMeta`].
//!
//! [`cli::ServeCommand::VodMeta`]: crate::cli::ServeCommand::VodMeta

use std::{
    fs,
    sync::{Arc, Mutex},
};

use actix_web::{error, web, App, HttpServer};
use slog_scope as log;

use crate::{
    api::{nginx, vod},
    cli,
    vod::meta::{
        schedule_nginx_vod_module_set,
        state::{PlaylistSlug, State},
    },
};

/// Runs [`cli::ServeCommand::VodMeta`].
///
/// # Errors
///
/// If running has failed and could not be performed. The appropriate error
/// is logged.
#[actix_rt::main]
pub async fn run(_opts: &cli::VodMetaOpts) -> Result<(), cli::Failure> {
    let state = serde_json::from_slice::<State>(
        &fs::read("example.vod.meta.json").map_err(|e| {
            log::error!("Failed to read persisted state: {}", e);
            cli::Failure
        })?,
    )
    .map_err(|e| {
        log::error!("Failed to deserialize persisted state: {}", e);
        cli::Failure
    })?;

    let state = Arc::new(Mutex::new(state));

    let _ = HttpServer::new(move || {
        App::new()
            .data(state.clone())
            .route(
                "/{location}/{playlist}/{filename}",
                web::get().to(produce_meta),
            )
            .route("/", web::put().to(renew_state))
    })
    .bind("0.0.0.0:8080")
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
    state: web::Data<Arc<Mutex<State>>>,
    path: web::Path<(String, String, String)>,
) -> Result<web::Json<nginx::vod_module::mapping::Set>, error::Error> {
    PlaylistSlug::new(&path.1)
        .and_then(|slug| state.lock().unwrap().0.get(&slug).cloned())
        .map(|playlist| web::Json(schedule_nginx_vod_module_set(&playlist)))
        .ok_or_else(|| {
            error::ErrorNotFound(format!("Unknown playlist '{}'", path.1))
        })
}

/// Renews the `vod-meta` server [`State`] with the new opne provided in
/// [`vod::meta::Request`].
async fn renew_state(
    state: web::Data<Arc<Mutex<State>>>,
    req: web::Json<vod::meta::Request>,
) -> Result<&'static str, error::Error> {
    let new_state = State::parse_request(req.0)
        .await
        .map_err(error::ErrorBadRequest)?;
    *state.lock().unwrap() = new_state;
    Ok("Ok")
}
