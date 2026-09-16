//! Home Assistant's media players, as devices in a listening session.
//!
//! A Sonos is a *device*, not a client. `harken::listening` already describes
//! one — it joins a room, becomes the output, is told things and reports what
//! it is doing — and nothing in that says the far end has to be somebody's
//! screen. So nothing here extends the protocol; what is here is the thing
//! that is a device on a speaker's behalf.
//!
//! **Home Assistant rather than Sonos directly**, and not for convenience: you
//! do not get Sonos, you get every `media_player` entity in the house. A
//! Chromecast, a television, an AirPlay receiver and a speaker group are all
//! the same six service calls, where talking UPnP would buy one make of
//! speaker and a discovery problem.
//!
//! [`Bridge`] owns no socket, like [`crate::listening::Desk`] and for the same
//! reason: what is worth asserting is the rules — which account holds a
//! speaker, what a command becomes, how a position report is read back — and
//! none of that needs a network to be wrong in.
//!
//! **The queue is harken's and the position is the speaker's.** The whole list
//! is enqueued, so the physical buttons and the Sonos app keep working; where
//! the speaker *is* in that list is then read back from what it says it is
//! playing. That is not two sources of truth, it is the rule the `Desk`
//! already has — the output reports, and whoever is making the sound is right
//! about it. Somebody skipping on the speaker itself moves every phone.

use std::collections::HashMap;

use harken::listening::{url, Command, Device, DeviceId, Track};

/// How Home Assistant describes a player, pared down to what decides
/// anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Playing {
    /// `playing`, `paused`, `idle`, `off`, `unavailable`. Anything that is not
    /// `playing` is not playing, which is the only distinction that matters.
    pub state: String,
    /// What it says it is playing, which is how it says where it is in the
    /// queue it was given. Empty when it is playing nothing.
    pub url: String,
    pub position_ms: i64,
}

/// What the bridge wants done. The socket performs these; this module never
/// sees one happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    /// Put this list on, from `at`, at this point. The socket turns it into
    /// one `play_media` and the rest as `enqueue: add`, which is what leaves
    /// the speaker's own next and previous working.
    Start {
        entity: DeviceId,
        urls: Vec<String>,
        at: usize,
        position_ms: i64,
        playing: bool,
    },
    /// `media_player.<verb>` on `entity`, with nothing else to say.
    Verb {
        entity: DeviceId,
        verb: &'static str,
    },
    Seek {
        entity: DeviceId,
        position_ms: i64,
    },
    /// …and the one that is not a service call: tell `user`'s room that the
    /// speaker they think they have is somebody else's now.
    ///
    /// A speaker is one piece of hardware and a room is one account's, so two
    /// people can both pick the kitchen. The second one gets it — which is
    /// what a real speaker does — and the first is told rather than left
    /// drawing a transport for a device playing somebody else's music.
    Release {
        user: String,
    },
}

/// What a speaker said, in the shape [`crate::listening::Desk::report`] takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Said {
    pub user: String,
    pub device: DeviceId,
    pub queue: Vec<Track>,
    pub at: usize,
    pub playing: bool,
    pub position_ms: i64,
}

/// What one speaker was handed, and by whom.
#[derive(Debug, Clone)]
struct Holding {
    user: String,
    queue: Vec<Track>,
    /// The same tracks as the speaker was given them, so what it reports back
    /// can be matched to an index without re-deriving anything.
    urls: Vec<String>,
}

/// Home Assistant, as a shelf of devices.
pub struct Bridge {
    players: Vec<Device>,
    /// Where a speaker fetches bytes from, which is not where a phone does:
    /// the phone may be on `https://harken.example.com` while the speaker
    /// only knows an address on the LAN.
    media: String,
    held: HashMap<DeviceId, Holding>,
}

impl Bridge {
    /// `players` are the entities to offer, already named. `media` is the base
    /// a speaker resolves a `file` against.
    pub fn new(players: Vec<Device>, media: &str) -> Bridge {
        Bridge {
            players,
            media: media.trim_end_matches('/').to_string(),
            held: HashMap::new(),
        }
    }

    /// The devices to stand in every room.
    pub fn devices(&self) -> &[Device] {
        &self.players
    }

    pub fn has(&self, entity: &str) -> bool {
        self.players.iter().any(|p| p.id == entity)
    }

