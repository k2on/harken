//! One listening session per account, as a `petros::live` room.
//!
//! `/sync` carries the log and this rides beside it: see [`harken::listening`]
//! for why none of what follows is ever written down, and `petros::live` for
//! how a channel that is not the log works. What is here is the other half of
//! that decision — the rules, as a state machine over values.
//!
//! [`Desk`] owns no socket, which is the engine's own shape for the engine's
//! own reason: every method takes what happened and delivers what falls out,
//! so who ends up with the sound, what a button does when the speaker is
//! unplugged, and what a closing laptop means are all tested against values
//! rather than against a network.
//!
//! Four rules decide everything, and the second is the one that changed:
//!
//! - **Exactly one device is the output**, and only it makes a sound. Every
//!   other device of that account draws what it is told.
//! - **The output survives its socket.** A laptop lid closing, a phone going
//!   to sleep and a tab being reloaded are not decisions to move the music.
//!   The sound still belongs to that device; it is simply not answering, and
//!   the session says so rather than handing itself to whoever asks next.
//! - **A command goes to the output, not to whoever asked.** That is the
//!   feature: pressing pause on a phone pauses the laptop.
//! - **…unless the output cannot be reached, and the asker can make a
//!   sound.** Then the asker takes it — because somebody pressed play and
//!   there is nothing else in the house that can answer. This is the only way
//!   a device takes the sound without being picked, and it needs a press: a
//!   device that merely arrives takes nothing.

use std::collections::BTreeMap;

use harken::listening::{Command, Device, DeviceId, Hear, Kind, Say, Session, Track};
use petros::live::{Live, Peer, Post, Room};
use tokio::sync::mpsc::UnboundedSender;

/// What a bridge standing in rooms needs to know: which rooms have somebody
/// listening in them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Watch {
    /// Somebody is listening as this account. Offer it whatever stands in
    /// every room.
    Open(Room),
    /// Nobody is, any more. Take those back out, so the room can empty, be
    /// written down and be let go — a bridge standing in every room would
    /// mean no room was ever empty.
    Shut(Room),
}

/// Every account's session.
///
/// One of these per server, handed to the hub as its realtime machine. The
/// rooms are keyed the way `petros::live` keys them, which is by account.
#[derive(Default)]
pub struct Desk {
    rooms: BTreeMap<Room, Session>,
    /// Which rooms have been announced as open, so that a second device
    /// arriving is not a second announcement.
    listening: std::collections::BTreeSet<Room>,
    /// Told whenever that changes. A channel rather than a call, deliberately:
    /// whatever answers this will call back into the hub to stand a device,
    /// and a hub that called it inline would be a hub calling itself with its
    /// own lock held.
    watchers: Vec<UnboundedSender<Watch>>,
}

/// What survives an empty room, and therefore a restart.
///
/// Not the device list: who is connected is a fact about now and every entry
/// in it would come back false. The one device kept is the one the sound
/// belongs to, because "your kitchen speaker, which is not answering" is
/// worth drawing and an id with no name is not.
#[derive(serde::Serialize, serde::Deserialize)]
struct Kept {
    output: Option<DeviceId>,
    out_device: Option<Device>,
    queue: Vec<Track>,
    at: u32,
    position_ms: i64,
}

impl Desk {
    pub fn new() -> Desk {
        Desk::default()
    }

    /// Be told when a room opens and when it closes. The rooms that already
    /// have somebody in them come back at once, so a bridge registered late
    /// is not a bridge that missed everyone.
    pub fn watch(&mut self, tx: UnboundedSender<Watch>) {
        for room in &self.listening {
            if tx.send(Watch::Open(room.clone())).is_err() {
                return;
            }
        }
        self.watchers.push(tx);
    }

    /// What one account's session is, for a test or a health page.
    pub fn session(&self, room: &str) -> Option<&Session> {
        self.rooms.get(room)
    }

    pub fn rooms(&self) -> usize {
        self.rooms.len()
    }

    fn tell_watchers(&mut self, news: Watch) {
        self.watchers.retain(|w| w.send(news.clone()).is_ok());
    }

    /// Whether anybody is *listening* here, as opposed to standing here. A
    /// speaker is in the session when somebody is listening, not the other way
    /// round.
    fn anyone_listening(session: &Session) -> bool {
        session
            .devices
            .iter()
            .any(|d| d.here && d.kind != Kind::Speaker)
    }

    /// Say whether the room is open, if that has changed since last time.
    fn reconsider(&mut self, room: &Room) {
        let open = self.rooms.get(room).is_some_and(Self::anyone_listening);
        if open && self.listening.insert(room.clone()) {
            self.tell_watchers(Watch::Open(room.clone()));
        } else if !open && self.listening.remove(room) {
            self.tell_watchers(Watch::Shut(room.clone()));
        }
    }
}

impl Live for Desk {
    type Say = Say;
    type Hear = Hear;

