//! [PCM] audio silence filler.
//!
//! [PCM]: https://wiki.multimedia.cx/index.php/PCM

use std::{
    future::Future as _,
    hint::unreachable_unchecked,
    pin::Pin,
    task::{Context, Poll},
    time::{self, Duration},
};

use futures::ready;
use tokio::{
    io::{self, AsyncRead},
    time::{delay_for, Delay},
};

/// Wrapper around `Src` that emits silence samples near the desired samples
/// rate, when `Src` produces no data itself.
///
/// # Warning
///
/// The produced sample rate is __not accurate__. It serves only the purpose to
/// emit _some_ data. To have a stable sample rate it should be resampled later
/// with an accurate solution like [`aresample` FFmpeg filter][1].
///
/// [1]: https://ffmpeg.org/ffmpeg-filters.html#aresample-1
pub struct Filler<Src> {
    /// Source audio stream to be filled with silence.
    src: Src,

    /// Expected samples rate to be produced.
    samples_rate: usize,

    /// Current [`State`] of this silence [`Filler`].
    state: State,
}

impl<Src> Filler<Src> {
    /// Creates new silence [`Filler`] wrapping the given `src` with the desired
    /// `samples_rate`.
    #[inline]
    pub fn new(src: Src, samples_rate: usize) -> Self {
        Self {
            src,
            samples_rate,
            state: State::Transmitting { timeout: None },
        }
    }
}

/// Possible states of [`Filler`].
enum State {
    /// Source audio data is transmitted. No silence is emitted.
    Transmitting {
        /// Timeout for transition into [`State::Silencing`] when no data is
        /// received from source.
        ///
        /// Without this timeout there is a possibility to insert silence into
        /// the source data, corrupting it this way.
        timeout: Option<Delay>,
    },

    /// Source audio data is absent. Silence is emitted.
    ///
    /// Silence is emitted every 100 milliseconds (time window).
    Silencing {
        /// Number of samples left to be emitted in the current time window
        /// (100 milliseconds).
        samples_left: usize,

        /// Time of when the next time window begins.
        next_reset: time::Instant,

        /// Time left to wait for the next time window to begin.
        wait: Option<Delay>,
    },
}

impl<Src> AsyncRead for Filler<Src>
where
    Src: AsyncRead + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        // If there is any data in source, just return it immediately "as is".
        if let Poll::Ready(bytes_read) =
            Pin::new(&mut self.src).poll_read(cx, buf)?
        {
            self.state = State::Transmitting { timeout: None };
            return Poll::Ready(Ok(bytes_read));
        }

        // Once source has no data, let's wait for `timeout` before start
        // silence emitting.
        if let State::Transmitting { timeout } = &mut self.state {
            if timeout.is_none() {
                *timeout = Some(delay_for(Duration::from_millis(50)));
            }
            ready!(Pin::new(timeout.as_mut().unwrap()).poll(cx));

            self.state = State::Silencing {
                samples_left: self.samples_rate / 10,
                next_reset: time::Instant::now() + Duration::from_millis(100),
                wait: None,
            };
        }

        // Once `timeout` fired, start to insert silence every 100 milliseconds.
        let samples_rate = self.samples_rate;
        if let State::Silencing {
            samples_left,
            next_reset,
            wait,
        } = &mut self.state
        {
            let now = time::Instant::now();
            if now >= *next_reset {
                *samples_left = samples_rate / 10;
                *next_reset = now + Duration::from_millis(100);
                *wait = None;
            }

            // One sample is 4 bytes long due to `f32be` encoding.
            let samples_written = (*samples_left).min(buf.len() / 4);

            if samples_written == 0 {
                if wait.is_none() {
                    *wait = Some(delay_for((*next_reset).duration_since(now)));
                }
                ready!(Pin::new(wait.as_mut().unwrap()).poll(cx));

                // In theory, this branch is unreachable, but it's better to
                // consider this case.
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }

            *samples_left = (*samples_left).saturating_sub(samples_written);

            let bytes_written = samples_written * 4;
            for b in &mut buf[..bytes_written] {
                *b = 0;
            }
            return Poll::Ready(Ok(bytes_written));
        }

        debug_assert!(false, "Unreachable Filler state reached");
        #[allow(unsafe_code)]
        unsafe {
            unreachable_unchecked();
        }
    }
}
