//! One account, one thing playing, however many devices are watching it.
//!
//! A person with a phone, a laptop and a browser tab has **one** listening
//! session, not three. Any of them can say what should happen; exactly one of
//! them is making the sound; and moving the sound from one to another is a
//! sentence in this protocol rather than a track started again somewhere else.
//!
//! **None of this is in the log, and none of it ever will be.** The log is
//! permanent and totally ordered: every peer replays every entry, forever. An
//! afternoon of listening is thousands of pauses, seeks and skips, and not one
//! of them is worth replaying tomorrow — "what is playing right now" is
//! precisely the state that should be *lost*. So it lives in a `petros::live`
//! room, in the server's memory, and `functions.rs` is still the only `apply`.
//!
//! **It rides the sync socket, and used to have one of its own.** A second
//! socket at `/listen` is a second thing to authenticate, a second thing to
//! reconnect, a second thing to keep alive through a proxy, and a second
//! answer to "am I online" — and the two disagree at the worst moment, which
//! on a phone waking up is every morning. The engine carries this now, the
//! same way it carries a mutation: see `petros::live`.
//!
//! **It is here, in the domain crate, because that is the only vocabulary the
//! server and the clients already share.** It is not the domain: nothing below
//! writes a row, and the module the phone loads never sees it — `storage` is
//! off there.
//!
//! **CBOR, because the engine's frames are CBOR.** It used to be JSON so the
//! phone could read it, back when the phone parsed the wire itself. It does
//! not any more: the frames go through the same UniFFI peer the mutations do,
//! so every end of this protocol is Rust and the types below are the one
//! description of it.

use serde::{Deserialize, Serialize};

/// One device, as everything here names it.
///
/// Whatever names one output stably. For a client that is the *login's*
/// session id — `petros-auth` says a session is "one login on one device",
/// which is exactly this and already exists, so a phone that reconnects is
/// the same device it was and signing out and back in is honestly a new one.
/// It is what `petros::live` calls a peer's `who`, so no device ever says its
/// own id: the server already knows.
///
/// It is not always a login, and that is the point of saying it this way: a
/// speaker in the kitchen has no login and is a device all the same. There it
/// is the entity that names it — `media_player.kitchen` — because that is the
/// thing that is still the same speaker tomorrow.
pub type DeviceId = String;

/// A track, carried rather than looked up.
///
/// Everything needed to play it, so that a device handed the session does not
/// have to find it in its own replica first — which it might not have yet, and
/// which would make a hand-off fail in a way nobody could see. `file` travels
/// for the same reason: the receiving device joins it to *its* server, the way
/// [`url`] does.
///
/// **`id` is a string and not an `Id<tables::Media>`**, which is the one place
/// in this program that rule is not followed and is deliberate. The rule is
/// about a *mutation's* arguments, where a wrong id writes a wrong row for
/// ever. Nothing here writes anything: this is a copy of a row, sent so that a
/// device which may not hold the original can play it, and the id travels only
/// so a client can find the row if it happens to have one. It is also the one
/// field that has to cross three languages, and a tagged id is a Rust type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "foreign", derive(uniffi::Record))]
pub struct Track {
    pub id: String,
    pub title: String,
    pub creator: String,
    pub album: String,
    pub duration_ms: i64,
    pub file: String,
}

/// What sort of thing a device is, for the one glyph a picker draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "foreign", derive(uniffi::Enum))]
pub enum Kind {
    #[default]
    Computer,
    Phone,
    /// Something in the house, reached through Home Assistant.
    Speaker,
}

/// Somewhere this account listens.
///
/// Every device the session knows about, which is **not** the same as every
/// device that is connected: the one the music belongs to stays in the list
/// after its socket goes, because "your kitchen speaker, which is not
/// answering" is a different and more useful thing to draw than nothing at
/// all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "foreign", derive(uniffi::Record))]
pub struct Device {
    pub id: DeviceId,
    /// What the picker draws. The client says it; nothing here interprets it.
    pub name: String,
    /// Whether it can make a sound at all. The desktop build cannot — it has
    /// no audio device — so it is a remote control and never an output, and
    /// saying which is a field rather than something the server guesses.
    pub audible: bool,
    /// Whether its socket is open right now.
    pub here: bool,
    pub kind: Kind,
}

