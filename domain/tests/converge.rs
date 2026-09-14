//! The domain, against a simulated fleet.
//!
//! `petros-testkit` is the engine's own simulator, made generic so an app can
//! point it at its own mutations. This is that: no sockets, no threads, no
//! sleeps, and a seed that reproduces the run exactly.

use harken::HarkenApp;
use petros_testkit::Sim;

#[test]
fn the_domain_converges_when_peers_go_dark_and_come_back() {
    let mut sim = Sim::<HarkenApp>::new(19, 3);
    for round in 0..3 {
        for i in 0..sim.clients() {
            sim.mutate(
                i,
                harken::add_song(
                    format!("c{i}-{round}"),
                    "someone".into(),
                    String::new(),
                    0,
                    String::new(),
                ),
            );
            sim.step();
        }
    }

    sim.partition(2);
    for round in 0..4 {
        sim.mutate(
            2,
            harken::add_song(
                format!("dark-{round}"),
                "someone".into(),
                String::new(),
                0,
                String::new(),
            ),
        );
        sim.mutate(
            0,
            harken::add_song(
                format!("lit-{round}"),
                "someone".into(),
                String::new(),
                0,
                String::new(),
            ),
        );
        sim.step();
    }
    // A playlist to put things on. There is no favourites table any more — a
    // heart is membership of whichever playlist a client shows.
    sim.mutate(0, harken::create_playlist("Favourites".into()));
    sim.settle();

    // And the interesting one: both sides put the whole library on the same
    // playlist while apart, so the positions have to be recomputed by replay
    // rather than merged.
    let store = &mut petros::backend::SqliteStore::new(sim.conn(0));
    let favs = harken::playlists(store).unwrap()[0].id;
    for peer in [0usize, 2] {
        let ids: Vec<harken::Id<harken::tables::Media>> =
            harken::library(&mut petros::backend::SqliteStore::new(sim.conn(peer)), favs)
                .unwrap()
                .iter()
                .map(|i| i.id)
                .collect();
        for id in ids {
            sim.mutate(peer, harken::add_to_playlist(favs, id));
        }
    }
    sim.step();

    // `settle` reconnects every client itself. The work authored while dark is
    // recovered only because a reconnecting client re-offers what it still has
    // pending, so leaving it to `settle` is what makes this depend on that.
    sim.settle();

    let first = sim.state_hash(0);
    for i in 1..sim.clients() {
        assert_eq!(first, sim.state_hash(i), "client {i} disagrees");
    }
    assert_eq!(first, sim.server_hash(), "the server disagrees");
    assert_eq!(
        harken::library(&mut petros::backend::SqliteStore::new(sim.conn(0)), favs)
            .unwrap()
            .len(),
        17,
        "9 shared + 4 dark + 4 lit, none lost and none duplicated"
    );
    let playlist =
        harken::playlist(&mut petros::backend::SqliteStore::new(sim.conn(0)), favs).unwrap();
    let places: Vec<i64> = playlist.iter().filter_map(|s| s.playlist_pos).collect();
    let mut sorted = places.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        places.len(),
        "every position on the playlist is distinct after the replay"
    );
}
