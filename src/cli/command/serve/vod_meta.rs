//! Implementation of [`cli::ServeCommand::VodMeta`].
//!
//! [`cli::ServeCommand::VodMeta`]: crate::cli::ServeCommand::VodMeta

use std::fs;

use actix_web::{error, web, App, HttpServer};
use slog_scope as log;

use crate::{
    cli,
    vod::{nginx, state::State},
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

    let _ = HttpServer::new(move || {
        App::new().data(state.clone()).route(
            "/{location}/{playlist}/{filename}",
            web::get().to(produce_meta),
        )
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
    state: web::Data<State>,
    path: web::Path<(String, String, String)>,
) -> Result<web::Json<nginx::mapping::Set>, error::Error> {
    state
        .0
        .get(&path.1)
        .map(|playlist| web::Json(nginx::mapping::Set::from(playlist)))
        .ok_or_else(|| {
            error::ErrorNotFound(format!("Unknown playlist '{}'", path.1))
        })
}
