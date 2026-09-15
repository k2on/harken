//! The other socket: what this account is listening to, and where.
//!
//! `/sync` carries the log and this does not — see [`harken::listening`] for
//! why none of it is ever written down. What is here is one device's end of
//! it: a `WebSocket` carrying JSON, the session as the server last described
//! it, and the two questions the rest of the program asks.
//!
//! **Am I the output?** If not, this build makes no sound and its transport
//! buttons are sent rather than obeyed. That is the whole rule, and it lives
//! here so that [`crate::App`] has one thing to ask rather than a condition to
//! remember at each button.
//!
//! **A browser, and only a browser** — the same `cfg` the media session and
//! `Player::AUDIBLE` are under, for the same reason. The desktop build has no
//! audio device, so it can never be the output; being a *remote control* is
//! the half it could still do, and that wants a native WebSocket client, which
//! is a dependency this workspace does not have and a `cargoVendorHash` to
//! move for it. `nix run .#web` is the desktop client for anyone who wants
//! one, so the browser build is not a subset of the feature.
//!
//! **A device is a login.** `petros-auth` says a session is "one login on one
//! device", which is exactly the identity this wants and already exists — so
//! the device id *is* the login's session id. A reloaded tab is the same
//! device rather than a second one in the picker.

use harken::listening::{decode, encode, Hear, Say};
// Re-exported rather than re-imported at the call site: the rest of this
// program asks *this* module about the session, and a second import path to
// the same three types is a second place to look.
pub use harken::listening::{Command, Device, DeviceId, Session, Track};

/// A dropped socket is retried on this cadence. Slower than the engine's,
/// because nothing here is durable: what a device missed while it was away is
/// the whole state, and it arrives complete on the next `Hello`.
const RETRY_MS: f64 = 3_000.0;

/// How far the position may drift before the output says so again.
///
/// One number doing two jobs, which is why it is one rule rather than a
/// heartbeat plus a seek test: while playing, the position advances past this
/// about once a second, so it *is* the heartbeat; while paused it never moves,
/// so a paused output is silent; and a seek crosses it at once however long
/// the last report was ago.
const DRIFT_MS: i64 = 1_100;

/// Where the listening socket is, from where the log's socket is.
///
/// The rule that turns `http` into `ws` is written once, in `petros_auth`, and
/// this swaps the path rather than making a second copy of it.
fn listen_url(server: &str) -> String {
    let sync = petros_auth::socket_url(server);
    format!("{}/listen", sync.trim_end_matches("/sync"))
}

/// What was last reported, so that a quiet second costs nothing.
#[derive(Debug, Clone, PartialEq)]
struct Said {
    at: usize,
    playing: bool,
    len: usize,
    position_ms: i64,
}

/// This device's end of the account's listening session.
pub struct Remote {
    /// This device, which is the login's session id.
    me: DeviceId,
    /// What the picker draws for it.
    name: String,
    server: String,
    token: String,
    link: Option<imp::Wire>,
    /// The session as the server last described it, and when that was by this
    /// machine's clock.
    ///
    /// *This* machine's, deliberately: the server has a clock and so does
    /// every device and they do not agree, so a scrubber that has to move
    /// between reports counts from when the state arrived here.
    session: Option<Session>,
    since: f64,
    retry_at: f64,
    said: Option<Said>,
    /// The server's last word on this token. The socket is gone behind it and
    /// re-dialling with the same token would get the same answer.
    denied: Option<String>,
}

impl Remote {
    pub fn new() -> Remote {
        Remote {
            me: String::new(),
            name: imp::device_name(),
            server: String::new(),
            token: String::new(),
            link: None,
            session: None,
            since: 0.0,
            retry_at: 0.0,
            said: None,
            denied: None,
        }
    }

    /// Point this at a server, as a device. Called at sign-in and whenever the
    /// login changes; dialling itself happens on the next tick.
    pub fn open(&mut self, server: &str, token: &str, device: &str) {
        self.server = server.to_string();
        self.token = token.to_string();
        self.me = device.to_string();
        self.session = None;
        self.said = None;
        self.denied = None;
        self.link = None;
        self.retry_at = 0.0;
    }

    /// Stop. A peer that has signed out has no session to be part of.
    pub fn close(&mut self) {
        self.token.clear();
        self.link = None;
        self.session = None;
    }

