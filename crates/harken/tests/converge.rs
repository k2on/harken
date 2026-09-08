//! The domain, against a simulated fleet.
//!
//! `petros-testkit` is the engine's own simulator, made generic so an app can
//! point it at its own mutations. This is that: no sockets, no threads, no
//! sleeps, and a seed that reproduces the run exactly.

use harken::TodoApp;
use petros_testkit::Sim;

#[test]
fn the_domain_converges_when_peers_go_dark_and_come_back() {
    let mut sim = Sim::<TodoApp>::new(19, 3);
    for round in 0..3 {
        for i in 0..sim.clients() {
            sim.mutate(i, harken::add(&format!("c{i}-{round}")));
            sim.step();
        }
    }

    sim.partition(2);
    for round in 0..4 {
        sim.mutate(2, harken::add(&format!("dark-{round}")));
        sim.mutate(0, harken::add(&format!("lit-{round}")));
        sim.step();
    }

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
        harken::list(sim.conn(0)).unwrap().len(),
        17,
        "9 shared + 4 dark + 4 lit, none lost and none duplicated"
    );
}
