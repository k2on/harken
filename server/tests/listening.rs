//! The listening session's rules, against a real `petros::Server` with no
//! socket under it.
//!
//! Driven through [`petros::Server::stand`] rather than a `Hello`, because
//! that is what a device with no replica does and it is exactly what the Home
//! Assistant bridge does for a speaker — so the harness is the feature rather
//! than a stub of it.
//!
//! What every test here is about is one sentence of a bug report: *"if I'm
//! playing something on a device, and then the connection goes, it switches to
//! the local device, when it should remain on the last device played."*

use std::collections::BTreeMap;

use harken::listening::{Command, Hear, Kind, Say, Session, Track};
use harken::HarkenApp;
use harken_server::listening::Desk;
use petros::{ActorId, ClientMsg, ConnId, Identity, Server, ServerMsg, Trusting};

/// One account's devices, and the server they are all talking to.
struct House {
    server: Server<HarkenApp>,
    conns: BTreeMap<String, ConnId>,
    user: String,
    next: ConnId,
}

impl House {
    fn new(user: &str) -> House {
        House {
            server: Server::open_with(petros::open_memory().unwrap(), Trusting)
                .unwrap()
                .with_live(Desk::new()),
            conns: BTreeMap::new(),
            user: user.to_string(),
            next: 1,
        }
    }

    fn conn(&mut self) -> ConnId {
        self.next += 1;
        self.next
    }

    /// A device arrives and says what it is — the two halves a client does in
    /// one breath. Answers with what each device was told.
    fn arrive(&mut self, device: &str, audible: bool, kind: Kind) -> Told {
        let conn = self.conn();
        self.server
            .stand(
                conn,
                Identity {
                    user: ActorId::from(self.user.as_str()),
                    session: device.to_string(),
                },
            )
            .unwrap();
        self.conns.insert(device.to_string(), conn);
        self.drain();
        self.say(
            device,
            Say::Here {
                name: device.to_string(),
                audible,
                kind,
            },
        )
    }

    /// Its socket went. Not a decision about the music — which is the whole
    /// point of the thing under test.
    fn leave(&mut self, device: &str) -> Told {
        if let Some(conn) = self.conns.remove(device) {
            self.server.unstand(conn);
        }
        self.drain()
    }

    fn say(&mut self, device: &str, what: Say) -> Told {
        let conn = self.conns[device];
        let say = petros::encode(&what).unwrap();
        self.server.recv(conn, ClientMsg::Say { say }).unwrap();
        self.drain()
    }

    fn drain(&mut self) -> Told {
        let by_conn: BTreeMap<ConnId, String> =
            self.conns.iter().map(|(d, c)| (*c, d.clone())).collect();
        let mut told = Told::default();
        for (conn, msg) in self.server.take_outgoing() {
            let ServerMsg::Heard { hear } = msg else {
                continue;
            };
            let Ok(hear) = petros::decode::<Hear>(&hear) else {
                continue;
            };
            let who = by_conn.get(&conn).cloned().unwrap_or_default();
            match hear {
                Hear::State { session } => told.states.push((who, session)),
                Hear::Do { command } => told.dos.push((who, command)),
            }
        }
        told
    }

    /// The session as every device last saw it.
    fn session(&mut self) -> Session {
        // A `Here` from a device that is already here changes nothing about
        // the output, so it is a safe way to ask — and one that goes through
        // the same broadcast everything else does.
        let device = self.conns.keys().next().expect("somebody is here").clone();
        let told = self.say(
            &device,
            Say::Here {
                name: device.clone(),
                audible: true,
                kind: Kind::Computer,
            },
        );
        told.states.last().expect("a broadcast").1.clone()
    }
}

#[derive(Debug, Default)]
struct Told {
    states: Vec<(String, Session)>,
    dos: Vec<(String, Command)>,
}

impl Told {
    /// What one device was told to do.
    fn dos(&self, device: &str) -> Vec<Command> {
        self.dos
            .iter()
            .filter(|(who, _)| who == device)
            .map(|(_, c)| c.clone())
            .collect()
    }

    fn state(&self, device: &str) -> Option<&Session> {
        self.states
            .iter()
            .rev()
            .find(|(who, _)| who == device)
            .map(|(_, s)| s)
    }
}

fn track(title: &str) -> Track {
    Track {
        id: title.into(),
        title: title.into(),
        creator: "Bach".into(),
        album: String::new(),
        duration_ms: 300_000,
        file: format!("music/{title}.mp3"),
    }
}

fn playing_on(house: &mut House, device: &str, position_ms: i64) {
    house.say(
        device,
        Say::Do {
            command: Command::Start {
                queue: vec![track("a"), track("b")],
                at: 0,
                position_ms: 0,
                playing: true,
            },
        },
    );
    house.say(
        device,
        Say::Report {
            queue: vec![track("a"), track("b")],
            at: 0,
            playing: true,
            position_ms,
        },
    );
}

