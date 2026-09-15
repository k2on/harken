//! The two builds of one `apply` have to agree.
//!
//! `domain` holds the domain. The server and the terminal peers link it
//! and call it; the phone loads it compiled to wasm so a new mutation reaches
//! it over Metro without a native build. That is two builds of one source,
//! which is not two implementations — but "not two implementations" is a claim,
//! and this is what makes it a checked one.
//!
//! Every verb, through both, against real SQLite, comparing the rows.

use petros::backend::SqliteStore;
use petros::{AutoCtx, Connection};
use petros_wasm_host::Mutators;

/// The module under test, read straight from where `nix run .#mutators` puts it.
/// A test fixture rather than part of the crate: `petros-wasm-host` runs
/// modules and has no idea which one you mean.
const MODULE: &[u8] = include_bytes!("../../target/wasm32-unknown-unknown/mutators/harken.wasm");

/// One row of both tables at once, so a difference in either shows up here.
#[derive(Debug, PartialEq)]
struct Row {
    id: String,
    title: String,
    creator: String,
    pos: i64,
    /// The playlist position, or 0 for a song that is not on it.
    fav: i64,
    user_id: String,
}

fn database() -> Connection {
    let mut conn = petros::open_memory().expect("open");
    petros::batch(&mut conn, harken::SCHEMA).expect("migrate");
    conn
}

fn rows(conn: &mut Connection, playlist: harken::Id<harken::tables::Playlist>) -> Vec<Row> {
    // Through the app's own read model, which is the thing both builds have to
    // agree about. It reads a song with its favorite hanging off it, so one
    // pass covers the library and the playlist it is read against.
    harken::library(&mut SqliteStore::new(conn), playlist)
        .expect("read")
        .into_iter()
        .map(|s| Row {
            id: s.id.to_string(),
            title: s.title,
            creator: s.creator,
            pos: s.pos,
            fav: s.playlist_pos.unwrap_or(0),
            user_id: s.user_id,
        })
        .collect()
}