/// The whole session. Small enough to send entire on every change, which is
/// why there are no deltas here: a queue is a few hundred bytes and a client
/// that receives the whole truth cannot drift from it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "foreign", derive(uniffi::Record))]
pub struct Session {
    /// Which device the sound belongs to. `None` only when nothing has played
    /// yet, or when somebody stopped it everywhere.
    ///
    /// **It survives that device's socket going**, which is the whole
    /// difference between this and what came before. A laptop lid closing is
    /// not a decision to move the music, and treating it as one is how the
    /// sound used to jump back to whichever device asked next.
    pub output: Option<DeviceId>,
    /// A hand-off in flight: the sound has been given to this device and it
    /// has not reported since. A client draws it as a device still connecting
    /// rather than as one that is playing, because a speaker in the house
    /// takes a second or two to fetch anything.
    pub moving: Option<DeviceId>,
    pub devices: Vec<Device>,
    /// What is playing and what comes after it. The snapshot taken when play
    /// was pressed, which is the desktop's rule for its own queue and the same
    /// reason: a live reference gets silently redirected by somebody else's
    /// edit arriving.
    pub queue: Vec<Track>,
    pub at: u32,
    pub playing: bool,
    /// How far into [`Session::now`] the output last said it was.
    ///
    /// There is deliberately no timestamp beside it. The server has a clock
    /// and the clients have clocks and they do not agree, so a client that
    /// wants a moving scrubber counts from when *it* received this — and the
    /// output resends about once a second, which is what keeps the counting
    /// honest.
    pub position_ms: i64,
}

impl Session {
    /// What is playing, if anything.
    pub fn now(&self) -> Option<&Track> {
        self.queue.get(self.at as usize)
    }

    /// Whether `device` is the one the sound belongs to.
    pub fn outputs(&self, device: &str) -> bool {
        self.output.as_deref() == Some(device)
    }

    /// The device the sound belongs to, as the picker names it.
    pub fn output_device(&self) -> Option<&Device> {
        let id = self.output.as_deref()?;
        self.devices.iter().find(|d| d.id == id)
    }

    /// Whether the device the sound belongs to is actually there.
    ///
    /// The question every transport button asks: a command to a device that
    /// has gone is a button that does nothing, so it is the difference
    /// between sending one and obeying it here.
    pub fn output_here(&self) -> bool {
        self.output_device().is_some_and(|d| d.here)
    }

    /// Whether `device` is waiting to take the sound.
    pub fn moving_to(&self, device: &str) -> bool {
        self.moving.as_deref() == Some(device)
    }

    pub fn device(&self, id: &str) -> Option<&Device> {
        self.devices.iter().find(|d| d.id == id)
    }
}

/// Something to be done, wherever the sound is coming from.
///
/// Play and pause are two verbs rather than one toggle for the reason the
/// media session's are: the device asking is not the device doing, so "the
/// other one of whatever you are" is not a thing it can mean. A phone showing
/// a paused bar asks to play, and if the laptop resumed a second earlier that
/// request is simply already true.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    Play,
    Pause,
    Next,
    Previous,
    Seek {
        position_ms: i64,
    },
    /// Take this session: this queue, from here, at this point, playing or
    /// not.
    ///
    /// One command for two things that would otherwise be two — somebody
    /// pressing a track, and the session moving to another device — because
    /// they are the same sentence with different numbers in it. Two commands
    /// would be two code paths on every client and one of them would be the
    /// one nobody tested.
    Start {
        queue: Vec<Track>,
        at: u32,
        position_ms: i64,
        playing: bool,
    },
}

/// What a device says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Say {
    /// The first frame: what to call this device and what it can do.
    ///
    /// It carries no token and no id. The socket this rides on is the one the
    /// engine already authenticated, so the server knows both — which is the
    /// concrete saving from having one socket rather than two, and the reason
    /// a sign-in cannot now be true on one of them and stale on the other.
    Here {
        name: String,
        audible: bool,
        kind: Kind,
    },
    /// I am the output, and this is what I am doing. Sent when something
    /// changes and about once a second while playing.
    Report {
        queue: Vec<Track>,
        at: u32,
        playing: bool,
        position_ms: i64,
    },
    /// Do this — here, or wherever the sound actually is.
    Do { command: Command },
    /// Move the sound to `to`, or stop it everywhere with `None`.
    Transfer { to: Option<DeviceId> },
}

/// What the server says back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hear {
    /// The session, entire, to every device of this account.
    State { session: Session },
    /// You are the output. Do this.
    Do { command: Command },
}

