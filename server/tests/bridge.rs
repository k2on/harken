//! A speaker reaches the picker the way it does in production: through a real
//! [`petros_axum::Hub`], a room the desk announced, and a peer the server
//! stands in for.
//!
//! `tests/listening.rs` holds the desk's rules against a bare `Server`; this
//! is the plumbing above it, which those tests cannot see and which is where
//! a device goes missing without any rule being wrong — a `Watch` nobody
//! receives, a `Hello` that never entered a room, a frame `deliver` addressed
//! to a peer it does not hold.

use harken::listening::{Hear, Kind, Say, Session};
use harken::HarkenApp;
use harken_server::listening::{Desk, Watch};
use petros::{ActorId, Authenticate, ClientMsg, Identity, ServerMsg};
use petros_axum::Hub;

/// `user:session`, and nothing else is anybody.
struct Tokens;

impl Authenticate for Tokens {
    fn authenticate(&mut self, token: Option<&str>) -> Option<Identity> {
        let (user, session) = token?.split_once(':')?;
        Some(Identity {
            user: ActorId::from(user),
            session: session.to_string(),
        })
    }
}

/// The last thing this peer was told the session is.
fn state(told: Vec<ServerMsg<<HarkenApp as petros::App>::Mutation>>) -> Option<Session> {
    told.into_iter()
        .filter_map(|m| match m {
            ServerMsg::Heard { hear } => petros::decode::<Hear>(&hear).ok(),
            _ => None,
        })
        .filter_map(|hear| match hear {
            Hear::State { session } => Some(session),
            Hear::Do { .. } => None,
        })
        .next_back()
}

fn here(name: &str, kind: Kind) -> Vec<u8> {
    petros::encode(&Say::Here {
        name: name.into(),
        audible: true,
        kind,
    })
    .unwrap()
}

#[test]
fn a_phone_sees_itself_and_the_speaker_stood_beside_it() {
    let mut desk = Desk::new();
    let (tx, mut rooms) = tokio::sync::mpsc::unbounded_channel::<Watch>();
    desk.watch(tx);
    let hub = Hub::<HarkenApp>::open_live(petros::open_memory().unwrap(), Tokens, desk).unwrap();

    // A phone: `Hello`, then `Here`, on one connection.
    let phone = hub.local();
    let told = hub.exchange(
        phone,
        ClientMsg::Hello {
            since: 0,
            token: Some("max:phone-login".into()),
        },
    );
    let joined = state(told).expect("a joining peer is told the session at once");
    assert!(
        joined.devices.is_empty(),
        "arriving puts nobody in the picker"
    );
    let told = hub.exchange(
        phone,
        ClientMsg::Say {
            say: here("Pixel", Kind::Phone),
        },
    );
    let session = state(told).expect("saying Here is answered with the session");
    assert_eq!(session.devices.len(), 1);
    assert_eq!(session.devices[0].id, "phone-login");

    // Somebody is listening as `max`, so the bridge is told — and only now.
    assert_eq!(rooms.try_recv(), Ok(Watch::Open("max".into())));

    // It stands a speaker in that room and says what it is, exactly as
    // `assistant::ha::open` does.
    let (stx, mut speaker_hears) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    let conn = hub
        .stand(
            Identity {
                user: ActorId::from("max"),
                session: "media_player.bedroom".into(),
            },
            stx,
        )
        .unwrap();
    hub.say(conn, here("Bedroom", Kind::Speaker));

    // The speaker was told the room, and then the room with itself in it.
    let mut heard = Vec::new();
    while let Ok(frame) = speaker_hears.try_recv() {
        heard.push(petros::decode::<Hear>(&frame).unwrap());
    }
    assert!(
        matches!(heard.last(), Some(Hear::State { session }) if session.devices.len() == 2),
        "the speaker hears the room it is in: {heard:?}"
    );

    // …and so was the phone. It has no socket here to have received the
    // broadcast on, so ask through the same door: a `Here` from a device
    // already here changes nothing and is answered with the session.
    let told = hub.exchange(
        phone,
        ClientMsg::Say {
            say: here("Pixel", Kind::Phone),
        },
    );
    let session = state(told).unwrap();
    let mut ids: Vec<&str> = session.devices.iter().map(|d| d.id.as_str()).collect();
    ids.sort_unstable();
    assert_eq!(ids, ["media_player.bedroom", "phone-login"]);
    // A speaker standing in the room does not count as somebody listening.
    assert!(rooms.try_recv().is_err(), "no second announcement");
}

/// The bug the debug screen found. A library longer than one batch has the
/// client send a *second* `Hello` on the same socket to ask for the rest —
/// and a `Hello` used to be a departure and an arrival, so the desk dropped
/// the browser from its own picker, saw nobody listening, sent the speakers
/// away, and answered every `State` after that with no devices at all. The
/// browser's connection count had not moved, so it never said `Here` again.
///
/// Asserted through the hub because that is where the two channels meet: the
/// second `Hello` is the log's, and what it must not do is to the room's.
#[test]
fn asking_for_the_next_batch_does_not_leave_the_room() {
    let mut desk = Desk::new();
    let (tx, mut rooms) = tokio::sync::mpsc::unbounded_channel::<Watch>();
    desk.watch(tx);
    let hub = Hub::<HarkenApp>::open_live(petros::open_memory().unwrap(), Tokens, desk).unwrap();
    let browser = hub.local();
    let hello = || ClientMsg::Hello {
        since: 0,
        token: Some("max:browser-login".into()),
    };
    hub.exchange(browser, hello());
    hub.exchange(
        browser,
        ClientMsg::Say {
            say: here("Browser", Kind::Computer),
        },
    );
    assert_eq!(rooms.try_recv(), Ok(Watch::Open("max".into())));
    let (stx, _speaker_hears) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    let conn = hub
        .stand(
            Identity {
                user: ActorId::from("max"),
                session: "media_player.bedroom".into(),
            },
            stx,
        )
        .unwrap();
    hub.say(conn, here("Bedroom", Kind::Speaker));

    // The next batch, please.
    let told = hub.exchange(browser, hello());
    if let Some(session) = state(told) {
        assert_eq!(
            session.devices.len(),
            2,
            "a resumed connection is still in the room: {session:?}"
        );
    }
    assert!(
        rooms.try_recv().is_err(),
        "nobody left, so the bridge is told nothing"
    );
    let told = hub.exchange(
        browser,
        ClientMsg::Say {
            say: here("Browser", Kind::Computer),
        },
    );
    let session = state(told).unwrap();
    let mut ids: Vec<&str> = session.devices.iter().map(|d| d.id.as_str()).collect();
    ids.sort_unstable();
    assert_eq!(ids, ["browser-login", "media_player.bedroom"]);
}
