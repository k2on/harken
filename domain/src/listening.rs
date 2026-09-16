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
//! precisely the state that should be *lost* when the server restarts. So it
//! lives in the server's memory, over a socket of its own at `/listen`, and
//! `functions.rs` is still the only `apply`.
//!
//! **It is here, in the domain crate, because that is the only vocabulary the
//! server and the clients already share.** It is not the domain: nothing below
//! writes a row, and the module the phone loads never sees it — `storage` is
//! off there. What it needs from the domain is [`Id`], which is what makes
//! "the track that is playing" the same value on both sides of the wire.
//!
//! **JSON, not CBOR.** The engine's frames are CBOR because they are the log;
//! these are not, and a third client reads them — the phone, in TypeScript,
//! over a `WebSocket` the platform already has. A protocol two languages speak
//! should be one every language speaks, and one `websocat` can print.

use serde::{Deserialize, Serialize};

use crate::schema::tables;
use crate::Id;

/// One device, as everything here names it.
///
/// It is the *login's* session id. `petros-auth` says a session is "one login
/// on one device", which is exactly this and already exists — so a phone that
/// reconnects is the same device it was, and signing out and back in is
/// honestly a new one.
pub type DeviceId = String;

/// A track, carried rather than looked up.
///
/// Everything needed to play it, so that a device handed the session does not
/// have to find it in its own replica first — which it might not have yet, and
/// which would make a hand-off fail in a way nobody could see. `file` travels
/// for the same reason: the receiving device joins it to *its* server, the way
/// `media_url` does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    pub id: Id<tables::Media>,
    pub title: String,
    pub creator: String,
    pub album: String,
    pub duration_ms: i64,
    pub file: String,
}

/// Somewhere this account is signed in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    pub id: DeviceId,
    /// What the picker draws. The client says it; nothing here interprets it.
    pub name: String,
    /// Whether it can make a sound at all. The desktop build cannot — it has
    /// no audio device — so it is a remote control and never an output, and
    /// saying which is a field rather than something the server guesses.
    pub audible: bool,
}

/// The whole session. Small enough to send entire on every change, which is
/// why there are no deltas here: a queue is a few hundred bytes and a client
/// that receives the whole truth cannot drift from it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Session {
    /// Which device is making the sound. `None` when nothing is.
    pub output: Option<DeviceId>,
    pub devices: Vec<Device>,
    /// What is playing and what comes after it. The snapshot taken when play
    /// was pressed, which is the desktop's rule for its own queue and the same
    /// reason: a live reference gets silently redirected by somebody else's
    /// edit arriving.
    pub queue: Vec<Track>,
    pub at: usize,
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
        self.queue.get(self.at)
    }

    /// Whether `device` is the one making the sound.
    pub fn outputs(&self, device: &str) -> bool {
        self.output.as_deref() == Some(device)
    }

    /// The device making the sound, as the picker names it.
    pub fn output_device(&self) -> Option<&Device> {
        let id = self.output.as_deref()?;
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
#[serde(tag = "do", rename_all = "snake_case")]
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
        at: usize,
        position_ms: i64,
        playing: bool,
    },
}

/// What a device says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "say", rename_all = "snake_case")]
pub enum Say {
    /// The first frame, and the only one carrying a token: who this is, which
    /// device, and whether it can be heard.
    Hello {
        token: String,
        device: DeviceId,
        name: String,
        audible: bool,
    },
    /// I am the output, and this is what I am doing. Sent when something
    /// changes and about once a second while playing.
    Report {
        queue: Vec<Track>,
        at: usize,
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
#[serde(tag = "hear", rename_all = "snake_case")]
pub enum Hear {
    /// That token proves nothing. The socket closes behind this.
    Denied { reason: String },
    /// The session, entire, to every device of this account.
    State { session: Session },
    /// You are the output. Do this.
    Do { command: Command },
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

/// A frame, as it travels: JSON, in a text message.
///
/// Here rather than at each end because the encoding is part of the protocol,
/// and a server that wrote `serde_json::to_string` and a client that wrote
/// something else would be two answers to one question. It also keeps
/// `serde_json` out of the programs: both already depend on this crate.
///
/// A frame that will not serialise is not a runtime possibility for any type
/// below — no maps with non-string keys, no floats that can be `NaN` — so the
/// empty string is unreachable rather than a swallowed error.
pub fn encode<T: Serialize>(frame: &T) -> String {
    serde_json::to_string(frame).unwrap_or_default()
}

/// The other direction. `None` is "not a frame I know", which is what an older
/// peer should do with a sentence a newer one invented: ignore it and carry on,
/// rather than drop the socket.
pub fn decode<T: serde::de::DeserializeOwned>(text: &str) -> Option<T> {
    serde_json::from_str(text).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(title: &str) -> Track {
        Track {
            id: Id::default(),
            title: title.into(),
            creator: "Bach".into(),
            album: "".into(),
            duration_ms: 1000,
            file: "music/a.mp3".into(),
        }
    }

    /// Every frame survives the wire, and the tags are the ones a TypeScript
    /// client is written against — which is the half of this that a Rust test
    /// can still check.
    #[test]
    fn the_frames_are_the_json_the_phone_reads() {
        let hello = Say::Hello {
            token: "t".into(),
            device: "d".into(),
            name: "Phone".into(),
            audible: true,
        };
        let text = encode(&hello);
        assert!(text.contains(r#""say":"hello""#), "{text}");
        assert_eq!(decode::<Say>(&text), Some(hello));

        let seek = Say::Do {
            command: Command::Seek { position_ms: 4200 },
        };
        let text = encode(&seek);
        assert!(text.contains(r#""do":"seek""#), "{text}");
        assert_eq!(decode::<Say>(&text), Some(seek));

        let state = Hear::State {
            session: Session {
                output: Some("d".into()),
                devices: vec![Device {
                    id: "d".into(),
                    name: "Phone".into(),
                    audible: true,
                }],
                queue: vec![track("Air")],
                at: 0,
                playing: true,
                position_ms: 12,
            },
        };
        let text = encode(&state);
        assert_eq!(decode::<Hear>(&text), Some(state));
        assert_eq!(decode::<Hear>("{}"), None, "not a frame this peer knows");
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

    /// The one derived fact, and the one that decides whether a client makes a
    /// sound.
    #[test]
    fn the_session_says_what_is_playing_and_who_is_playing_it() {
        let mut session = Session {
            queue: vec![track("Air"), track("Gigue")],
            at: 1,
            ..Session::default()
        };
        assert_eq!(session.now().map(|t| t.title.as_str()), Some("Gigue"));
        assert!(!session.outputs("d"), "nobody is the output yet");

        session.output = Some("d".into());
        assert!(session.outputs("d"));
        assert!(!session.outputs("other"));
        // A device that is the output and not in the list is not a device the
        // picker can draw, so `output_device` answers about the list.
        assert!(session.output_device().is_none());

        session.at = 9;
        assert!(session.now().is_none(), "past the end is nothing playing");
    }
}