/// The bug, as one test. A laptop is playing; its socket goes; the sound is
/// still the laptop's. Before this the session cleared its output when the
/// socket dropped, so the next device to ask for anything got the music — and
/// what that looks like is a phone in your pocket starting to play because a
/// lid closed.
#[test]
fn the_sound_stays_with_a_device_whose_socket_went() {
    let mut house = House::new("alice");
    house.arrive("laptop", true, Kind::Computer);
    house.arrive("phone", true, Kind::Phone);
    playing_on(&mut house, "laptop", 42_000);
    assert_eq!(house.session().output.as_deref(), Some("laptop"));

    let told = house.leave("laptop");
    let session = told.state("phone").expect("the phone is told");
    assert_eq!(
        session.output.as_deref(),
        Some("laptop"),
        "the sound belongs to the laptop; it is simply not answering"
    );
    assert!(
        !session.playing,
        "and it cannot be playing, there is nobody there"
    );
    assert!(
        !session.device("laptop").expect("still in the picker").here,
        "drawn as away rather than dropped: \"your laptop, which is not answering\" \
         is worth drawing and nothing is not"
    );
    assert_eq!(session.position_ms, 42_000, "and where it had got to");
}

/// A device that merely turns up takes nothing. This is the other half of the
/// same bug: with the laptop away, a phone opening its app must not become the
/// output just by existing.
#[test]
fn arriving_is_not_how_a_device_gets_the_sound() {
    let mut house = House::new("alice");
    house.arrive("laptop", true, Kind::Computer);
    playing_on(&mut house, "laptop", 10_000);
    house.leave("laptop");

    let told = house.arrive("phone", true, Kind::Phone);
    assert_eq!(
        told.state("phone").unwrap().output.as_deref(),
        Some("laptop")
    );
    assert!(told.dos("phone").is_empty(), "and it is told to do nothing");
}

/// A command goes to the output, not to whoever asked. That is the feature:
/// pressing pause on a phone pauses the laptop.
#[test]
fn a_command_goes_to_the_device_making_the_sound() {
    let mut house = House::new("alice");
    house.arrive("laptop", true, Kind::Computer);
    house.arrive("phone", true, Kind::Phone);
    playing_on(&mut house, "laptop", 0);

    let told = house.say(
        "phone",
        Say::Do {
            command: Command::Pause,
        },
    );
    assert_eq!(told.dos("laptop"), [Command::Pause]);
    assert!(told.dos("phone").is_empty());
    assert_eq!(
        house.session().output.as_deref(),
        Some("laptop"),
        "and asking did not move it"
    );
}

/// …unless the output cannot be reached and the asker can make a sound. Which
/// needs a press that *means* a sound: a pause aimed at a device that has gone
/// is a press with nothing to do, and answering it by seizing the music would
/// be exactly the assumption of control this whole change is against.
#[test]
fn only_a_press_that_means_a_sound_takes_it_from_a_device_that_has_gone() {
    let mut house = House::new("alice");
    house.arrive("laptop", true, Kind::Computer);
    house.arrive("phone", true, Kind::Phone);
    playing_on(&mut house, "laptop", 77_000);
    house.leave("laptop");

    for quiet in [Command::Pause, Command::Seek { position_ms: 5 }] {
        let told = house.say("phone", Say::Do { command: quiet });
        assert!(told.dos("phone").is_empty());
        assert_eq!(
            told.state("phone").unwrap().output.as_deref(),
            Some("laptop"),
            "nothing to pause is not a reason to take the music"
        );
    }

    let told = house.say(
        "phone",
        Say::Do {
            command: Command::Play,
        },
    );
    assert_eq!(
        told.state("phone").unwrap().output.as_deref(),
        Some("phone"),
        "somebody pressed play and there is nothing else that can answer"
    );
    assert_eq!(
        told.dos("phone"),
        [Command::Start {
            queue: vec![track("a"), track("b")],
            at: 0,
            position_ms: 77_000,
            playing: true,
        }],
        "handed the session, because a phone that has been watching a laptop \
         all evening holds no queue of its own — and from where it had got to"
    );
}

/// A device that cannot make a sound never takes it, however hard it presses.
/// The desktop build has no audio device at all, so it is a remote control.
#[test]
fn a_device_that_cannot_be_heard_never_takes_the_sound() {
    let mut house = House::new("alice");
    house.arrive("laptop", true, Kind::Computer);
    house.arrive("desktop", false, Kind::Computer);
    playing_on(&mut house, "laptop", 0);
    house.leave("laptop");

    let told = house.say(
        "desktop",
        Say::Do {
            command: Command::Play,
        },
    );
    assert!(told.dos("desktop").is_empty());
    assert_eq!(
        told.state("desktop").unwrap().output.as_deref(),
        Some("laptop")
    );
}

