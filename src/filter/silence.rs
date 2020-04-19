use std::{
    collections::HashMap,
    fmt,
    future::Future,
    hint::unreachable_unchecked,
    mem,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::{self, Duration},
};

use futures::{future::Fuse, ready, FutureExt as _};
use tokio::{
    io::{self, AsyncRead},
    time::{delay_for, Delay},
};

pub struct Filler<Src> {
    src: Src,
    samples_rate: usize,
    state: State,
}

impl<Src> Filler<Src> {
    pub fn new(src: Src, samples_rate: usize) -> Self {
        Self {
            src,
            samples_rate,
            state: State::Transmitting { timeout: None },
        }
    }
}

enum State {
    Transmitting {
        timeout: Option<Delay>,
    },
    Silencing {
        samples_left: usize,
        next_reset: time::Instant,
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
        // If there is any data, just return it immediately "as is".
        if let Poll::Ready(bytes_read) =
            Pin::new(&mut self.src).poll_read(cx, buf)?
        {
            self.state = State::Transmitting { timeout: None };
            return Poll::Ready(Ok(bytes_read));
        }

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

            *samples_left =
                (*samples_left).checked_sub(samples_written).unwrap_or(0);

            let bytes_written = samples_written * 4;
            for b in &mut buf[..bytes_written] {
                *b = 0;
            }
            return Poll::Ready(Ok(bytes_written));
        }

        unreachable!();
    }
}
