//! This device's end of the account's listening session.
//!
//! **It has no socket.** It used to: a second `WebSocket` at `/listen`,
//! carrying JSON, dialled and retried beside the log's. That was a second
//! thing to authenticate, a second thing to reconnect, a second thing to keep
//! alive through a proxy, and a second answer to "am I online" — and the two
//! disagreed at the worst possible moment, which on a laptop waking up is
//! every morning. It rides `/sync` now, as a `petros::live` room: what goes
//! out is [`petros::Client::say`] and what comes back is
//! [`petros::Client::heard`], on the same wire, through the same sign-in, with
//! the same keepalive under it. See [`harken::listening`].
//!
//! What is left here is a *state machine*: the session as the server last
//! described it, an outbox the pump drains, and the two questions the rest of
//! the program asks.
//!
//! **Am I the output?** If not, this build makes no sound and its transport
//! buttons are sent rather than obeyed. That is the whole rule, and it lives
//! here so that [`crate::App`] has one thing to ask rather than a condition to
//! remember at each button.
//!
//! **The desktop is in the session now**, which is the other thing losing the
//! second socket bought. This module used to be browser-only — not because a
//! desktop has nothing to say, but because `/listen` would have needed a
//! native WebSocket client and this workspace had none. The log's socket has
//! been native all along, so being a *remote control* costs nothing now:
//! `Player::AUDIBLE` is still false there, so it can never be the output, and
//! it can pause the phone.
//!
//! **A device is a login.** `petros-auth` says a session is "one login on one
//! device", which is exactly the identity this wants and already exists — so
//! the device id *is* the login's session id. It is also what the engine
//! already puts on a room's peer, which means this device never says its own
//! id: the server knows who is talking to it.

use harken::listening::{Hear, Kind, Say};
use petros::Client;
// Re-exported rather than re-imported at the call site: the rest of this
// program asks *this* module about the session, and a second import path to
// the same types is a second place to look.
pub use harken::listening::{Command, Device, DeviceId, Session, Track};

/// How far the position may drift before the output says so again.
///
/// One number doing two jobs, which is why it is one rule rather than a
/// heartbeat plus a seek test: while playing, the position advances past this
/// about once a second, so it *is* the heartbeat; while paused it never moves,
/// so a paused output is silent; and a seek crosses it at once however long
/// the last report was ago.
const DRIFT_MS: i64 = 1_100;

/// What was last reported, so that a quiet second costs nothing.
#[derive(Debug, Clone, PartialEq)]
struct Said {
    at: u32,
    playing: bool,
    len: usize,
    position_ms: i64,
}

/// This device's end of the account's listening session.
#[derive(Default)]
pub struct Remote {
    /// This device, which is the login's session id. Held only so the picker
    /// can mark which row is you; nothing is ever sent with it in.
    me: DeviceId,
    /// What the picker draws for it.
    name: String,
    /// The session as the server last described it, and when that was by this
    /// machine's clock.
    ///
    /// *This* machine's, deliberately: the server has a clock and so does
    /// every device and they do not agree, so a scrubber that has to move
    /// between reports counts from when the state arrived here.
    session: Option<Session>,
    since: f64,
    said: Option<Said>,
    /// What to say on the next pump. An outbox rather than a client handed to
    /// every caller: a button knows what it wants, not where the socket is.
    out: Vec<Say>,
    /// Which connection this device last introduced itself on. A room is the
    /// server's memory of a socket, so a new socket is a room that has never
    /// heard of this device — and the engine counts connections for exactly
    /// this.
    epoch: u64,
}

impl Remote {
    pub fn new() -> Remote {
        Remote {
            name: device_name(),
            ..Remote::default()
        }
    }

    /// This device is a login, so a new login is a new device. Called at
    /// sign-in and whenever the login changes.
    pub fn open(&mut self, device: &str) {
        self.me = device.to_string();
        self.session = None;
        self.said = None;
        self.out.clear();
        self.epoch = 0;
    }

    /// Stop. A peer that has signed out has no session to be part of.
    pub fn close(&mut self) {
        self.session = None;
        self.said = None;
        self.out.clear();
        self.epoch = 0;
    }

    /// Whether there is a session at all — which is what decides whether the
    /// bar has a device picker to draw.
    pub fn live(&self) -> bool {
        self.session.is_some()
    }

