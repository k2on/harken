//! Where the time goes in one tap.
//!
//! These measure the *native* path, which is what the server and the terminal
//! peers run: `apply` is a function call, not a module. `the_cost_of_the_thread`
//! measures the wasm path the phone takes, so the two can be compared.
//!
//! Measurements rather than assertions, so they are `#[ignore]`d: the suite has
//! to stay under thirty seconds and the depth sweep alone takes a minute.
//!
//!     just latency
//!
//! They are checked in because the numbers decided real changes — rendering on
//! the tick rather than the tap, `synchronous = FULL`, a thread per wasm call —
//! and the next person to wonder should be able to re-run them rather than
//! trust a commit message.

use std::time::Instant;

const MODULE: &[u8] = harken_ffi::app::BUNDLED;

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

/// How much of a mutation is the thread, and how much is the work?
#[test]
#[ignore = "a measurement, not an assertion: run it with `just latency`"]
fn the_cost_of_the_thread() {
    use petros_wasm_host::Mutators;

    let m = Mutators::load(MODULE).unwrap();
    let mut conn = petros::open_memory().unwrap();
    petros::diesel::connection::SimpleConnection::batch_execute(&mut conn, harken::SCHEMA).unwrap();
    let mut auto = petros::AutoCtx::seeded(1);

    let mut fill = vec![];
    let mut apply = vec![];
    for i in 0..100 {
        let raw = harken::add_song(&format!("item {i}"), "Bicep");
        let mut bytes = Vec::new();
        ciborium::into_writer(&raw.0, &mut bytes).unwrap();

        let t = Instant::now();
        let filled = m.fill_auto(&bytes, &mut auto).unwrap();
        fill.push(t.elapsed().as_secs_f64() * 1000.0);

        let t = Instant::now();
        m.apply(&mut conn, &filled, "alice").unwrap().unwrap();
        apply.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    println!("\n  one wasm call, thread + instantiate + work:");
    println!("    fill_auto (no sql)   {:>7.3} ms", median(fill));
    println!("    apply (3 sql calls)  {:>7.3} ms", median(apply));
}

/// Is the cost the wasm, or is it the disk?
///
/// Every local write commits its intent durably on its own — `docs/decisions.md`
/// explains why — and in WAL mode `synchronous = FULL` fsyncs on each of those.
/// This times the same mutation with the pragma at each setting.
#[test]
#[ignore = "a measurement, not an assertion: run it with `just latency`"]
fn fsync_or_wasm() {
    use petros::diesel::connection::SimpleConnection;
    use petros::diesel::Connection as _;

    println!("\n  one mutation on a file-backed database, by durability setting:");
    for sync in ["FULL", "NORMAL", "OFF"] {
        let dir = std::env::temp_dir().join(format!("petros-sync-{sync}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let mut conn = petros::Connection::establish(&dir.join("p.db").to_string_lossy()).unwrap();
        conn.batch_execute(&format!(
            "PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON; \
             PRAGMA busy_timeout = 5000; PRAGMA synchronous = {sync};"
        ))
        .unwrap();
        let mut client =
            petros::Client::<harken::HarkenApp>::open(conn, "alice", petros::AutoCtx::system())
                .unwrap();

        client.mutate(harken::add_song("warm", "Bicep")).unwrap();
        let mut ts = vec![];
        for i in 0..25 {
            let t = Instant::now();
            client
                .mutate(harken::add_song(&format!("tap {i}"), "Bicep"))
                .unwrap();
            ts.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        println!("    synchronous = {:<7} {:>7.2} ms", sync, median(ts));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// One mutation at a *fixed* pending depth.
///
/// Timing a run of mutations gets this wrong: while a peer is offline nothing
/// is acked, so pending grows under the benchmark and the median mixes depths.
/// Each sample here builds its own peer to the depth, then times a single tap.
#[test]
#[ignore = "a measurement, not an assertion: run it with `just latency`"]
fn one_tap_at_a_fixed_depth() {
    use petros::diesel::connection::SimpleConnection;
    use petros::diesel::Connection as _;

    fn one(sync: &str, depth: usize, n: usize) -> f64 {
        let mut ts = vec![];
        for run in 0..n {
            let dir = std::env::temp_dir().join(format!(
                "petros-d{depth}-{sync}-{run}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let mut conn =
                petros::Connection::establish(&dir.join("p.db").to_string_lossy()).unwrap();
            conn.batch_execute(&format!(
                "PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000; \
                 PRAGMA synchronous = {sync};"
            ))
            .unwrap();
            let mut c =
                petros::Client::<harken::HarkenApp>::open(conn, "alice", petros::AutoCtx::system())
                    .unwrap();
            for i in 0..depth {
                c.mutate(harken::add_song(&format!("filler {i}"), "Bicep"))
                    .unwrap();
            }
            let t = Instant::now();
            c.mutate(harken::add_song("the tap being timed", "Bicep"))
                .unwrap();
            ts.push(t.elapsed().as_secs_f64() * 1000.0);
            drop(c);
            let _ = std::fs::remove_dir_all(&dir);
        }
        median(ts)
    }

    println!("\n  one tap, by pending depth and durability (ms):");
    println!("      pending      FULL (today)    NORMAL");
    for depth in [0usize, 1, 5, 10, 25, 50] {
        println!(
            "      {:>7}      {:>9.1}     {:>9.1}",
            depth,
            one("FULL", depth, 7),
            one("NORMAL", depth, 7)
        );
    }
}
