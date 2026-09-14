//! What is playing, and whether anything can be heard.
//!
//! In a browser this is an `<audio>` element the page owns. Handing it a URL
//! buys streaming, buffering, range requests and seeking from the platform —
//! four problems that are the browser's job and would each have to be solved
//! again to play the same bytes from Rust.
//!
//! On the desktop nothing is wired to an audio device. That is a real gap and
//! not a hidden one: [`Player::AUDIBLE`] says so, the now-playing bar reads it,
//! and a track selected there is shown rather than silently pretended to play.
//! Closing it means a decoder and an output device (`rodio`, so `cpal`, so
//! ALSA) plus an HTTP reader to feed them — worth doing when the desktop
//! client has a media store to stream from, which it does not yet.

use harken::Id;

/// What the bar is showing: enough to draw it without going back to the list,
/// because the list can change underneath a playing track.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub id: Id<harken::tables::Media>,
    pub title: String,
    pub creator: String,
    /// What the library says it runs for. The element reports its own duration
    /// once it has read enough of the stream, and that one is preferred when
    /// it arrives — a transcode is not always the length the catalogue claims.
    pub ms: i64,
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use wasm_bindgen::JsCast;
    use web_sys::HtmlAudioElement;

    pub struct Sink(Option<HtmlAudioElement>);

    impl Sink {
        pub const AUDIBLE: bool = true;

        pub fn new() -> Sink {
            Sink(None)
        }

        /// One element for the life of the page, reused across tracks. A new
        /// one per track leaks a media element and, in Safari, the first
        /// gesture's permission with it.
        fn el(&mut self) -> Option<&HtmlAudioElement> {
            if self.0.is_none() {
                let el = web_sys::window()?
                    .document()?
                    .create_element("audio")
                    .ok()?
                    .dyn_into::<HtmlAudioElement>()
                    .ok()?;
                el.set_preload("none");
                self.0 = Some(el);
            }
            self.0.as_ref()
        }

        pub fn play(&mut self, url: &str) {
            if let Some(el) = self.el() {
                el.set_src(url);
                let _ = el.play();
            }
        }

        pub fn resume(&mut self) {
            if let Some(el) = self.el() {
                let _ = el.play();
            }
        }

        pub fn pause(&mut self) {
            if let Some(el) = self.el() {
                el.pause().ok();
            }
        }

        pub fn is_playing(&self) -> bool {
            self.0
                .as_ref()
                .is_some_and(|el| !el.paused() && !el.ended())
        }

        pub fn ended(&self) -> bool {
            self.0.as_ref().is_some_and(|el| el.ended())
        }

        pub fn position(&self) -> f64 {
            self.0.as_ref().map(|el| el.current_time()).unwrap_or(0.0)
        }

        /// What the stream says it runs for, once enough of it has been read.
        /// `NaN` until then, and infinite for a live one — neither is a length,
        /// so both come back as `None` and the catalogue's figure is used.
        pub fn duration(&self) -> Option<f64> {
            let d = self.0.as_ref()?.duration();
            (d.is_finite() && d > 0.0).then_some(d)
        }

        pub fn seek(&mut self, secs: f64) {
            if let Some(el) = self.el() {
                el.set_current_time(secs);
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    /// The desktop has no audio device wired up. Everything here is a no-op on
    /// purpose rather than by omission: the bar asks `AUDIBLE` and says so.
    pub struct Sink;

    impl Sink {
        pub const AUDIBLE: bool = false;

        pub fn new() -> Sink {
            Sink
        }
        pub fn play(&mut self, _url: &str) {}
        pub fn resume(&mut self) {}
        pub fn pause(&mut self) {}
        pub fn is_playing(&self) -> bool {
            false
        }
        pub fn ended(&self) -> bool {
            false
        }
        pub fn position(&self) -> f64 {
            0.0
        }
        pub fn duration(&self) -> Option<f64> {
            None
        }
        pub fn seek(&mut self, _secs: f64) {}
    }
}

/// The selection and the thing that sounds it, together.
pub struct Player {
    track: Option<Track>,
    sink: imp::Sink,
}

impl Player {
    /// Whether this build can sound anything at all. False on the desktop.
    pub const AUDIBLE: bool = imp::Sink::AUDIBLE;

    pub fn new() -> Player {
        Player {
            track: None,
            sink: imp::Sink::new(),
        }
    }

    /// Start something. A track with no file is still *selected* — the bar
    /// shows it and says there is nothing to stream, which is what the library
    /// looks like when a song was typed in rather than seeded.
    pub fn play(&mut self, track: Track, url: &str) {
        self.track = Some(track);
        if url.is_empty() {
            self.sink.pause();
        } else {
            self.sink.play(url);
        }
    }

    pub fn toggle(&mut self) {
        if self.sink.is_playing() {
            self.sink.pause();
        } else {
            self.sink.resume();
        }
    }

    pub fn track(&self) -> Option<&Track> {
        self.track.as_ref()
    }

    pub fn is_playing(&self) -> bool {
        self.sink.is_playing()
    }

    /// The current track has run out, so the bar should move on.
    pub fn ended(&self) -> bool {
        self.track.is_some() && self.sink.ended()
    }

    pub fn position(&self) -> f64 {
        self.sink.position()
    }

    /// Seconds, from the stream if it knows and from the library otherwise.
    pub fn duration(&self) -> f64 {
        self.sink
            .duration()
            .unwrap_or_else(|| self.track.as_ref().map_or(0.0, |t| t.ms as f64 / 1000.0))
    }

    pub fn seek(&mut self, secs: f64) {
        self.sink.seek(secs);
    }
}