    /// A socket opened. Nothing is claimed and nothing is announced: the
    /// device has not said what it is yet, and — the rule this file exists
    /// for — *arriving* is not how a device gets the sound.
    ///
    /// It is told the session at once, though, so a tab that has just opened
    /// draws what the house is doing rather than an empty bar it will fill in
    /// a moment.
    fn join(&mut self, peer: &Peer, post: &mut Post<'_, Hear>) {
        let session = self.rooms.entry(peer.room.clone()).or_default();
        // A reconnect: the same device, which may well still be the output.
        if let Some(device) = session.devices.iter_mut().find(|d| d.id == peer.who) {
            device.here = true;
        }
        let session = session.clone();
        post.tell(&peer.who, Hear::State { session });
    }

    fn say(&mut self, peer: &Peer, say: Say, post: &mut Post<'_, Hear>) {
        match say {
            Say::Here {
                name,
                audible,
                kind,
            } => self.here(peer, name, audible, kind, post),
            Say::Report {
                queue,
                at,
                playing,
                position_ms,
            } => self.report(peer, queue, at, playing, position_ms, post),
            Say::Do { command } => self.command(peer, command, post),
            Say::Transfer { to } => self.transfer(peer, to, post),
        }
    }

    /// A socket closed. The device is marked away and the sound stays where
    /// it was — which is the whole of the fix for music jumping back to
    /// whichever device happened to be looking.
    fn part(&mut self, peer: &Peer, post: &mut Post<'_, Hear>) {
        let Some(session) = self.rooms.get_mut(&peer.room) else {
            return;
        };
        if session.moving_to(&peer.who) {
            // It was given the sound and never took it. Somebody has to be
            // told that stopped being true.
            session.moving = None;
        }
        if session.outputs(&peer.who) {
            // It still owns the sound. What it cannot be is *playing*: there
            // is nothing on the other end of that socket to be making one.
            session.playing = false;
            if let Some(device) = session.devices.iter_mut().find(|d| d.id == peer.who) {
                device.here = false;
            }
        } else {
            // Anything else that has gone is simply gone. Keeping it would
            // fill the picker with every tab anyone ever opened.
            session.devices.retain(|d| d.id != peer.who);
        }
        let session = session.clone();
        post.tell_room(Hear::State { session });
        post.keep();
        self.reconsider(&peer.room);
    }

    fn snapshot(&mut self, room: &Room) -> Option<Vec<u8>> {
        let session = self.rooms.get(room)?;
        // A room with nothing playing and nowhere for it to play is a room
        // worth no disk at all — and saying so deletes the row rather than
        // leaving a stale one.
        if session.output.is_none() && session.queue.is_empty() {
            return None;
        }
        petros::encode(&Kept {
            output: session.output.clone(),
            out_device: session.output_device().cloned(),
            queue: session.queue.clone(),
            at: session.at,
            position_ms: session.position_ms,
        })
        .ok()
    }

    /// Yesterday's session, from the disk. Paused, and with every device
    /// away: what was true is where it was playing, never that it is playing.
    fn wake(&mut self, room: &Room, snapshot: &[u8]) {
        let Ok(kept) = petros::decode::<Kept>(snapshot) else {
            return;
        };
        let devices = kept
            .out_device
            .into_iter()
            .map(|d| Device { here: false, ..d })
            .collect();
        self.rooms.insert(
            room.clone(),
            Session {
                output: kept.output,
                moving: None,
                devices,
                queue: kept.queue,
                at: kept.at,
                playing: false,
                position_ms: kept.position_ms,
            },
        );
    }

    fn close(&mut self, room: &Room) {
        self.rooms.remove(room);
        self.listening.remove(room);
    }
}

impl Desk {
    /// A device says what it is. This is where it enters the picker, and
    /// where a reconnecting output is recognised as the device that still
    /// owns the sound.
    fn here(
        &mut self,
        peer: &Peer,
        name: String,
        audible: bool,
        kind: Kind,
        post: &mut Post<'_, Hear>,
    ) {
        let Some(session) = self.rooms.get_mut(&peer.room) else {
            return;
        };
        let device = Device {
            id: peer.who.clone(),
            name,
            audible,
            here: true,
            kind,
        };
        session.devices.retain(|d| d.id != device.id);
        session.devices.push(device);
        session.devices.sort_by(|a, b| a.name.cmp(&b.name));
        let session = session.clone();
        post.tell_room(Hear::State { session });
        post.keep();
        self.reconsider(&peer.room);
    }

