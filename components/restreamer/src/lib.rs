//! Ephyr [RTMP] re-streaming server.
//!
//! [RTMP]: https://en.wikipedia.org/wiki/Real-Time_Messaging_Protocol

#![deny(
    broken_intra_doc_links,
    missing_debug_implementations,
    nonstandard_style,
    rust_2018_idioms,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code
)]
#![warn(
    deprecated_in_future,
    missing_docs,
    unreachable_pub,
    unused_import_braces,
    unused_labels,
    unused_lifetimes,
    unused_qualifications,
    unused_results
)]

pub mod api;
pub mod cli;
pub mod server;
pub mod srs;
pub mod state;

use std::{
    any::Any,
    collections::HashMap,
    future::Future,
    sync::{Arc, Mutex},
};

use ephyr_log::slog;
use futures::future::{self, FutureExt as _};
use once_cell::sync::Lazy;
use tokio::task::JoinHandle;

pub use self::state::State;

/// Runs application.
///
/// # Errors
///
/// If running has failed and could not be performed. The appropriate error
/// is logged.
pub fn run() -> Result<(), cli::Failure> {
    let cfg = cli::Opts::from_args();

    // This guard should be held till the end of the program for the logger
    // to present in global context.
    let _log_guard = ephyr_log::init(cfg.verbose.or_else(|| {
        if cfg.debug {
            Some(slog::Level::Debug)
        } else {
            None
        }
    }));

    server::run(cfg)
}

/// Collection of [`JoinHandle`]s being awaited for completion of `async`
/// [`Drop`] at the moment.
///
/// See [`await_all_drops`]'s documentation for details.
#[allow(clippy::type_complexity)]
pub(crate) static ASYNC_DROPS: Lazy<Arc<Mutex<HashMap<u64, JoinHandle<()>>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

/// Awaits for all [`Drop`]s containing `async` operations to be completed.
///
/// Call this function __before__ shutting down the [`tokio::runtime`],
/// otherwise [`Drop`]s may not proceed normally.
///
/// This is required due to [`tokio::runtime`] [doesn't wait][1] all
/// [`tokio::spawn`]ed tasks to be fully processed when shutting down.
///
/// [1]: https://github.com/tokio-rs/tokio/issues/2053
pub async fn await_async_drops() {
    let drops = {
        ASYNC_DROPS
            .lock()
            .unwrap()
            .drain()
            .map(|(_, hndl)| hndl)
            .collect::<Vec<_>>()
    };

    let _ = future::join_all(drops).await;
}

/// [`tokio::spawn`]s the given `async` [`Drop`] operations and tracks its
/// completion via [`ASYNC_DROPS`].
///
/// All such operations can be awaited for completion via [`await_async_drops`]
/// function.
pub(crate) fn register_async_drop<F: Future + Send + 'static>(fut: F) {
    use rand::Rng as _;

    let mut drops = ASYNC_DROPS.lock().unwrap();

    let id = loop {
        let id = rand::thread_rng().gen::<u64>();
        if !drops.contains_key(&id) {
            break id;
        }
    };

    let _ = drops.insert(
        id,
        tokio::spawn(fut.map(move |_| {
            let _ = ASYNC_DROPS.lock().unwrap().remove(&id);
        })),
    );
}

/// Interprets given [panic payload][1] as displayable message.
///
/// [1]: std::panic::PanicInfo::payload
pub fn display_panic<'a>(err: &'a (dyn Any + Send + 'static)) -> &'a str {
    if let Some(s) = err.downcast_ref::<&str>() {
        return s;
    }
    if let Some(s) = err.downcast_ref::<String>() {
        return s.as_str();
    }
    "Box<Any>"
}