/// The protocol as a boundary crosses it, which is not how Rust writes it.
///
/// Every type here is a *record* or a fieldless enum, and that is the whole
/// reason the module exists. `Command`, `Say` and `Hear` are enums with fields,
/// which is the right shape in Rust and the one shape a three-language
/// boundary handles worst: UniFFI renders each variant as its own class with
/// its own constructor, so a call site in TypeScript looks nothing like the
/// Rust it is calling and a reader cannot check one against the other. Flat
/// records and a plain `Verb` enum are what `PatchOp` already is on that side,
/// and they read the same in both languages.
///
/// It costs a conversion each way, written once, here, beside the definition
/// it is a conversion *of*.
///
/// So `Command`, `Say` and `Hear` deliberately carry no `uniffi` derive at
/// all. Leaving one on is not harmless: a derive registers the type in the
/// component interface whether or not any exported signature names it, so the
/// three would be generated, unused, in exactly the shape this module exists
/// to avoid — and the next person to read the TypeScript would find two ways
/// to say the same thing and no way to tell which is meant.
#[cfg(feature = "foreign")]
pub mod foreign {
    use super::{Command, DeviceId, Kind, Session, Track};

    /// Which command, without what it carries.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
    pub enum Verb {
        Play,
        Pause,
        Next,
        Previous,
        Seek,
        Start,
    }

    /// A command, flattened. The fields a verb does not use are ignored, which
    /// is what a record costs and what the conversions below make harmless.
    #[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
    pub struct Doing {
        pub verb: Verb,
        #[uniffi(default = [])]
        pub queue: Vec<Track>,
        #[uniffi(default = 0)]
        pub at: u32,
        #[uniffi(default = 0)]
        pub position_ms: i64,
        #[uniffi(default = true)]
        pub playing: bool,
    }

    /// What the session has said since the last ask.
    #[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
    pub struct Listened {
        /// The newest state, if one arrived. The older ones are dropped
        /// deliberately: a state is the whole truth about the session, so a
        /// previous one is only a previous truth and drawing it would be a
        /// frame of the wrong answer.
        pub session: Option<Session>,
        /// Every command, in order. These are not interchangeable the way the
        /// states are — two skips are two tracks.
        pub todo: Vec<Doing>,
    }

    impl From<Command> for Doing {
        fn from(command: Command) -> Doing {
            let flat = |verb| Doing {
                verb,
                queue: Vec::new(),
                at: 0,
                position_ms: 0,
                playing: true,
            };
            match command {
                Command::Play => flat(Verb::Play),
                Command::Pause => flat(Verb::Pause),
                Command::Next => flat(Verb::Next),
                Command::Previous => flat(Verb::Previous),
                Command::Seek { position_ms } => Doing {
                    position_ms,
                    ..flat(Verb::Seek)
                },
                Command::Start {
                    queue,
                    at,
                    position_ms,
                    playing,
                } => Doing {
                    verb: Verb::Start,
                    queue,
                    at,
                    position_ms,
                    playing,
                },
            }
        }
    }

    impl From<Doing> for Command {
        fn from(doing: Doing) -> Command {
            match doing.verb {
                Verb::Play => Command::Play,
                Verb::Pause => Command::Pause,
                Verb::Next => Command::Next,
                Verb::Previous => Command::Previous,
                Verb::Seek => Command::Seek {
                    position_ms: doing.position_ms,
                },
                Verb::Start => Command::Start {
                    queue: doing.queue,
                    at: doing.at,
                    position_ms: doing.position_ms,
                    playing: doing.playing,
                },
            }
        }
    }

    /// A phone is a phone and can always be heard, which is the whole reason
    /// it is the interesting device in this feature. Said here rather than at
    /// the call site so that nothing in `mobile/src` has to know it.
    pub const PHONE: Kind = Kind::Phone;

    /// Re-exported so a caller has one import for the boundary.
    pub type Device = super::Device;
    pub type Id = DeviceId;
}