/// Picking a speaker halfway through a track picks up halfway through it. The
/// hand-off is one `Start` carrying the queue, the place in it, the point in
/// the track and whether it was playing — which is the same sentence somebody
/// pressing a track says, with different numbers in it.
///
/// And until the speaker says otherwise it is *connecting*, not playing: a
/// speaker in the house takes a second or two to fetch anything, and a picker
/// that goes straight to "playing" spends that second lying.
#[test]
fn picking_a_speaker_picks_up_where_the_track_was() {
    let mut house = House::new("alice");
    house.arrive("phone", true, Kind::Phone);
    house.arrive("media_player.kitchen", true, Kind::Speaker);
    playing_on(&mut house, "phone", 91_500);

    let told = house.say(
        "phone",
        Say::Transfer {
            to: Some("media_player.kitchen".into()),
        },
    );
    assert_eq!(
        told.dos("media_player.kitchen"),
        [Command::Start {
            queue: vec![track("a"), track("b")],
            at: 0,
            position_ms: 91_500,
            playing: true,
        }]
    );
    let session = told.state("phone").unwrap();
    assert_eq!(session.output.as_deref(), Some("media_player.kitchen"));
    assert_eq!(
        session.moving.as_deref(),
        Some("media_player.kitchen"),
        "still connecting, which is what a client draws a spinner from"
    );

    // …and the speaker's first report is what ends that, because taking a
    // hand-off is exactly "started playing" and there is no other evidence of
    // it there could be.
    let told = house.say(
        "media_player.kitchen",
        Say::Report {
            queue: vec![track("a"), track("b")],
            at: 0,
            playing: true,
            position_ms: 92_100,
        },
    );
    let session = told.state("phone").unwrap();
    assert_eq!(session.moving, None);
    assert_eq!(session.position_ms, 92_100);
}

/// The sound cannot be handed to something that is not answering, which is the
/// one way to lose it entirely.
#[test]
fn the_sound_is_not_handed_to_a_device_that_has_gone() {
    let mut house = House::new("alice");
    house.arrive("phone", true, Kind::Phone);
    house.arrive("media_player.kitchen", true, Kind::Speaker);
    playing_on(&mut house, "phone", 0);
    house.leave("media_player.kitchen");

    let told = house.say(
        "phone",
        Say::Transfer {
            to: Some("media_player.kitchen".into()),
        },
    );
    let session = told.state("phone").unwrap();
    assert_eq!(session.output.as_deref(), Some("phone"));
    assert_eq!(session.moving, None);
}

/// The last device out does *not* take the room with it any more. What is
/// kept is where the music was and what it was — never that it is playing, and
/// never that anybody is there.
#[test]
fn an_empty_room_is_written_down_and_comes_back() {
    let mut house = House::new("alice");
    house.arrive("laptop", true, Kind::Computer);
    playing_on(&mut house, "laptop", 61_000);
    house.leave("laptop");

    let told = house.arrive("phone", true, Kind::Phone);
    let session = told.state("phone").expect("the phone is told");
    assert_eq!(session.output.as_deref(), Some("laptop"));
    assert_eq!(session.queue, vec![track("a"), track("b")]);
    assert_eq!(session.position_ms, 61_000);
    assert!(
        !session.playing,
        "what was true is where it was, not that it is"
    );
    assert!(
        !session.device("laptop").expect("drawn as away").here,
        "nobody is here but the phone"
    );
}

/// Stopping it everywhere is a device of its own in the picker, and it means
/// what it says.
#[test]
fn stopping_it_everywhere_leaves_nothing_the_output() {
    let mut house = House::new("alice");
    house.arrive("laptop", true, Kind::Computer);
    playing_on(&mut house, "laptop", 5_000);

    let told = house.say("laptop", Say::Transfer { to: None });
    let session = told.state("laptop").unwrap();
    assert_eq!(session.output, None);
    assert!(!session.playing);
}

/// A room is an account, not a connection: two people's sessions never meet,
/// however many devices each of them has.
#[test]
fn two_accounts_are_two_sessions() {
    let mut alice = House::new("alice");
    alice.arrive("laptop", true, Kind::Computer);
    playing_on(&mut alice, "laptop", 0);

    // Bob, on the same server. The conns come from the same counter, which is
    // what would make a room keyed by anything but the account leak.
    let conn = alice.conn();
    alice
        .server
        .stand(
            conn,
            Identity {
                user: ActorId::from("bob"),
                session: "phone".into(),
            },
        )
        .unwrap();
    alice.conns.insert("bob-phone".into(), conn);
    let told = alice.drain();
    let session = told.state("bob-phone").expect("bob is told his own");
    assert_eq!(session.output, None);
    assert!(session.queue.is_empty());
    assert!(session.devices.is_empty());
}