    pub fn session(&self) -> Option<&Session> {
        self.session.as_ref()
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

    /// Whether a hand-off is in flight, and to where.
    ///
    /// A speaker in the house takes a second or two to fetch anything, so the
    /// picker draws this row as *connecting* rather than as the one playing —
    /// which is the difference between a press that appears to have done
    /// nothing and one that is visibly under way.
    pub fn moving(&self) -> Option<&str> {
        self.session.as_ref()?.moving.as_deref()
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
        session.position_ms + (now_ms() - self.since).max(0.0) as i64
    }

    /// Move whatever is waiting in each direction, over the log's own socket.
    ///
    /// Returns what this device has been told to *do*, which the server only
    /// ever sends to the output.
    pub fn pump(&mut self, client: &mut Client<harken::HarkenApp>) -> Vec<Command> {
        if !client.linked() {
            // The room is the server's memory of a socket. With no socket
            // there is no room, and drawing the last thing it said would be a
            // picker full of devices nobody can reach.
            self.session = None;
            self.said = None;
            self.out.clear();
            self.epoch = 0;
            return Vec::new();
        }
        if self.epoch != client.epoch() {
            self.epoch = client.epoch();
            // A fresh connection is a room that has never heard of this
            // device, so it says what it is again — and forgets what it last
            // reported, because nobody over there remembers hearing it.
            self.said = None;
            self.out.insert(
                0,
                Say::Here {
                    name: self.name.clone(),
                    // What this build can actually do. The desktop cannot be
                    // heard, so it is a remote control and the server is told
                    // rather than left to guess from a user agent.
                    audible: crate::Player::AUDIBLE,
                    kind: Kind::Computer,
                },
            );
        }
        for say in self.out.drain(..) {
            let _ = client.say(&say);
        }
        let mut todo = Vec::new();
        for hear in client.heard::<Hear>() {
            match hear {
                Hear::State { session } => {
                    self.session = Some(session);
                    self.since = now_ms();
                }
                Hear::Do { command } => todo.push(command),
            }
        }
        todo
    }

    /// Say what this device is doing, if it is the one doing it.
    ///
    /// Called from the tick rather than from each button, for the reason the
    /// media session's announcement is: the element pauses itself when a
    /// stream stalls or runs out, and that was nobody's button press.
    pub fn report(&mut self, queue: &[Track], at: u32, playing: bool, position_ms: i64) {
        if self.elsewhere() || self.session.is_none() {
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
        self.out.push(Say::Report {
            queue: queue.to_vec(),
            at,
            playing,
            position_ms,
        });
    }

    /// Ask for something, wherever the sound is.
    pub fn ask(&mut self, command: Command) {
        self.assume(&command);
        self.out.push(Say::Do { command });
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
        self.since = now_ms();
    }

    /// Move the sound, or stop it everywhere with `None`.
    pub fn transfer(&mut self, to: Option<DeviceId>) {
        // The session about to be handed over is the one this device last
        // reported; forgetting that here means the next report says it again,
        // which is what a device that has just *lost* the output should not do.
        self.said = None;
        self.out.push(Say::Transfer { to });
    }
}

#[cfg(target_arch = "wasm32")]
fn now_ms() -> f64 {
    web_sys::js_sys::Date::now()
}

#[cfg(not(target_arch = "wasm32"))]
fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}

/// What this device is called in somebody else's picker.
///
/// No more than the build, deliberately: more than this would want a
/// user-agent string, which is a guess dressed as a fact. The picker marks
/// which row is you anyway.
fn device_name() -> String {
    if cfg!(target_arch = "wasm32") {
        "Browser".to_string()
    } else {
        "Desktop".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(output: Option<&str>) -> Session {
        Session {
            output: output.map(str::to_string),
            ..Session::default()
        }
    }

    /// The three states a device can be in, and the one that is not a
    /// negation of another.
    #[test]
    fn nowhere_playing_is_neither_here_nor_elsewhere() {
        let mut remote = Remote::new();
        remote.open("me");
        assert!(!remote.outputs_here());
        assert!(!remote.elsewhere(), "no session at all");

        remote.session = Some(session(None));
        assert!(!remote.outputs_here());
        assert!(
            !remote.elsewhere(),
            "nothing is the output, so play here and let the report claim it"
        );

        remote.session = Some(session(Some("me")));
        assert!(remote.outputs_here());
        assert!(!remote.elsewhere());

        remote.session = Some(session(Some("other")));
        assert!(!remote.outputs_here());
        assert!(remote.elsewhere());
    }

    /// A paused session's clock does not run, and a playing one's does.
    #[test]
    fn the_position_only_counts_forward_while_it_is_playing() {
        let mut remote = Remote::new();
        remote.session = Some(Session {
            output: Some("other".into()),
            position_ms: 4_000,
            ..Session::default()
        });
        remote.since = now_ms() - 5_000.0;
        assert_eq!(remote.position_ms(), 4_000, "paused is where it was left");

        remote.session.as_mut().unwrap().playing = true;
        let counted = remote.position_ms();
        assert!(
            (8_900..=9_200).contains(&counted),
            "five seconds on from four, got {counted}"
        );
    }

    /// A report that says what the last one said is not sent. The output
    /// speaks about once a second while playing, never while paused, and at
    /// once on a seek — three behaviours from one number.
    #[test]
    fn one_number_decides_when_the_output_speaks() {
        let mut remote = Remote::new();
        remote.open("me");
        remote.session = Some(session(Some("me")));

        remote.report(&[], 0, true, 0);
        assert_eq!(remote.out.len(), 1, "the first word is always worth saying");
        remote.out.clear();

        remote.report(&[], 0, true, 900);
        assert!(remote.out.is_empty(), "under a second of drift is nothing");

        remote.report(&[], 0, true, 1_200);
        assert_eq!(remote.out.len(), 1, "…and over it is the heartbeat");
        remote.out.clear();

        // A seek backwards is the same rule from the other side, which is why
        // the comparison is an absolute value and not a subtraction.
        remote.report(&[], 0, true, 0);
        assert_eq!(remote.out.len(), 1, "a seek crosses it at once");
    }

    /// A device that is not making the sound does not describe it. Reporting
    /// is what claims the output, so a tab left open on somebody's desk must
    /// not take the music from the phone in their pocket.
    #[test]
    fn a_device_that_is_not_the_output_says_nothing_about_it() {
        let mut remote = Remote::new();
        remote.open("me");
        remote.session = Some(session(Some("other")));
        remote.report(&[], 0, true, 0);
        assert!(remote.out.is_empty());
    }
}