    /// The output says what it is doing. From anyone else it is ignored: a
    /// device that is not making the sound cannot be right about it.
    fn report(
        &mut self,
        peer: &Peer,
        queue: Vec<Track>,
        at: u32,
        playing: bool,
        position_ms: i64,
        post: &mut Post<'_, Hear>,
    ) {
        let Some(session) = self.rooms.get_mut(&peer.room) else {
            return;
        };
        if !session.outputs(&peer.who) {
            return;
        }
        let was = session.clone();
        // It has taken the hand-off. A report is the only evidence of that
        // there can be, because taking it is exactly "started playing".
        if session.moving_to(&peer.who) {
            session.moving = None;
        }
        let structural = session.queue != queue || session.at != at || session.playing != playing;
        session.queue = queue;
        session.at = at;
        session.playing = playing;
        session.position_ms = position_ms;
        // A report a second means a broadcast a second, and the position it
        // carries is the only thing that changed — which is exactly what the
        // other devices' scrubbers are waiting for, so it is still worth
        // sending. What is not worth sending is a report that changed nothing
        // at all, which is what a paused output sends.
        if was != *session {
            let session = session.clone();
            post.tell_room(Hear::State { session });
        }
        // A position that moved is not worth a disk write; a different track
        // is. This is the difference `Post::keep` exists to let an app draw.
        if structural {
            post.keep();
        }
    }

    /// Somebody asks for something.
    fn command(&mut self, peer: &Peer, command: Command, post: &mut Post<'_, Hear>) {
        let Some(session) = self.rooms.get_mut(&peer.room) else {
            return;
        };

        // The ordinary case, and the feature: the sound is somewhere that is
        // answering, so that is where the button goes.
        if let Some(output) = session.output.clone() {
            if post.here(&output) {
                post.tell(&output, Hear::Do { command });
                return;
            }
        }

        // Nothing is answering where the sound belongs. Only a press that
        // means "make a sound" moves it — a pause or a seek aimed at a device
        // that has gone is a press with nothing to do, and answering it by
        // seizing the sound would be a device assuming control it was never
        // given.
        let wants_sound = matches!(
            command,
            Command::Play | Command::Next | Command::Previous | Command::Start { .. }
        );
        let can = session
            .device(&peer.who)
            .is_some_and(|d| d.audible && d.here);
        if !wants_sound || !can {
            // Told rather than ignored: the bar that asked is the bar that has
            // to show the press went nowhere.
            let session = session.clone();
            post.tell_room(Hear::State { session });
            return;
        }

        session.output = Some(peer.who.clone());
        session.moving = None;
        match command {
            // It brought its own queue; nothing here knows better.
            Command::Start { .. } => post.tell(&peer.who, Hear::Do { command }),
            other => {
                // Hand it the session first, because it may never have had
                // one — a phone that has been watching a speaker all evening
                // holds no queue of its own. Then the press it actually made,
                // so `Next` still means next.
                post.tell(
                    &peer.who,
                    Hear::Do {
                        command: Command::Start {
                            queue: session.queue.clone(),
                            at: session.at,
                            position_ms: session.position_ms,
                            playing: true,
                        },
                    },
                );
                if !matches!(other, Command::Play) {
                    post.tell(&peer.who, Hear::Do { command: other });
                }
            }
        }
        let session = session.clone();
        post.tell_room(Hear::State { session });
        post.keep();
    }

    /// Move the sound. `None` stops it everywhere.
    ///
    /// The new output is handed the session as one [`Command::Start`] — the
    /// queue, the place in it, the point in the track and whether it was
    /// playing — because a hand-off is that sentence and nothing else. That
    /// is also what makes picking a speaker halfway through a track resume
    /// rather than restart.
    ///
    /// The old one is told nothing: it learns from the broadcast that it is no
    /// longer the output, and a device that is not the output is silent. One
    /// rule, in one place, rather than a stop command that a dropped socket
    /// could lose.
    fn transfer(&mut self, peer: &Peer, to: Option<DeviceId>, post: &mut Post<'_, Hear>) {
        let Some(session) = self.rooms.get_mut(&peer.room) else {
            return;
        };
        match to {
            None => {
                session.output = None;
                session.moving = None;
                session.playing = false;
            }
            Some(id) => {
                // Only a device that can be heard, and only one that is here:
                // handing the sound to something that is not answering is the
                // one way to lose it entirely.
                let ready = session
                    .device(&id)
                    .is_some_and(|d| d.audible && d.here && post.here(&id));
                if !ready {
                    let session = session.clone();
                    post.tell_room(Hear::State { session });
                    return;
                }
                if session.outputs(&id) && session.moving.is_none() {
                    return;
                }
                session.output = Some(id.clone());
                // Until it says otherwise it is *connecting*, not playing. A
                // speaker in the house takes a second or two to fetch
                // anything, and a picker that goes straight to "playing"
                // spends that second lying.
                session.moving = Some(id.clone());
                let take = Command::Start {
                    queue: session.queue.clone(),
                    at: session.at,
                    position_ms: session.position_ms,
                    playing: session.playing,
                };
                post.tell(&id, Hear::Do { command: take });
            }
        }
        let session = session.clone();
        post.tell_room(Hear::State { session });
        post.keep();
    }
}