/// The native side, exactly as `harken::Payload`'s `Mutation::apply` runs it:
/// the same typed writes, through a store backed by a real connection.
fn native_apply(
    conn: &mut Connection,
    payload: &harken::Payload,
    actor: &str,
) -> Result<(), String> {
    harken::apply(
        &mut SqliteStore::new(conn),
        &payload.0,
        &petros::Ctx::from_user(actor),
    )
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
fn both_ways(script: &[(&str, serde_json::Value)]) -> (Connection, Connection) {
    let mut auto = AutoCtx::seeded(4);
    let payloads: Vec<harken::Payload> = script
        .iter()
        .map(|(kind, args)| {
            let mut p = match harken::from_value(kind, args.clone()) {
                Ok(p) => p,
                Err(e) if e.starts_with("no verb named") => undeclared(kind),
                Err(e) => panic!("author {kind}: {e}"),
            };
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
            .apply(&mut wasm_db, &encode(p), &petros::Ctx::from_user("alice"))
            .expect("the host ran");
    }

    (native_db, wasm_db)
}

#[test]
fn every_verb_produces_the_same_rows_natively_and_in_wasm() {
    use serde_json::json;

    let ghost = "67e55084-765d-446c-9191-4ff9861f6d8e";
    let script: Vec<(&str, serde_json::Value)> = vec![
        ("CreatePlaylist", json!({ "name": "Favorites" })),
        ("AddSong", json!({ "title": "Glue", "artist": "Bicep" })),
        (
            "AddSong",
            json!({ "title": "  Opal  ", "artist": "  Bicep  " }),
        ),
        // Refused by both, and refused identically.
        ("AddSong", json!({ "title": "   ", "artist": "nobody" })),
    ];
    let (mut native_db, mut wasm_db) = both_ways(&script);

    // Both builds invented the same playlist id from the same seeded
    // `fill_auto`, which is itself worth asserting: everything after this
    // depends on the two agreeing about it.
    let native_list = harken::playlists(&mut SqliteStore::new(&mut native_db)).unwrap();
    let wasm_list = harken::playlists(&mut SqliteStore::new(&mut wasm_db)).unwrap();
    assert_eq!(native_list.len(), 1);
    assert_eq!(
        native_list[0].id, wasm_list[0].id,
        "the two builds chose different ids"
    );
    let favs = native_list[0].id;
    let favs_str = favs.to_string();

    // The rest of the script, now that there is a playlist to name.
    let rest: Vec<(&str, serde_json::Value)> = vec![
        ("AddAllToPlaylist", json!({ "playlist_id": favs_str })),
        ("AddSong", json!({ "title": "Aura", "artist": "Bicep" })),
        // Something nobody has: a no-op, not an error.
        (
            "AddToPlaylist",
            json!({ "playlist_id": favs_str, "media_id": ghost }),
        ),
        (
            "RemoveFromPlaylist",
            json!({ "playlist_id": favs_str, "media_id": ghost }),
        ),
        ("RemoveMedia", json!({ "id": ghost })),
        // A verb neither build knows.
        ("Frobnicate", json!({})),
    ];
    let mut auto = AutoCtx::seeded(9);
    let module = Mutators::load(MODULE).expect("load the module");
    for (kind, args) in &rest {
        let mut p = match harken::from_value(kind, args.clone()) {
            Ok(p) => p,
            Err(e) if e.starts_with("no verb named") => undeclared(kind),
            Err(e) => panic!("author {kind}: {e}"),
        };
        <harken::Payload as petros::Mutation>::fill_auto(&mut p, &mut auto);
        let _ = native_apply(&mut native_db, &p, "alice");
        let _ = module.apply(&mut wasm_db, &encode(&p), &petros::Ctx::from_user("alice"));
    }

    let key = favs;
    let native = rows(&mut native_db, key);
    let wasm = rows(&mut wasm_db, key);

    assert_eq!(native, wasm, "the two builds of `apply` disagree");
    assert!(
        !native.is_empty(),
        "the script should have written something"
    );
    // And the rows are the ones the script describes, so a shared bug that
    // wrote nothing at all could not pass.
    let titles: Vec<&str> = native.iter().map(|r| r.title.as_str()).collect();
    assert_eq!(titles, vec!["Glue", "Opal", "Aura"]);
    assert_eq!(native[1].creator, "Bicep", "trimmed on the way in");
    // `AddAllToPlaylist` swept the two that existed then, in library order, and
    // the song added afterwards is not on the playlist.
    let places: Vec<i64> = native.iter().map(|r| r.fav).collect();
    assert_eq!(places, vec![1, 2, 0]);
}

/// A payload for a verb this app does not declare.
///
/// `from_value` refuses one now, which is where an undeclared verb should be
/// caught — so getting one as far as `apply` means building the map by hand.
/// What is being compared is that both builds refuse it and say the same
/// thing, and that is a property of `apply` rather than of the authoring
/// helper that normally stops it getting here.
fn undeclared(kind: &str) -> harken::Payload {
    harken::Payload(ciborium::value::Value::Map(vec![(
        ciborium::value::Value::Text("t".into()),
        ciborium::value::Value::Text(kind.into()),
    )]))
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
        let mut p = match harken::from_value(kind, args) {
            Ok(p) => p,
            // Only an undeclared verb may skip authoring; a declared one that
            // fails to author is a bug in this script.
            Err(e) if e.starts_with("no verb named") => undeclared(kind),
            Err(e) => panic!("author {kind}: {e}"),
        };
        <harken::Payload as petros::Mutation>::fill_auto(&mut p, &mut auto);

        let mut a = database();
        let native = native_apply(&mut a, &p, "alice");
        let mut b = database();
        let wasm = module
            .apply(&mut b, &encode(&p), &petros::Ctx::from_user("alice"))
            .expect("host ran");

        assert_eq!(
            native,
            wasm.map(|_| ()),
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

    // One verb of each shape `fill_auto` has to handle: an id and a clock, a
    // clock alone, and neither.
    for kind in ["CreatePlaylist", "AddToPlaylist", "RemoveMedia"] {
        let id = "67e55084-765d-446c-9191-4ff9861f6d8e";
        let args = match kind {
            "CreatePlaylist" => serde_json::json!({ "name": "Favorites" }),
            "AddToPlaylist" => {
                serde_json::json!({ "playlist_id": id, "media_id": id })
            }
            _ => serde_json::json!({ "id": id }),
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
        harken::fill_auto(&mut native, uuid.clone(), now);

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