    /// Whether there is a session at all — which is what decides whether the
    /// bar has a device picker to draw.
    pub fn live(&self) -> bool {
        self.session.is_some()
    }

    pub fn session(&self) -> Option<&Session> {
        self.session.as_ref()
    }

    pub fn denied(&self) -> Option<&str> {
        self.denied.as_deref()
    }

    /// This device, as the picker names it.
    pub fn me(&self) -> &str {
        &self.me
    }

    /// Whether this device is the one making the sound.
    pub fn outputs_here(&self) -> bool {
        self.session.as_ref().is_some_and(|s| s.outputs(&self.me))
    }

    /// Whether the sound is somewhere else — which is when a transport button
    /// is a message rather than an instruction.
    ///
    /// Not the negation of [`Remote::outputs_here`]: a session with *no*
    /// output is neither, and there the right answer is to play here and let
    /// the report claim it.
    pub fn elsewhere(&self) -> bool {
        self.session
            .as_ref()
            .is_some_and(|s| s.output.is_some() && !s.outputs(&self.me))
    }

    /// Every device of this account, for the picker.
    pub fn devices(&self) -> &[Device] {
        self.session.as_ref().map_or(&[], |s| &s.devices)
    }

    /// What the session says is playing, for a bar drawing somebody else's
    /// device.
    pub fn now(&self) -> Option<&Track> {
        self.session.as_ref().and_then(|s| s.now())
    }

    pub fn playing(&self) -> bool {
        self.session.as_ref().is_some_and(|s| s.playing)
    }

    /// How far in, counted forward from when the state arrived.
    ///
    /// The output resends whenever the position drifts past [`DRIFT_MS`], so
    /// this is never extrapolating more than about a second — which is the
    /// difference between a scrubber that moves and one that jumps.
    pub fn position_ms(&self) -> i64 {
        let Some(session) = &self.session else {
            return 0;
        };
        if !session.playing {
            return session.position_ms;
        }
        session.position_ms + (imp::now_ms() - self.since).max(0.0) as i64
    }

    /// Take whatever arrived, and dial again if the socket has gone.
    ///
    /// Returns what this device has been told to *do*, which the server only
    /// ever sends to the output.
    pub fn poll(&mut self) -> Vec<Command> {
        let mut todo = Vec::new();
        if self.token.is_empty() || self.denied.is_some() {
            return todo;
        }
        let Some(link) = &self.link else {
            if imp::now_ms() >= self.retry_at {
                self.dial();
            }
            return todo;
        };
        while let Some(text) = link.try_recv() {
            match decode::<Hear>(&text) {
                Some(Hear::State { session }) => {
                    self.session = Some(session);
                    self.since = imp::now_ms();
                }
                Some(Hear::Do { command }) => todo.push(command),
                Some(Hear::Denied { reason }) => {
                    self.denied = Some(reason);
                    self.link = None;
                    return todo;
                }
                // A sentence a newer server invented. Carrying on is the right
                // answer; dropping the socket over it is not.
                None => {}
            }
        }
        if !link.is_alive() {
            self.link = None;
            self.session = None;
            self.said = None;
            self.retry_at = imp::now_ms() + RETRY_MS;
        }
        todo
    }

    /// Say what this device is doing, if it is the one doing it.
    ///
    /// Called from the tick rather than from each button, for the reason the
    /// media session's announcement is: the element pauses itself when a
    /// stream stalls or runs out, and that was nobody's button press.
    pub fn report(&mut self, queue: &[Track], at: usize, playing: bool, position_ms: i64) {
        if self.elsewhere() || self.link.is_none() {
            return;
        }
        let now = Said {
            at,
            playing,
            len: queue.len(),
            position_ms,
        };
        let stale = match &self.said {
            None => true,
            Some(was) => {
                was.at != now.at
                    || was.playing != now.playing
                    || was.len != now.len
                    || (was.position_ms - now.position_ms).abs() >= DRIFT_MS
            }
        };
        if !stale {
            return;
        }
        self.said = Some(now);
        self.say(&Say::Report {
            queue: queue.to_vec(),
            at,
            playing,
            position_ms,
        });
    }

    /// Ask for something, wherever the sound is.
    pub fn ask(&mut self, command: Command) {
        self.assume(&command);
        self.say(&Say::Do { command });
    }

