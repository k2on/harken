//! One listening session per account, and the socket that relays it.
//!
//! `/sync` is the log and this is not: see [`harken::listening`] for why none
//! of what follows is ever written down. What is here is the other half of
//! that decision — a `HashMap` in the server's memory, lost on restart, which
//! is the correct lifetime for "what is playing right now".
//!
//! [`Desk`] is the whole of it and owns no socket: every method takes what
//! happened and delivers what falls out, so the rules — who becomes the
//! output, what a command does when nothing can be heard, what a device
//! leaving means — are tested against channels rather than against a network.
//! That is the engine's own shape, for the engine's own reason.
//!
//! Three rules decide everything:
//!
//! - **Exactly one device is the output**, and only it makes a sound. Every
//!   other device of that account draws what it is told.
//! - **A command goes to the output, not to whoever asked.** That is the
//!   feature: pressing pause on a phone pauses the laptop.
//! - **A device that can be heard and asks for something, when nothing else
//!   is the output, becomes the output.** Otherwise the first tap of the day
//!   would do nothing and there would be a device to pick before any music
//!   could start, which is a setup step for the common case.

use std::collections::HashMap;

use harken::listening::{Command, Device, DeviceId, Hear, Session, Track};
use tokio::sync::mpsc::UnboundedSender;

/// Where one device's frames go.
pub type Wire = UnboundedSender<Hear>;

/// Everything one account is listening to.
struct Room {
    session: Session,
    wires: HashMap<DeviceId, Wire>,
}

/// Every account's session. One of these per server, behind a mutex.
#[derive(Default)]
pub struct Desk {
    rooms: HashMap<String, Room>,
}

impl Desk {
    pub fn new() -> Desk {
        Desk::default()
    }

    /// A device arrives.
    ///
    /// A device id that is already here is a *reconnect* — the same login on
    /// the same device, after a suspended phone or a reloaded tab — so it
    /// replaces its wire rather than appearing twice. Which also means the
    /// output survives a reconnect, and a laptop that blinked does not hand
    /// the music to somebody else.
    pub fn join(&mut self, user: &str, device: Device, wire: Wire) {
        let room = self.rooms.entry(user.to_string()).or_insert_with(|| Room {
            session: Session::default(),
            wires: HashMap::new(),
        });
        room.wires.insert(device.id.clone(), wire);
        room.session.devices.retain(|d| d.id != device.id);
        room.session.devices.push(device);
        room.session.devices.sort_by(|a, b| a.name.cmp(&b.name));
        room.broadcast();
    }

    /// A device goes. If it was the output, nothing is playing anywhere —
    /// which is the truth, and better than a bar that goes on counting for a
    /// laptop that has been shut.
    pub fn leave(&mut self, user: &str, device: &str) {
        let Some(room) = self.rooms.get_mut(user) else {
            return;
        };
        room.wires.remove(device);
        room.session.devices.retain(|d| d.id != device);
        if room.session.outputs(device) {
            room.session.output = None;
            room.session.playing = false;
        }
        if room.wires.is_empty() {
            // Nobody is listening and nothing is playing. Keeping the queue
            // would mean a phone opened tomorrow resumes an afternoon nobody
            // remembers — and the log is where things are kept.
            self.rooms.remove(user);
            return;
        }
        room.broadcast();
    }

    /// The output says what it is doing. From anyone else it is ignored:
    /// a device that is not making the sound cannot be right about it.
    pub fn report(
        &mut self,
        user: &str,
        from: &str,
        queue: Vec<Track>,
        at: usize,
        playing: bool,
        position_ms: i64,
    ) {
        let Some(room) = self.rooms.get_mut(user) else {
            return;
        };
        room.claim(from);
        if !room.session.outputs(from) {
            return;
        }
        let was = room.session.clone();
        room.session.queue = queue;
        room.session.at = at;
        room.session.playing = playing;
        room.session.position_ms = position_ms;
        // A report a second means a broadcast a second, and the position it
        // carries is the only thing that changed — which is exactly what the
        // other devices' scrubbers are waiting for, so it is still worth
        // sending. What is not worth sending is a report that changed nothing
        // at all, which is what a paused output sends.
        if was != room.session {
            room.broadcast();
        }
    }

