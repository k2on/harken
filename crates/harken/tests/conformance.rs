//! The two builds of one `apply` have to agree.
//!
//! `crates/harken` holds the domain. The server and the terminal peers link it
//! and call it; the phone loads it compiled to wasm so a new mutation reaches
//! it over Metro without a native build. That is two builds of one source,
//! which is not two implementations — but "not two implementations" is a claim,
//! and this is what makes it a checked one.
//!
//! Every verb, through both, against real SQLite, comparing the rows.

use petros::backend::SqliteStore;
use petros::{AutoCtx, Connection};
use petros_wasm_host::Mutators;

/// The module under test, read straight from where `just mutators` puts it.
/// A test fixture rather than part of the crate: `petros-wasm-host` runs
/// modules and has no idea which one you mean.
const MODULE: &[u8] = include_bytes!("../../../target/wasm32-unknown-unknown/mutators/harken.wasm");

/// One row of both tables at once, so a difference in either shows up here.
#[derive(Debug, PartialEq)]
struct Row {
    id: String,
    title: String,
    artist: String,
    pos: i64,
    /// The playlist position, or 0 for a song that is not on it.
    fav: i64,
    actor: String,
}

fn database() -> Connection {
    let mut conn = petros::open_memory().expect("open");
    petros::batch(&mut conn, harken::SCHEMA).expect("migrate");
    conn
}

fn rows(conn: &mut Connection) -> Vec<Row> {
    let mut store = SqliteStore(conn);
    // `hex()` and `COALESCE()` are expressions, so SQLite has no declared type
    // for either and both are named here.
    petros_sql::query!(
        store,
        "SELECT hex(s.id) AS \"id: Text\", s.title, s.artist, s.pos, s.actor,
                COALESCE(f.pos, 0) AS \"fav: Int\"
           FROM song s LEFT JOIN favorite f ON f.song_id = s.id
          ORDER BY s.pos, s.id"
    )
    .into_iter()
    .map(|r| Row {
        id: r.id,
        title: r.title,
        artist: r.artist,
        pos: r.pos,
        fav: r.fav,
        actor: r.actor,
    })
    .collect()
}

/// The native side, exactly as `harken::Payload`'s `Mutation::apply` runs it:
/// the same checked SQL, through a store backed by a real connection.
fn native_apply(
    conn: &mut Connection,
    payload: &harken::Payload,
    actor: &str,
) -> Result<(), String> {
    harken::domain::apply(&mut petros::backend::SqliteStore(conn), &payload.0, actor)
}

fn encode(p: &harken::Payload) -> Vec<u8> {
    let mut out = Vec::new();
    ciborium::into_writer(&p.0, &mut out).unwrap();
    out
}

/// A session of mutations, run both ways from the same authored payloads.
///
/// The payloads are filled once and handed to both, so that `apply` is compared
/// against the same input rather than against two different rolls of the dice.
/// That leaves `fill_auto` itself uncovered, which
/// [`fill_auto_agrees_between_the_two_builds`] exists to close — a gap found by
/// trying to make this test fail and watching it pass.
fn both_ways(script: &[(&str, serde_json::Value)]) -> (Vec<Row>, Vec<Row>) {
    let mut auto = AutoCtx::seeded(4);
    let payloads: Vec<harken::Payload> = script
        .iter()
        .map(|(kind, args)| {
            let mut p = harken::from_value(kind, args.clone()).expect("author");
            <harken::Payload as petros::Mutation>::fill_auto(&mut p, &mut auto);
            p
        })
        .collect();

    let mut native_db = database();
    for p in &payloads {
        // A refusal is a legitimate outcome; both sides must reach the same one.
        let _ = native_apply(&mut native_db, p, "alice");
    }

    let module = Mutators::load(MODULE).expect("load the module");
    let mut wasm_db = database();
    for p in &payloads {
        let _ = module
            .apply(&mut wasm_db, &encode(p), "alice")
            .expect("the host ran");
    }

    (rows(&mut native_db), rows(&mut wasm_db))
}