    /// `user`'s room told `entity` to do something.
    pub fn told(&mut self, user: &str, entity: &str, command: Command) -> Vec<Act> {
        if !self.has(entity) {
            return Vec::new();
        }
        let mut acts = Vec::new();
        // Somebody else had it. They are told before it moves, so the message
        // that takes the speaker away and the sound that leaves are the same
        // event rather than two.
        if let Some(held) = self.held.get(entity) {
            if held.user != user {
                acts.push(Act::Release {
                    user: held.user.clone(),
                });
            }
        }
        let entity = entity.to_string();
        match command {
            Command::Start {
                queue,
                at,
                position_ms,
                playing,
            } => {
                let urls: Vec<String> = queue.iter().map(|t| url(&self.media, &t.file)).collect();
                self.held.insert(
                    entity.clone(),
                    Holding {
                        user: user.to_string(),
                        queue,
                        urls: urls.clone(),
                    },
                );
                acts.push(Act::Start {
                    entity,
                    urls,
                    at,
                    position_ms,
                    playing,
                });
            }
            other => {
                // Everything else is about a queue the speaker already has, so
                // a command for a speaker nobody handed anything to is a
                // command with nothing to do. Play is the exception worth
                // making: a speaker that was paused by hand is one `play` away
                // from carrying on.
                if !self.held.contains_key(&entity) && !matches!(other, Command::Play) {
                    return acts;
                }
                if let Some(held) = self.held.get_mut(&entity) {
                    held.user = user.to_string();
                }
                acts.push(match other {
                    Command::Play => Act::Verb {
                        entity,
                        verb: "media_play",
                    },
                    Command::Pause => Act::Verb {
                        entity,
                        verb: "media_pause",
                    },
                    Command::Next => Act::Verb {
                        entity,
                        verb: "media_next_track",
                    },
                    Command::Previous => Act::Verb {
                        entity,
                        verb: "media_previous_track",
                    },
                    Command::Seek { position_ms } => Act::Seek {
                        entity,
                        position_ms,
                    },
                    Command::Start { .. } => unreachable!("matched above"),
                });
            }
        }
        acts
    }

    /// Home Assistant said something about a player.
    ///
    /// `None` for a player nobody in a session handed anything to: a speaker
    /// somebody is using from the Sonos app is not this server's business, and
    /// reporting it would put a stranger's music in somebody's bar.
    pub fn heard(&mut self, entity: &str, now: &Playing) -> Option<Said> {
        let held = self.held.get(entity)?;
        // Where it says it is. A URL that is not one of ours means it has been
        // sent somewhere else entirely — a radio stream, a doorbell chime — so
        // the queue is no longer what it is playing and holding on to it would
        // be a lie with a scrubber on it.
        let at = held.urls.iter().position(|u| *u == now.url);
        let Some(at) = at else {
            self.held.remove(entity);
            return None;
        };
        Some(Said {
            user: held.user.clone(),
            device: entity.to_string(),
            queue: held.queue.clone(),
            at,
            playing: now.state == "playing",
            position_ms: now.position_ms.max(0),
        })
    }