    /// Somebody asks for something. It goes to the output.
    pub fn command(&mut self, user: &str, from: &str, command: Command) {
        let Some(room) = self.rooms.get_mut(user) else {
            return;
        };
        let before = room.session.output.clone();
        room.claim(from);
        let Some(output) = room.session.output.clone() else {
            // Nothing can be heard: no output, and the asker is a remote
            // control. Say so by broadcasting rather than by silence — the
            // bar that asked is the bar that has to show it did not happen.
            room.broadcast();
            return;
        };
        if room.session.output != before {
            room.broadcast();
        }
        room.tell(&output, Hear::Do { command });
    }

    /// Move the sound. `None` stops it everywhere.
    ///
    /// The new output is handed the session as one [`Command::Start`] — the
    /// queue, the place in it, the point in the track and whether it was
    /// playing — because a hand-off is that sentence and nothing else. The old
    /// one is told nothing: it learns from the broadcast that it is no longer
    /// the output, and a client that is not the output is silent. One rule, in
    /// one place, rather than a stop command that a dropped socket could lose.
    pub fn transfer(&mut self, user: &str, to: Option<DeviceId>) {
        let Some(room) = self.rooms.get_mut(user) else {
            return;
        };
        match to {
            None => {
                room.session.output = None;
                room.session.playing = false;
            }
            Some(id) => {
                // Only a device that can be heard, and only one that is here.
                if !room.session.devices.iter().any(|d| d.id == id && d.audible) {
                    room.broadcast();
                    return;
                }
                if room.session.outputs(&id) {
                    return;
                }
                room.session.output = Some(id.clone());
                let take = Command::Start {
                    queue: room.session.queue.clone(),
                    at: room.session.at,
                    position_ms: room.session.position_ms,
                    playing: room.session.playing,
                };
                room.tell(&id, Hear::Do { command: take });
            }
        }
        room.broadcast();
    }

    /// What one account's session is, for a test or a health page.
    pub fn session(&self, user: &str) -> Option<&Session> {
        self.rooms.get(user).map(|r| &r.session)
    }

    /// How many accounts are listening.
    pub fn rooms(&self) -> usize {
        self.rooms.len()
    }
}

impl Room {
    /// Let `device` take the sound if nothing else has it and it can be
    /// heard. A no-op otherwise — including when `device` is already the
    /// output, which is the common case.
    fn claim(&mut self, device: &str) {
        if self.session.output.is_some() {
            return;
        }
        if self
            .session
            .devices
            .iter()
            .any(|d| d.id == device && d.audible)
        {
            self.session.output = Some(device.to_string());
        }
    }

    fn tell(&mut self, device: &str, msg: Hear) {
        if let Some(wire) = self.wires.get(device) {
            let _ = wire.send(msg);
        }
    }

    /// The session, entire, to everybody. A closed wire is a device that has
    /// gone and not yet been reaped; `leave` does the reaping.
    fn broadcast(&mut self) {
        let msg = Hear::State {
            session: self.session.clone(),
        };
        for wire in self.wires.values() {
            let _ = wire.send(msg.clone());
        }
    }
}

/// The socket at `/listen`, and the one axum-shaped thing in this file.
///
/// The token is in the first frame rather than in a header or the query, for
/// the reason `petros-axum` puts it there: a browser's `WebSocket` cannot set
/// a header, and a token in a URL is a token in an access log. Until that
/// frame arrives this connection is nobody and is told nothing.
///
/// One task per socket rather than two. `petros-axum` splits its socket
/// because the engine's frames can queue up behind a catch-up of the whole
/// log; nothing here is ever more than a few hundred bytes, so a `select!`
/// over the socket and the mailbox is the smaller thing that does the same
/// job — and the mailbox is unbounded, so the desk never blocks on a device
/// that is slow to read.
pub mod route {
    use std::sync::{Arc, Mutex};