/// Where a [`Track::file`] is, from where a server is.
///
/// The column has always been a *path*, and every device joins it to its own
/// server — which is what makes a hand-off work between a phone on the LAN and
/// a speaker that only knows an address on it. An absolute URL is already an
/// answer and passes through, because the demo's library is Wikimedia links.
///
/// Here rather than in a client because there are three of them now — the
/// browser, the phone, and a speaker that has no client at all — and the rule
/// is the domain's: it is about what `file` means.
pub fn url(base: &str, file: &str) -> String {
    if file.is_empty() || file.starts_with("http://") || file.starts_with("https://") {
        return file.to_string();
    }
    let mut out = format!("{}/media", base.trim_end_matches('/'));
    for part in file.split('/') {
        out.push('/');
        encode_segment(part, &mut out);
    }
    out
}

/// Percent-encode one path segment.
///
/// Real libraries are full of spaces, ampersands and the occasional `#`, and a
/// `#` is the one that is silently destructive: everything after it is a
/// fragment, so the request goes out for a path that stops mid-filename and
/// the server answers 404 — or worse, a single-page fallback answers 200 with
/// HTML. Everything outside RFC 3986's unreserved set is escaped, which covers
/// those and every non-ASCII byte.
fn encode_segment(part: &str, out: &mut String) {
    for b in part.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(title: &str) -> Track {
        Track {
            id: String::new(),
            title: title.into(),
            creator: "Bach".into(),
            album: String::new(),
            duration_ms: 1000,
            file: "music/a.mp3".into(),
        }
    }

    fn device(id: &str, here: bool) -> Device {
        Device {
            id: id.into(),
            name: id.into(),
            audible: true,
            here,
            kind: Kind::Computer,
        }
    }

    /// Every frame survives the wire the engine actually carries it on.
    #[test]
    fn the_frames_survive_cbor() {
        let here = Say::Here {
            name: "Phone".into(),
            audible: true,
            kind: Kind::Phone,
        };
        let bytes = petros::encode(&here).unwrap();
        assert_eq!(petros::decode::<Say>(&bytes).unwrap(), here);

        let told = Hear::Do {
            command: Command::Seek { position_ms: 4200 },
        };
        let bytes = petros::encode(&told).unwrap();
        assert_eq!(petros::decode::<Hear>(&bytes).unwrap(), told);

        let state = Hear::State {
            session: Session {
                output: Some("d".into()),
                devices: vec![device("d", true)],
                queue: vec![track("Air")],
                at: 0,
                playing: true,
                position_ms: 12,
                moving: None,
            },
        };
        let bytes = petros::encode(&state).unwrap();
        assert_eq!(petros::decode::<Hear>(&bytes).unwrap(), state);
    }

    /// A path becomes a URL against whichever server is asking, and a URL is
    /// already an answer.
    #[test]
    fn a_file_is_joined_to_the_server_that_is_asking() {
        assert_eq!(
            url("http://10.0.0.2:8787", "music/Bach/air.flac"),
            "http://10.0.0.2:8787/media/music/Bach/air.flac"
        );
        assert_eq!(
            url("https://harken.example.com/", "music/a.mp3"),
            "https://harken.example.com/media/music/a.mp3",
            "a trailing slash is not a second one"
        );
        // The characters a real library is full of, and the one that is
        // silently destructive.
        assert_eq!(
            url("http://h", "music/Boléro & co/no #1.mp3"),
            "http://h/media/music/Bol%C3%A9ro%20%26%20co/no%20%231.mp3"
        );
        // Already an answer, and not this server's to give.
        assert_eq!(
            url("http://h", "https://upload.wikimedia.org/x.mp3"),
            "https://upload.wikimedia.org/x.mp3"
        );
        assert_eq!(url("http://h", ""), "", "nothing to stream is not a URL");
    }

    /// The derived facts, and the one that decides whether a button is an
    /// instruction or a message.
    #[test]
    fn the_session_says_what_is_playing_and_whether_it_can_be_reached() {
        let mut session = Session {
            queue: vec![track("Air"), track("Gigue")],
            at: 1,
            ..Session::default()
        };
        assert_eq!(session.now().map(|t| t.title.as_str()), Some("Gigue"));
        assert!(!session.outputs("d"), "nobody has it yet");

        session.devices = vec![device("d", true)];
        session.output = Some("d".into());
        assert!(session.outputs("d"));
        assert!(session.output_here());

        // The case the whole redesign is about: the device the sound belongs
        // to has gone, and it still belongs to it.
        session.devices = vec![device("d", false)];
        assert!(session.outputs("d"), "still its sound");
        assert!(!session.output_here(), "and still nowhere to send a button");

        session.at = 9;
        assert!(session.now().is_none(), "past the end is nothing playing");
    }
}
