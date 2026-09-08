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
            sim.mutate(i, harken::add_song(&format!("c{i}-{round}"), "someone"));
            sim.step();
        }
    }

    sim.partition(2);
    for round in 0..4 {
        sim.mutate(2, harken::add_song(&format!("dark-{round}"), "someone"));
        sim.mutate(0, harken::add_song(&format!("lit-{round}"), "someone"));
        sim.step();
    }
    // And the interesting one: both sides favourite while apart, so the
    // playlist positions have to be recomputed rather than merged.
    sim.mutate(0, harken::favorite_all());
    sim.mutate(2, harken::favorite_all());
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
        harken::library(sim.conn(0)).unwrap().len(),
        17,
        "9 shared + 4 dark + 4 lit, none lost and none duplicated"
    );
    let playlist = harken::favorites(sim.conn(0)).unwrap();
    assert_eq!(playlist.len(), 17, "both FavoriteAll intents, merged by replay");
    let places: Vec<i64> = playlist.iter().filter_map(|s| s.favorite_pos).collect();
    let mut sorted = places.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        places.len(),
        sorted.len(),
        "every song holds a distinct place in the playlist"
    );
}