    /// Apply what was just asked for to the copy of the session this device
    /// holds, so the bar answers the button now rather than on the round trip.
    ///
    /// Optimistic in exactly the way the engine's own view is, and corrected
    /// the same way: the output does it, reports it, and the broadcast
    /// replaces whatever this guessed. A scrubber that waits for a round trip
    /// before it moves reads as a control that did not take — and it is
    /// dragged, so it would be a round trip per pixel.
    ///
    /// Only the three that can be guessed from here. `Next` and `Previous`
    /// change *which* track it is, which means guessing against a queue the
    /// other device holds, and `Start` is that with a list attached. Those
    /// wait, because a bar showing the wrong title is worse than a bar a
    /// moment behind.
    fn assume(&mut self, command: &Command) {
        let counted = self.position_ms();
        let Some(session) = &mut self.session else {
            return;
        };
        match command {
            Command::Play => {
                session.position_ms = counted;
                session.playing = true;
            }
            Command::Pause => {
                session.position_ms = counted;
                session.playing = false;
            }
            Command::Seek { position_ms } => session.position_ms = *position_ms,
            _ => return,
        }
        self.since = imp::now_ms();
    }

    /// Move the sound, or stop it everywhere with `None`.
    pub fn transfer(&mut self, to: Option<DeviceId>) {
        // The session about to be handed over is the one this device last
        // reported; forgetting that here means the next report says it again,
        // which is what a device that has just *lost* the output should not do.
        self.said = None;
        self.say(&Say::Transfer { to });
    }

    fn say(&self, frame: &Say) {
        if let Some(link) = &self.link {
            link.send(&encode(frame));
        }
    }