    use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
    use axum::extract::State;
    use axum::response::Response;
    use harken::listening::{decode, encode, Device, Hear, Say};
    use petros_auth::server::Auth;
    use tokio::sync::mpsc::unbounded_channel;

    use super::Desk;

    /// What the route reads: who a token proves, and every account's session.
    #[derive(Clone)]
    pub struct Listening {
        pub auth: Arc<Auth>,
        pub desk: Arc<Mutex<Desk>>,
    }

    pub async fn listen(State(state): State<Listening>, upgrade: WebSocketUpgrade) -> Response {
        upgrade.on_upgrade(move |socket| serve(socket, state))
    }

    async fn serve(mut socket: WebSocket, state: Listening) {
        let (tx, mut rx) = unbounded_channel::<Hear>();
        // Who this socket turned out to be: the account, and which of its
        // devices. `None` until the `Hello`, and the reason everything below
        // has two cases.
        let mut who: Option<(String, String)> = None;

        loop {
            tokio::select! {
                // Something for this device. A closed socket ends the loop
                // rather than the send failing quietly.
                Some(msg) = rx.recv() => {
                    if socket.send(Message::Text(encode(&msg).into())).await.is_err() {
                        break;
                    }
                    if matches!(msg, Hear::Denied { .. }) {
                        break;
                    }
                }
                frame = socket.recv() => {
                    let Some(Ok(Message::Text(text))) = frame else {
                        // A close, an error, or a frame this socket does not
                        // carry: binary is the log's socket and a ping answers
                        // itself.
                        match frame {
                            Some(Ok(_)) => continue,
                            _ => break,
                        }
                    };
                    let Some(say) = decode::<Say>(&text) else { continue };
                    match (say, who.clone()) {
                        (Say::Hello { token, device, name, audible }, None) => {
                            let Some(login) = state.auth.whoami(&token) else {
                                let _ = tx.send(Hear::Denied {
                                    reason: "that login is not live".into(),
                                });
                                continue;
                            };
                            let user = login.user.id.clone();
                            if let Ok(mut desk) = state.desk.lock() {
                                desk.join(
                                    &user,
                                    Device { id: device.clone(), name, audible },
                                    tx.clone(),
                                );
                            }
                            who = Some((user, device));
                        }
                        // Anything before the `Hello` is from nobody, and a
                        // second `Hello` is a socket changing who it is
                        // halfway through. Neither is a thing to answer.
                        (_, None) | (Say::Hello { .. }, Some(_)) => break,
                        (say, Some((user, device))) => {
                            let Ok(mut desk) = state.desk.lock() else { break };
                            match say {
                                Say::Hello { .. } => unreachable!("matched above"),
                                Say::Report { queue, at, playing, position_ms } => {
                                    desk.report(&user, &device, queue, at, playing, position_ms)
                                }
                                Say::Do { command } => desk.command(&user, &device, command),
                                Say::Transfer { to } => desk.transfer(&user, to),
                            }
                        }
                    }
                }
                else => break,
            }
        }

        // However this ended — a close, a reload, a phone going to sleep — the
        // device is gone, and if it was the output then so is the sound.
        if let Some((user, device)) = who {
            if let Ok(mut desk) = state.desk.lock() {
                desk.leave(&user, &device);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harken::Id;
    use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

    fn device(id: &str, audible: bool) -> Device {
        Device {
            id: id.into(),
            name: id.into(),
            audible,
        }
    }

    fn track(title: &str) -> Track {
        Track {
            id: Id::default(),
            title: title.into(),
            creator: "Bach".into(),
            album: String::new(),
            duration_ms: 60_000,
            file: "music/a.mp3".into(),
        }
    }

    /// Everything a device heard since it was last asked.
    fn drain(rx: &mut UnboundedReceiver<Hear>) -> Vec<Hear> {
        let mut out = Vec::new();
        while let Ok(msg) = rx.try_recv() {
            out.push(msg);
        }
        out
    }

    /// The last session a device was told about.
    fn latest(rx: &mut UnboundedReceiver<Hear>) -> Session {
        drain(rx)
            .into_iter()
            .filter_map(|m| match m {
                Hear::State { session } => Some(session),
                _ => None,
            })
            .next_back()
            .expect("a state")
    }

    fn join(desk: &mut Desk, user: &str, id: &str, audible: bool) -> UnboundedReceiver<Hear> {
        let (tx, rx) = unbounded_channel();
        desk.join(user, device(id, audible), tx);
        rx
    }

    /// Two devices of one account see one session; a third account sees none
    /// of it.
    #[test]
    fn a_session_belongs_to_an_account_and_not_to_a_device() {
        let mut desk = Desk::new();
        let mut phone = join(&mut desk, "alice", "phone", true);
        let mut laptop = join(&mut desk, "alice", "laptop", true);
        let mut bob = join(&mut desk, "bob", "phone", true);

        // The laptop arriving is news to the phone, which was already here.
        let seen = latest(&mut phone);
        assert_eq!(seen.devices.len(), 2);
        assert_eq!(latest(&mut laptop).devices.len(), 2);
        assert_eq!(latest(&mut bob).devices.len(), 1, "a different account");

        desk.report("alice", "phone", vec![track("Air")], 0, true, 1_500);
        let seen = latest(&mut laptop);
        assert_eq!(seen.now().map(|t| t.title.as_str()), Some("Air"));
        assert_eq!(seen.output.as_deref(), Some("phone"));
        assert!(drain(&mut bob).is_empty(), "not bob's session");
    }

    /// The whole feature in four lines: the laptop presses pause, the phone is
    /// the one told to do it.
    #[test]
    fn a_command_goes_to_the_output_and_not_to_whoever_asked() {
        let mut desk = Desk::new();
        let mut phone = join(&mut desk, "alice", "phone", true);
        let mut laptop = join(&mut desk, "alice", "laptop", false);
        desk.report("alice", "phone", vec![track("Air")], 0, true, 0);
        drain(&mut phone);
        drain(&mut laptop);

        desk.command("alice", "laptop", Command::Pause);
        assert_eq!(
            drain(&mut phone),
            [Hear::Do {
                command: Command::Pause
            }]
        );
        assert!(
            !drain(&mut laptop)
                .iter()
                .any(|m| matches!(m, Hear::Do { .. })),
            "the asker is not the doer"
        );
    }

    /// Nothing to be heard: a remote control asking for music when no output
    /// exists gets an answer rather than silence, and does not become one.
    #[test]
    fn a_device_that_cannot_be_heard_never_becomes_the_output() {
        let mut desk = Desk::new();
        let mut laptop = join(&mut desk, "alice", "laptop", false);
        drain(&mut laptop);

        desk.command("alice", "laptop", Command::Play);
        assert_eq!(desk.session("alice").unwrap().output, None);
        // Told, not ignored: the bar has to be able to show that the press
        // went nowhere.
        assert!(matches!(
            drain(&mut laptop).as_slice(),
            [Hear::State { .. }]
        ));

        // And a report from it is not believed either.
        desk.report("alice", "laptop", vec![track("Air")], 0, true, 0);
        assert!(desk.session("alice").unwrap().queue.is_empty());
    }

    /// The first device to ask for something, when nothing is playing
    /// anywhere, is the one that plays it. Without this every session would
    /// start by picking a device.
    #[test]
    fn asking_first_is_how_a_device_becomes_the_output() {
        let mut desk = Desk::new();
        let mut phone = join(&mut desk, "alice", "phone", true);
        drain(&mut phone);

        desk.command("alice", "phone", Command::Play);
        assert_eq!(
            desk.session("alice").unwrap().output.as_deref(),
            Some("phone")
        );
        assert!(drain(&mut phone).iter().any(|m| matches!(
            m,
            Hear::Do {
                command: Command::Play
            }
        )));
    }

    /// Moving the sound hands over the queue, the place in it and the point in
    /// the track — one command, so the new output can simply do it.
    #[test]
    fn a_transfer_hands_over_the_whole_session() {
        let mut desk = Desk::new();
        let mut phone = join(&mut desk, "alice", "phone", true);
        let mut speaker = join(&mut desk, "alice", "speaker", true);
        desk.report(
            "alice",
            "phone",
            vec![track("Air"), track("Gigue")],
            1,
            true,
            12_345,
        );
        drain(&mut phone);
        drain(&mut speaker);

        desk.transfer("alice", Some("speaker".into()));
        let told: Vec<Command> = drain(&mut speaker)
            .into_iter()
            .filter_map(|m| match m {
                Hear::Do { command } => Some(command),
                _ => None,
            })
            .collect();
        assert_eq!(
            told,
            [Command::Start {
                queue: vec![track("Air"), track("Gigue")],
                at: 1,
                position_ms: 12_345,
                playing: true,
            }]
        );
        // The old output is told nothing at all; it reads the state and goes
        // quiet, which is the one rule rather than a second command.
        assert!(
            !drain(&mut phone)
                .iter()
                .any(|m| matches!(m, Hear::Do { .. })),
            "the old output is not commanded, it is informed"
        );
        assert_eq!(
            desk.session("alice").unwrap().output.as_deref(),
            Some("speaker")
        );
    }

    /// A device that cannot be heard cannot be picked either, and asking is
    /// not an error — the picker simply does not move.
    #[test]
    fn the_sound_cannot_be_moved_to_something_that_cannot_make_it() {
        let mut desk = Desk::new();
        let mut phone = join(&mut desk, "alice", "phone", true);
        let _laptop = join(&mut desk, "alice", "laptop", false);
        desk.report("alice", "phone", vec![track("Air")], 0, true, 0);
        drain(&mut phone);

        desk.transfer("alice", Some("laptop".into()));
        assert_eq!(
            desk.session("alice").unwrap().output.as_deref(),
            Some("phone")
        );
        desk.transfer("alice", Some("nobody-here".into()));
        assert_eq!(
            desk.session("alice").unwrap().output.as_deref(),
            Some("phone")
        );
    }

    /// Stopping everywhere is a transfer to nobody.
    #[test]
    fn transferring_to_nobody_stops_the_music() {
        let mut desk = Desk::new();
        let mut phone = join(&mut desk, "alice", "phone", true);
        let _laptop = join(&mut desk, "alice", "laptop", false);
        desk.report("alice", "phone", vec![track("Air")], 0, true, 0);

        desk.transfer("alice", None);
        let seen = latest(&mut phone);
        assert_eq!(seen.output, None);
        assert!(!seen.playing);
        // The queue survives, so picking a device again resumes rather than
        // starting from an empty bar.
        assert_eq!(seen.queue.len(), 1);
    }

    /// The output going away is the session stopping, not the session
    /// continuing somewhere nobody can hear.
    #[test]
    fn the_output_leaving_stops_the_session() {
        let mut desk = Desk::new();
        let mut phone = join(&mut desk, "alice", "phone", true);
        let mut laptop = join(&mut desk, "alice", "laptop", false);
        desk.report("alice", "phone", vec![track("Air")], 0, true, 4_000);
        drain(&mut laptop);
        drain(&mut phone);

        desk.leave("alice", "phone");
        let seen = latest(&mut laptop);
        assert_eq!(seen.output, None);
        assert!(!seen.playing);
        assert_eq!(seen.devices.len(), 1);
        assert!(drain(&mut phone).is_empty(), "it has gone");

        // The last device out takes the room with it.
        desk.leave("alice", "laptop");
        assert!(desk.session("alice").is_none());
        assert_eq!(desk.rooms(), 0);
    }

    /// A reloaded tab is the same device, so it does not appear twice and does
    /// not lose the sound.
    #[test]
    fn a_reconnect_replaces_a_device_rather_than_adding_one() {
        let mut desk = Desk::new();
        let mut phone = join(&mut desk, "alice", "phone", true);
        desk.report("alice", "phone", vec![track("Air")], 0, true, 0);
        drop(phone);
        phone = join(&mut desk, "alice", "phone", true);

        let seen = latest(&mut phone);
        assert_eq!(seen.devices.len(), 1);
        assert_eq!(seen.output.as_deref(), Some("phone"));
        assert_eq!(seen.now().map(|t| t.title.as_str()), Some("Air"));
    }
}
