#![deny(
    nonstandard_style,
    rust_2018_idioms,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code
)]
#![warn(
    deprecated_in_future,
    missing_docs,
    unused_import_braces,
    unused_labels,
    unused_qualifications,
    unreachable_pub
)]

pub mod input;
//pub mod mixer;

use std::marker::PhantomData;

use futures::StreamExt as _;
use slog_scope as log;

use self::input::teamspeak;
use slog::Drain;

fn main() {
    // This guard should be held till the end of the program for the logger
    // to present in global context.
    let _log_guard = slog_scope::set_global_logger({
        use slog::Drain as _;
        use slog_async::OverflowStrategy::Drop;

        let decorator = slog_term::TermDecorator::new().build();
        let drain = slog_term::CompactFormat::new(decorator).build().fuse();
        let drain = drain.filter_level(slog::Level::Error).fuse();
        let drain = slog_async::Async::new(drain)
            .overflow_strategy(Drop)
            .build()
            .fuse();

        slog::Logger::root(drain, slog::o!())
    });

    tokio_compat::run_std(async {
        let mut ts_input = teamspeak::Input::new("allatra.ruvoice.com:10335")
            .channel("Translation/test_channel")
            .name_as("[Bot] Origin SRS")
            .build();

        /*
        let mut ffmpeg = tokio::process::Command::new("ffplay")
            .arg("-f")
            .arg("f32be")
            .arg("-sample_rate")
            .arg("48000")
            //.arg("-use_wallclock_as_timestamps")
            //.arg("true")
            .arg("-i")
            .arg("pipe:0")
            .arg("-af")
            .arg("aresample=async=1")
            .arg("-i")
            .arg("http://radio.casse-tete.solutions/salut-radio-64.mp3")
            .arg("-map")
            .arg("0")
            .arg("-map")
            .arg("1")
            //.arg("-acodec")
            //.arg("libmp3lame")
            //.arg("-infbuf")
            .arg("-loglevel")
            .arg("debug")
            .stdin(std::process::Stdio::piped())
            //.stdout(std::process::Stdio::null())
            //.stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .expect("Failed to spawn FFmpeg");
        let ffmpeg_stdin =
            &mut ffmpeg.stdin.expect("FFmpeg's STDIN hasn't been captured");
*/
        let mut file = tokio::fs::File::create("test.pcm")
         .await.expect("create failed");

        //tokio::io::copy(&mut ts_input, ffmpeg_stdin)
        tokio::io::copy(&mut ts_input, &mut file)
            .await
            .expect("Failed to write data");

        /*
        let mixer = mixer::ffmpeg::new()
            .ts_audio(tsclient::TeamSpeakSettings {
                server_addr: "allatra.ruvoice.com:10335".into(),
                channel: "Translation/test_channel".into(),
                name_as: "[Bot] Origin SRS".into(),
            })
            .cmd("ffplay http://radio.casse-tete.solutions/salut-radio-64.mp3")
            .build()
            .start();

        let fetcher = tsclient::AudioFetcher {
            server_addr: "allatra.ruvoice.com:10335".into(),
            name: "[Bot] Origin SRS".into(),
            channel: "Translation/test_channel".into(),
            verbose: 1,
        };
        let conn = fetcher
            .start()
            .await
            .map_err(|e| log::error!("Starting AudioFetcher failed: {}", e));

        log::info!("woohoo!");

        tokio::time::delay_for(Duration::from_secs(60)).await;
        */
    });
}

pub struct State<V, S> {
    inner: V,
    _state: PhantomData<S>,
}

impl<V: Default, S> Default for State<V, S> {
    #[inline]
    fn default() -> Self {
        Self::wrap(V::default())
    }
}

impl<V, S> State<V, S> {
    #[inline]
    fn wrap(val: V) -> Self {
        Self {
            inner: val,
            _state: PhantomData,
        }
    }

    #[inline]
    fn unwrap(self) -> V {
        self.inner
    }

    #[inline]
    fn transit<IntoS>(self) -> State<V, IntoS> {
        State::wrap(self.inner)
    }
}
