//! Implementation of [`cli::ServeCommand::VodPlaylist`].
//!
//! [`cli::ServeCommand::VodPlaylist`]: crate::cli::ServeCommand::VodPlaylist

use actix_web::{web, App, HttpServer, Responder};
use slog_scope as log;

use crate::cli;

/// Runs [`cli::ServeCommand::VodPlaylist`].
///
/// # Errors
///
/// If running has failed and could not be performed. The appropriate error
/// is logged.
#[actix_rt::main]
pub async fn run(_opts: &cli::VodPlaylistOpts) -> Result<(), cli::Failure> {
    let _ = HttpServer::new(|| {
        App::new().service(web::resource("/{name}/{id}/index.html").to(index))
    })
    .bind("127.0.0.1:8080")
    .map_err(|e| {
        log::error!("Failed to bind web server: {}", e);
        cli::Failure
    })?
    .run()
    .await;

    Ok(())
}

/// Test index.
async fn index(info: web::Path<(String, u32)>) -> impl Responder {
    format!("Hello {}! id:{}", info.0, info.1)
}
