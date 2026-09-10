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

const MODULE: &[u8] = harken::BUNDLED;

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
    petros::batch(&mut conn, harken::SCHEMA).unwrap();
    let mut auto = petros::AutoCtx::seeded(1);

    let mut fill = vec![];
    let mut apply = vec![];
    for i in 0..100 {
        let raw = harken::add_song(format!("item {i}"), "Bicep".into());
        let mut bytes = Vec::new();
        ciborium::into_writer(&raw, &mut bytes).unwrap();

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
    println!("\n  one mutation on a file-backed database, by durability setting:");
    for sync in ["FULL", "NORMAL", "OFF"] {
        let dir = std::env::temp_dir().join(format!("petros-sync-{sync}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // `open_path` already sets WAL, foreign keys and the busy timeout; the
        // durability setting is the one this test varies.
        let mut conn = petros::open_path(dir.join("p.db")).unwrap();
        petros::batch(&mut conn, &format!("PRAGMA synchronous = {sync};")).unwrap();
        let mut client =
            petros::Client::<harken::HarkenApp>::open(conn, "alice", petros::AutoCtx::system())
                .unwrap();

        client
            .mutate(harken::add_song("warm".into(), "Bicep".into()))
            .unwrap();
        let mut ts = vec![];
        for i in 0..25 {
            let t = Instant::now();
            client
                .mutate(harken::add_song(format!("tap {i}"), "Bicep".into()))
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
    fn one(sync: &str, depth: usize, n: usize) -> f64 {
        let mut ts = vec![];
        for run in 0..n {
            let dir = std::env::temp_dir().join(format!(
                "petros-d{depth}-{sync}-{run}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let mut conn = petros::open_path(dir.join("p.db")).unwrap();
            petros::batch(&mut conn, &format!("PRAGMA synchronous = {sync};")).unwrap();
            let mut c =
                petros::Client::<harken::HarkenApp>::open(conn, "alice", petros::AutoCtx::system())
                    .unwrap();
            for i in 0..depth {
                c.mutate(harken::add_song(format!("filler {i}"), "Bicep".into()))
                    .unwrap();
            }
            let t = Instant::now();
            c.mutate(harken::add_song(
                "the tap being timed".into(),
                "Bicep".into(),
            ))
            .unwrap();
            ts.push(t.elapsed().as_secs_f64() * 1000.0);
            drop(c);
            let _ = std::fs::remove_dir_all(&dir);
        }
        median(ts)
    }

    println!("\n  one tap, by pending depth and durability (ms):");
    println!("      pending      FULL (today)    NORMAL");
    for depth in [0usize, 25, 50, 100, 200, 400] {
        println!(
            "      {:>7}      {:>9.1}     {:>9.1}",
            depth,
            one("FULL", depth, 7),
            one("NORMAL", depth, 7)
        );
    }
}

/// What a bulk mutation costs now that it is a loop.
///
/// `favorite_all` was one `INSERT ... SELECT` with a window function; typed
/// writes made it a scan and a write per song, because a statement that inserts
/// a thousand rows reports one result and not which rows they were — which is
/// exactly what an incremental view cannot work from. That trade is only worth
/// it if the loop is affordable, and on the phone every row in it crosses the
/// sandbox boundary. This measures both sides at three library sizes.
#[test]
#[ignore = "a measurement, not an assertion: run it with `just latency`"]
fn a_bulk_mutation_row_by_row() {
    use petros::backend::SqliteStore;
    use petros_wasm_host::Mutators;

    println!("\n  favorite_all, as a loop over N songs:");
    println!("    {:>6}  {:>10}  {:>10}", "songs", "native", "wasm");
    for n in [10, 100, 1000] {
        let mut times = Vec::new();
        for build in 0..2 {
            let mut conn = petros::open_memory().unwrap();
            petros::batch(&mut conn, harken::SCHEMA).unwrap();
            let mut auto = petros::AutoCtx::seeded(1);
            let m = Mutators::load(MODULE).unwrap();

            // Fill the library through the same path, so the rows are real.
            for i in 0..n {
                let raw = harken::add_song(format!("song {i}"), "Bicep".into());
                let mut bytes = Vec::new();
                ciborium::into_writer(&raw, &mut bytes).unwrap();
                let filled = m.fill_auto(&bytes, &mut auto).unwrap();
                m.apply(&mut conn, &filled, "alice").unwrap().unwrap();
            }

            let raw = harken::favorite_all();
            let mut bytes = Vec::new();
            ciborium::into_writer(&raw, &mut bytes).unwrap();
            let filled = m.fill_auto(&bytes, &mut auto).unwrap();

            let t = Instant::now();
            if build == 0 {
                let payload: harken::Payload = ciborium::from_reader(&filled[..])
                    .map(harken::Payload)
                    .unwrap();
                harken::apply(&mut SqliteStore::new(&mut conn), &payload.0, "alice").unwrap();
            } else {
                m.apply(&mut conn, &filled, "alice").unwrap().unwrap();
            }
            times.push(t.elapsed().as_secs_f64() * 1000.0);

            // The playlist is the whole library, or the loop skipped rows.
            assert_eq!(
                harken::favorites(&mut SqliteStore::new(&mut conn))
                    .unwrap()
                    .len(),
                n
            );
        }
        println!("    {:>6}  {:>7.2} ms  {:>7.2} ms", n, times[0], times[1]);
    }
}

/// What the maintained view actually bought, on the path a client takes.
///
/// The microbenchmarks in `petros-ivm` time an operator. This times what the
/// desktop client does after a tap: take the changes, bring the view up to
/// date, and produce the list a screen renders — against the same thing done by
/// re-running the query, which is what it did before.
#[test]
#[ignore = "a measurement, not an assertion: run it with `just latency`"]
fn maintained_against_re_read_on_the_client_path() {
    use petros::Changes;

    println!("\n  one tap, then the list a screen renders:");
    println!(
        "    {:>7}  {:>12}  {:>12}  {:>7}",
        "songs", "re-read", "maintained", "ratio"
    );

    for n in [10usize, 100, 1_000] {
        let mut client = petros::Client::<harken::HarkenApp>::open(
            petros::open_memory().unwrap(),
            "alice",
            petros::AutoCtx::seeded(1),
        )
        .unwrap();
        for i in 0..n {
            client
                .mutate(harken::add_song(format!("song {i}"), "Bicep".into()))
                .unwrap();
        }

        let mut view = harken::library_view();
        {
            let mut store = client.store();
            view.hydrate(&mut store);
        }
        let _ = client.take_changes();
        let mut rendered = harken::songs_of(&view);

        let (mut fresh, mut kept) = (vec![], vec![]);
        for i in 0..30 {
            client
                .mutate(harken::add_song(format!("tap {i}"), "Bicep".into()))
                .unwrap();

            let changes = client.take_changes();
            let t = Instant::now();
            match changes {
                Changes::Applied(changes) => {
                    let patches = {
                        let mut store = client.store();
                        view.apply(&mut store, &changes)
                    };
                    harken::patch(&mut rendered, &patches);
                }
                Changes::Rebuilt => {
                    {
                        let mut store = client.store();
                        view.hydrate(&mut store);
                    }
                    rendered = harken::songs_of(&view);
                }
            }
            kept.push(t.elapsed().as_secs_f64() * 1000.0);

            let t = Instant::now();
            let _ = harken::library(&mut client.store()).unwrap();
            fresh.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        let (f, k) = (median(fresh), median(kept));
        println!(
            "    {:>7}  {:>9.3} ms  {:>9.3} ms  {:>6.1}x",
            n,
            f,
            k,
            f / k
        );
    }
}

/// What crosses the boundary, in bytes and in rows.
///
/// The phone's cost is not the query — it is the list crossing UniFFI on every
/// change. This is the half that can be measured from here: what the peer hands
/// over. What React Native then does with it is not measured, and the numbers
/// below are a floor rather than the whole story.
///
/// The rows are the honest column. Bytes are counted by encoding the same
/// values with ciborium, which is not UniFFI's format — it is a stand-in with
/// the same shape, and what matters is how it scales rather than its constant.
#[test]
#[ignore = "a measurement, not an assertion: run it with `just latency`"]
fn what_crosses_the_boundary() {
    use harken::Peer;

    println!("\n  one tap, then what the peer hands the phone:");
    println!(
        "    {:>7}  {:>20}  {:>20}",
        "songs", "library()", "library_update()"
    );

    for n in [10usize, 100, 1_000] {
        let dir = std::env::temp_dir().join(format!("petros-cross-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let peer = Peer::open(
            dir.join("peer.db").to_string_lossy().into_owned(),
            "alice".into(),
        )
        .unwrap();
        peer.load_mutators(harken::BUNDLED.to_vec()).unwrap();

        for i in 0..n {
            peer.add_song(format!("song {i}"), "Bicep".into()).unwrap();
        }
        let _ = peer.library_update().unwrap();

        let (mut whole, mut moved) = (0usize, 0usize);
        let (mut whole_rows, mut moved_rows) = (0usize, 0usize);
        for i in 0..20 {
            peer.add_song(format!("tap {i}"), "Bicep".into()).unwrap();

            let update = peer.library_update().unwrap();
            moved_rows += update.patches.len();
            moved += size_of_songs(update.patches.iter().filter_map(|p| p.song.as_ref()));

            let all = peer.library().unwrap();
            whole_rows += all.len();
            whole += size_of_songs(all.iter());
        }
        println!(
            "    {:>7}  {:>6} rows {:>8} B  {:>6} rows {:>8} B",
            n,
            whole_rows / 20,
            whole / 20,
            moved_rows / 20,
            moved / 20,
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// A stand-in for what the boundary charges: the same values, encoded.
fn size_of_songs<'a>(songs: impl Iterator<Item = &'a harken::foreign::Song>) -> usize {
    songs
        .map(|s| {
            let mut out = Vec::new();
            ciborium::into_writer(
                &(
                    &s.id,
                    &s.title,
                    &s.artist,
                    s.pos,
                    s.added_ms,
                    &s.actor,
                    s.favorite_pos,
                    s.favorited,
                ),
                &mut out,
            )
            .unwrap();
            out.len()
        })
        .sum()
}