    /// The speaker is gone — unavailable, or the room it was in has closed.
    pub fn forget(&mut self, entity: &str) {
        self.held.remove(entity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harken::Id;

    fn speaker(id: &str) -> Device {
        Device {
            id: id.into(),
            name: id.into(),
            audible: true,
        }
    }

    fn track(file: &str) -> Track {
        Track {
            id: Id::default(),
            title: file.into(),
            creator: "Bach".into(),
            album: String::new(),
            duration_ms: 60_000,
            file: file.into(),
        }
    }

    fn bridge() -> Bridge {
        Bridge::new(
            vec![
                speaker("media_player.kitchen"),
                speaker("media_player.study"),
            ],
            "http://10.0.0.2:8787/",
        )
    }

    fn start(queue: &[&str], at: usize) -> Command {
        Command::Start {
            queue: queue.iter().map(|f| track(f)).collect(),
            at,
            position_ms: 0,
            playing: true,
        }
    }

    /// A hand-off arrives as the whole list, resolved against the address the
    /// *speaker* can reach — which is not the one a phone uses.
    #[test]
    fn a_hand_off_is_the_whole_queue_as_urls_the_speaker_can_fetch() {
        let mut bridge = bridge();
        let acts = bridge.told(
            "alice",
            "media_player.kitchen",
            start(&["music/a.mp3", "music/b.mp3"], 1),
        );
        assert_eq!(
            acts,
            [Act::Start {
                entity: "media_player.kitchen".into(),
                urls: vec![
                    "http://10.0.0.2:8787/media/music/a.mp3".into(),
                    "http://10.0.0.2:8787/media/music/b.mp3".into(),
                ],
                at: 1,
                position_ms: 0,
                playing: true,
            }]
        );
    }

    /// An entity this server was not told to offer is not a device, so a
    /// command for one does nothing rather than reaching into the house.
    #[test]
    fn nothing_is_said_to_a_player_that_was_not_offered() {
        let mut bridge = bridge();
        assert!(bridge
            .told("alice", "media_player.bedroom", start(&["music/a.mp3"], 0))
            .is_empty());
        assert!(bridge
            .heard("media_player.bedroom", &playing("x", 0))
            .is_none());
    }

    fn playing(url: &str, position_ms: i64) -> Playing {
        Playing {
            state: "playing".into(),
            url: url.into(),
            position_ms,
        }
    }

    /// Where the speaker says it is, is where it is — including when nobody
    /// asked it to move. Somebody pressing the button on the speaker moves
    /// every phone.
    #[test]
    fn what_the_speaker_says_it_is_playing_is_where_it_is_in_the_queue() {
        let mut bridge = bridge();
        bridge.told(
            "alice",
            "media_player.kitchen",
            start(&["music/a.mp3", "music/b.mp3", "music/c.mp3"], 0),
        );

        let said = bridge
            .heard(
                "media_player.kitchen",
                &playing("http://10.0.0.2:8787/media/music/c.mp3", 4_200),
            )
            .expect("it is playing one of ours");
        assert_eq!(said.user, "alice");
        assert_eq!(said.at, 2, "the third, because that is what it named");
        assert_eq!(said.queue.len(), 3);
        assert!(said.playing);
        assert_eq!(said.position_ms, 4_200);

        // Paused is every state that is not `playing`, which is the only
        // distinction this needs to make.
        let said = bridge
            .heard(
                "media_player.kitchen",
                &Playing {
                    state: "paused".into(),
                    ..playing("http://10.0.0.2:8787/media/music/c.mp3", 4_200)
                },
            )
            .unwrap();
        assert!(!said.playing);
    }

    /// A speaker sent somewhere else entirely is no longer in this session,
    /// and saying otherwise would be a lie with a scrubber on it.
    #[test]
    fn a_speaker_playing_something_that_is_not_ours_is_let_go() {
        let mut bridge = bridge();
        bridge.told("alice", "media_player.kitchen", start(&["music/a.mp3"], 0));
        assert!(bridge
            .heard(
                "media_player.kitchen",
                &playing("http://radio.example/stream", 0)
            )
            .is_none());
        // …and it stays let go, rather than being recovered by the next
        // report that happens to match.
        assert!(bridge
            .heard(
                "media_player.kitchen",
                &playing("http://10.0.0.2:8787/media/music/a.mp3", 0)
            )
            .is_none());
    }

    /// Two people can both pick the kitchen, because a kitchen is one room.
    /// The second gets it and the first is told, rather than left drawing a
    /// transport for somebody else's music.
    #[test]
    fn a_speaker_taken_by_somebody_else_releases_the_first() {
        let mut bridge = bridge();
        bridge.told("alice", "media_player.kitchen", start(&["music/a.mp3"], 0));

        let acts = bridge.told("bob", "media_player.kitchen", start(&["music/b.mp3"], 0));
        assert_eq!(
            acts[0],
            Act::Release {
                user: "alice".into()
            }
        );
        assert!(matches!(acts[1], Act::Start { .. }));
        assert_eq!(acts.len(), 2);

        // …and it is bob's now, so his pause is not alice's release.
        let acts = bridge.told("bob", "media_player.kitchen", Command::Pause);
        assert_eq!(
            acts,
            [Act::Verb {
                entity: "media_player.kitchen".into(),
                verb: "media_pause",
            }]
        );
        assert_eq!(
            bridge
                .heard(
                    "media_player.kitchen",
                    &playing("http://10.0.0.2:8787/media/music/b.mp3", 0)
                )
                .unwrap()
                .user,
            "bob"
        );
    }

    /// The four verbs that are about a queue the speaker already has, and the
    /// one that is not.
    #[test]
    fn a_verb_needs_a_queue_except_the_one_that_does_not() {
        let mut bridge = bridge();
        // Nothing has been handed over, so there is nothing to pause or skip.
        for command in [Command::Pause, Command::Next, Command::Previous] {
            assert!(bridge
                .told("alice", "media_player.study", command)
                .is_empty());
        }
        // Play is the exception: a speaker somebody paused by hand is one
        // press away from carrying on.
        assert_eq!(
            bridge.told("alice", "media_player.study", Command::Play),
            [Act::Verb {
                entity: "media_player.study".into(),
                verb: "media_play",
            }]
        );

        bridge.told("alice", "media_player.study", start(&["music/a.mp3"], 0));
        assert_eq!(
            bridge.told(
                "alice",
                "media_player.study",
                Command::Seek { position_ms: 9_000 }
            ),
            [Act::Seek {
                entity: "media_player.study".into(),
                position_ms: 9_000,
            }]
        );
        assert_eq!(
            bridge.told("alice", "media_player.study", Command::Next),
            [Act::Verb {
                entity: "media_player.study".into(),
                verb: "media_next_track",
            }],
            "the speaker's own next, because it was given the whole list"
        );
    }
}
