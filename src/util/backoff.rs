//! [Backoff] implementation.
//!
//! [Backoff]: https://en.wikipedia.org/wiki/Exponential_backoff

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures::ready;
use pin_project::pin_project;
use slog_scope as log;
use tokio::{
    io::{self, AsyncRead},
    time::{delay_for, Delay},
};

/// [Backoff] implementation for the operation `T` produced by the factory `F`.
///
/// [Backoff]: https://en.wikipedia.org/wiki/Exponential_backoff
#[pin_project(project = BackoffProj)]
pub struct Backoff<T, F> {
    /// Factory function which produces a new instance of performed operation.
    factory: F,

    /// Current [`State`] of this [`Backoff`].
    #[pin]
    state: State<T>,

    // TODO #7: Use `backoff::ExponentialBackoff` instead.
    //      https://github.com/ALLATRA-IT/ephyr/issues/7
    /// Duration to perform the next [`Backoff`] delay with.
    duration: Duration,
}

impl<T, F> Backoff<T, F> {
    /// Minimum possible duration of [`Backoff`].
    pub const MIN_DURATION: Duration = Duration::from_millis(100);

    /// Maximum possible duration of [`Backoff`].
    pub const MAX_DURATION: Duration = Duration::from_secs(60);

    /// Creates and returns new [`Backoff`] wrapping an operations produced by
    /// the given `factory`.
    pub fn new(mut factory: F) -> Self
    where
        F: FnMut() -> T,
    {
        let state = State::Active {
            operation: (factory)(),
        };
        Self {
            factory,
            state,
            duration: Self::MIN_DURATION,
        }
    }

    /// Increases [`Backoff`] duration twice if it's not more than
    /// [`Backoff::MAX_DURATION`].
    #[inline]
    fn increase_duration(duration: &mut Duration) {
        if *duration < Self::MAX_DURATION {
            *duration *= 2;
        }
    }

    /// Resets [`Backoff`] duration to the default [`Backoff::MIN_DURATION`].
    #[inline]
    fn reset_duration(duration: &mut Duration) {
        if *duration > Self::MIN_DURATION {
            *duration = Self::MIN_DURATION;
        }
    }
}

/// Possible states of [`Backoff`].
#[pin_project(project = StateProj)]
enum State<T> {
    /// Operation is performing normally, without [`Backoff`] delay.
    Active {
        /// Operation being performed.
        #[pin]
        operation: T,
    },

    /// Operation is delayed due to failure and will be recreated once delay
    /// finishes.
    Delayed {
        /// [`Future`] that will be resolved once [`Backoff`] delay is finished.
        #[pin]
        delay: Delay,
    },
}

impl<T, F> AsyncRead for Backoff<T, F>
where
    T: AsyncRead,
    F: FnMut() -> T,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let BackoffProj {
            factory,
            mut state,
            duration,
        } = self.project();

        loop {
            let new_state = match state.as_mut().project() {
                StateProj::Active { operation } => {
                    match ready!(operation.poll_read(cx, buf)) {
                        Ok(num) => {
                            Self::reset_duration(duration);
                            return Poll::Ready(Ok(num));
                        }

                        Err(e) => {
                            if is_permanent(&e) {
                                log::error!("Permanent error: {}", e);
                                return Poll::Ready(Err(e));
                            }

                            log::error!("Backoff due to error: {}", e);
                            State::Delayed {
                                delay: delay_for(*duration),
                            }
                        }
                    }
                }

                StateProj::Delayed { delay } => {
                    ready!(delay.poll(cx));

                    log::debug!("Restart after backoff");
                    Self::increase_duration(duration);
                    State::Active {
                        operation: (factory)(),
                    }
                }
            };
            state.set(new_state);
        }
    }
}

/// Checks whether the given [`io::Error`] carries [`backoff::Error::Permanent`]
/// under-the-hood.
fn is_permanent(err: &io::Error) -> bool {
    err.get_ref()
        .and_then(|e| e.downcast_ref::<backoff::Error<io::Error>>())
        .map_or(false, |e| matches!(e, backoff::Error::Permanent(_)))
}