    fn dial(&mut self) {
        // Set before the attempt, so a connect that throws does not spin.
        self.retry_at = imp::now_ms() + RETRY_MS;
        let Some(link) = imp::Wire::connect(&listen_url(&self.server)) else {
            return;
        };
        link.send(&encode(&Say::Hello {
            token: self.token.clone(),
            device: self.me.clone(),
            name: self.name.clone(),
            // What this build can actually do. The desktop cannot be heard,
            // so it is a remote control and the server is told rather than
            // left to guess from a user agent.
            audible: crate::Player::AUDIBLE,
        }));
        self.link = Some(link);
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;

    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::{JsCast, JsValue};
    use web_sys::{js_sys, MessageEvent, WebSocket};

    /// Shared between the socket's callbacks and the caller.
    struct Inbox {
        frames: VecDeque<String>,
        open: bool,
        closed: bool,
    }

    /// The same shape as the engine's browser transport, carrying text rather
    /// than CBOR. Not *reused* from it: `petros::transport::web::Link` is
    /// typed to the engine's own envelopes, and widening it to carry an app's
    /// unrelated protocol would be the transport learning about the app.
    pub struct Wire {
        socket: WebSocket,
        inbox: Rc<RefCell<Inbox>>,
        /// Frames written before the socket finished opening — which is always
        /// the `Hello`, since it is sent in the same breath as the connect.
        backlog: RefCell<Vec<String>>,
        _on_message: Closure<dyn FnMut(MessageEvent)>,
        _on_close: Closure<dyn FnMut(JsValue)>,
        _on_open: Closure<dyn FnMut(JsValue)>,
    }

    impl Wire {
        pub fn connect(url: &str) -> Option<Wire> {
            let socket = WebSocket::new(url).ok()?;
            let inbox = Rc::new(RefCell::new(Inbox {
                frames: VecDeque::new(),
                open: false,
                closed: false,
            }));

            let on_message = {
                let inbox = inbox.clone();
                Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
                    if let Some(text) = e.data().as_string() {
                        inbox.borrow_mut().frames.push_back(text);
                    }
                })
            };
            socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));

            let on_close = {
                let inbox = inbox.clone();
                Closure::<dyn FnMut(JsValue)>::new(move |_| {
                    inbox.borrow_mut().closed = true;
                })
            };
            socket.set_onclose(Some(on_close.as_ref().unchecked_ref()));
            socket.set_onerror(Some(on_close.as_ref().unchecked_ref()));

            let on_open = {
                let inbox = inbox.clone();
                Closure::<dyn FnMut(JsValue)>::new(move |_| {
                    inbox.borrow_mut().open = true;
                })
            };
            socket.set_onopen(Some(on_open.as_ref().unchecked_ref()));

            Some(Wire {
                socket,
                inbox,
                backlog: RefCell::new(Vec::new()),
                _on_message: on_message,
                _on_close: on_close,
                _on_open: on_open,
            })
        }

        pub fn send(&self, text: &str) {
            if !self.is_alive() {
                return;
            }
            if self.inbox.borrow().open {
                self.flush();
                let _ = self.socket.send_with_str(text);
            } else {
                self.backlog.borrow_mut().push(text.to_string());
            }
        }

        pub fn try_recv(&self) -> Option<String> {
            if self.inbox.borrow().open {
                self.flush();
            }
            self.inbox.borrow_mut().frames.pop_front()
        }

        pub fn is_alive(&self) -> bool {
            !self.inbox.borrow().closed
        }

        fn flush(&self) {
            for text in self.backlog.borrow_mut().drain(..) {
                let _ = self.socket.send_with_str(&text);
            }
        }
    }

    impl Drop for Wire {
        fn drop(&mut self) {
            self.socket.set_onmessage(None);
            self.socket.set_onclose(None);
            self.socket.set_onerror(None);
            self.socket.set_onopen(None);
            let _ = self.socket.close();
        }
    }

    pub fn now_ms() -> f64 {
        js_sys::Date::now()
    }

    /// What this device is called in somebody else's picker.
    ///
    /// The page cannot know more than this without a user-agent string, which
    /// would be a guess dressed as a fact. "This browser" is true and the
    /// picker marks which row is you anyway.
    pub fn device_name() -> String {
        "Browser".to_string()
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    /// The desktop has no socket here, for the reason at the top of the file.
    /// `connect` answering `None` is the whole of it: the link stays empty,
    /// `live()` is false, and the bar says so rather than drawing a picker
    /// with nothing in it.
    pub struct Wire;

    impl Wire {
        pub fn connect(_url: &str) -> Option<Wire> {
            None
        }
        pub fn send(&self, _text: &str) {}
        pub fn try_recv(&self) -> Option<String> {
            None
        }
        pub fn is_alive(&self) -> bool {
            false
        }
    }

    pub fn now_ms() -> f64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as f64)
            .unwrap_or(0.0)
    }

    pub fn device_name() -> String {
        "Desktop".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The listening socket is beside the log's, whatever scheme the server
    /// was named with — and the rule for that is `petros_auth`'s, not a second
    /// copy of it.
    #[test]
    fn the_listening_socket_is_beside_the_logs() {
        for (server, want) in [
            ("http://127.0.0.1:8787", "ws://127.0.0.1:8787/listen"),
            (
                "https://harken.example.com",
                "wss://harken.example.com/listen",
            ),
            (
                "https://harken.example.com/",
                "wss://harken.example.com/listen",
            ),
            ("harken.example.com", "ws://harken.example.com/listen"),
        ] {
            assert_eq!(listen_url(server), want, "from {server}");
        }
    }

    /// The three states a device can be in, and the one that is not a
    /// negation of another.
    #[test]
    fn nowhere_playing_is_neither_here_nor_elsewhere() {
        let mut remote = Remote::new();
        remote.open("http://x", "t", "me");
        assert!(!remote.outputs_here());
        assert!(!remote.elsewhere(), "no session at all");

        remote.session = Some(Session {
            output: None,
            devices: Vec::new(),
            queue: Vec::new(),
            at: 0,
            playing: false,
            position_ms: 0,
        });
        assert!(!remote.outputs_here());
        assert!(
            !remote.elsewhere(),
            "nothing is the output, so play here and let the report claim it"
        );

        remote.session.as_mut().unwrap().output = Some("me".into());
        assert!(remote.outputs_here());
        assert!(!remote.elsewhere());

        remote.session.as_mut().unwrap().output = Some("other".into());
        assert!(!remote.outputs_here());
        assert!(remote.elsewhere());
    }

    /// A paused session's clock does not run, and a playing one's does.
    #[test]
    fn the_position_only_counts_forward_while_it_is_playing() {
        let mut remote = Remote::new();
        remote.session = Some(Session {
            output: Some("other".into()),
            devices: Vec::new(),
            queue: Vec::new(),
            at: 0,
            playing: false,
            position_ms: 4_000,
        });
        remote.since = imp::now_ms() - 5_000.0;
        assert_eq!(remote.position_ms(), 4_000, "paused is where it was left");

        remote.session.as_mut().unwrap().playing = true;
        let counted = remote.position_ms();
        assert!(
            (8_900..=9_200).contains(&counted),
            "five seconds on from four, got {counted}"
        );
    }
}