#[test]
fn every_verb_produces_the_same_rows_natively_and_in_wasm() {
    use serde_json::json;

    let ghost = "67e55084-765d-446c-9191-4ff9861f6d8e";
    let script: Vec<(&str, serde_json::Value)> = vec![
        ("AddSong", json!({ "title": "Glue", "artist": "Bicep" })),
        (
            "AddSong",
            json!({ "title": "  Opal  ", "artist": "  Bicep  " }),
        ),
        // Refused by both, and refused identically.
        ("AddSong", json!({ "title": "   ", "artist": "nobody" })),
        ("AddAlbum", json!({})),
        ("FavoriteAll", json!({})),
        ("AddSong", json!({ "title": "Aura", "artist": "Bicep" })),
        // A song nobody has: a no-op, not an error.
        ("Favorite", json!({ "id": ghost })),
        ("Unfavorite", json!({ "id": ghost })),
        ("RemoveSong", json!({ "id": ghost })),
        // A verb neither build knows.
        ("Frobnicate", json!({})),
    ];

    let (native, wasm) = both_ways(&script);

    assert_eq!(native, wasm, "the two builds of `apply` disagree");
    assert!(
        !native.is_empty(),
        "the script should have written something"
    );
    // And the rows are the ones the script describes, so a shared bug that
    // wrote nothing at all could not pass.
    let titles: Vec<&str> = native.iter().map(|r| r.title.as_str()).collect();
    assert_eq!(
        titles,
        vec!["Glue", "Opal", "Track 1", "Track 2", "Track 3", "Track 4", "Track 5", "Aura"]
    );
    assert_eq!(native[1].artist, "Bicep", "trimmed on the way in");
    // `FavoriteAll` swept the seven that existed then, in library order, and
    // the song added afterwards is not on the playlist.
    let places: Vec<i64> = native.iter().map(|r| r.fav).collect();
    assert_eq!(places, vec![1, 2, 3, 4, 5, 6, 7, 0]);
}

#[test]
fn refusals_match_too() {
    let module = Mutators::load(MODULE).expect("load");
    let mut auto = AutoCtx::seeded(11);

    for (kind, args) in [
        (
            "AddSong",
            serde_json::json!({ "title": "", "artist": "nobody" }),
        ),
        ("Frobnicate", serde_json::json!({})),
    ] {
        let mut p = harken::from_value(kind, args).expect("author");
        <harken::Payload as petros::Mutation>::fill_auto(&mut p, &mut auto);

        let mut a = database();
        let native = native_apply(&mut a, &p, "alice");
        let mut b = database();
        let wasm = module
            .apply(&mut b, &encode(&p), "alice")
            .expect("host ran");

        assert_eq!(
            native, wasm,
            "{kind} is refused differently by the two builds"
        );
        assert!(native.is_err(), "{kind} should be refused");
    }
}

/// `fill_auto` has to agree too, and the test above cannot see it.
///
/// It runs once, at the authoring peer, and both sides then apply whatever it
/// produced — so a difference there is invisible to a comparison of `apply`.
/// It is also the one place non-determinism is allowed, which makes it the
/// worst place for the two builds to drift: the log would freeze whichever
/// answer the authoring peer happened to be built with.
#[test]
fn fill_auto_agrees_between_the_two_builds() {
    let module = Mutators::load(MODULE).expect("load");

    for kind in ["AddSong", "AddAlbum", "FavoriteAll", "Favorite"] {
        let args = match kind {
            "Favorite" => serde_json::json!({ "id": "67e55084-765d-446c-9191-4ff9861f6d8e" }),
            "AddSong" => serde_json::json!({ "title": "Glue", "artist": "Bicep" }),
            _ => serde_json::json!({}),
        };

        // The same seed both ways: the uuid and the clock are the input, not
        // the thing being compared.
        let seeded = || {
            let mut a = AutoCtx::seeded(19);
            let id = a.uuid().as_uuid().as_bytes().to_vec();
            (id, a.now_ms())
        };
        let (uuid, now) = seeded();

        let mut native = harken::from_value(kind, args.clone()).expect("author").0;
        harken::domain::fill_auto(&mut native, uuid.clone(), now);

        let authored = harken::from_value(kind, args).expect("author");
        let from_wasm = module
            .fill_auto_with(&encode(&authored), &uuid, now)
            .expect("the module filled it");
        let from_wasm: ciborium::value::Value =
            ciborium::from_reader(from_wasm.as_slice()).expect("decode");

        assert_eq!(
            native, from_wasm,
            "`{kind}` is filled differently by the two builds"
        );
    }
}
