//! The whole path, through the public client: a mutation authored here reaches
//! the module, the module writes the row, and swapping the module mid-session
//! changes what the next mutation does without disturbing the database.

use harken::Peer;

const MODULE: &[u8] = harken::BUNDLED;

/// The point of all of it: a mutation made through the public client runs the
/// module, not a linked `apply` — and swapping the module changes what the very
/// next mutation does, with the database and the connection carrying on.
#[test]
fn the_client_runs_the_module() {
    let dir = std::env::temp_dir().join(format!("petros-wasm-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("peer.db").to_string_lossy().into_owned();

    let client = Peer::open(db, "alice".into()).expect("open");

    // The generation is process-wide and other tests in this binary move it
    // too, so what matters is that a swap advances it, not what it reads.
    let installed = client
        .load_mutators(MODULE.to_vec())
        .expect("install the module");
    assert_eq!(client.mutators_generation(), installed);

    client
        .add_song("Glue".into(), "Bicep".into())
        .expect("the module accepted it");
    client
        .add_song("Opal".into(), "Bicep".into())
        .expect("second");
    // The row's id comes from the module, so it arrives through `list`.
    let opal = client.library().expect("library")[1].id.clone();
    client.favorite(opal).expect("heart it");

    let items = client.library().expect("library");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].title, "Glue");
    assert_eq!(items[0].actor, "alice");
    assert_eq!((items[0].pos, items[1].pos), (1, 2));
    assert!(items[1].favorited, "the second one is on the playlist");

    // The refusal came from inside the sandbox, through the engine, to here.
    let refused = client
        .add_song("   ".into(), "nobody".into())
        .expect_err("a blank title is refused");
    assert!(
        format!("{refused}").contains("a song needs a title"),
        "{refused}"
    );

    // A hot swap mid-session: same client, same database, new module.
    let swapped = client.load_mutators(MODULE.to_vec()).unwrap();
    assert!(
        swapped > installed,
        "the generation moves: {installed} -> {swapped}"
    );
    client
        .add_song("after the swap".into(), "Bicep".into())
        .expect("still working");
    assert_eq!(client.library().unwrap().len(), 3);
    assert_eq!(
        client.library().unwrap()[2].pos,
        3,
        "state survived the swap"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// The claim the generic entry point exists to make: a verb the module gained
/// can be authored without any Rust function naming it, and therefore without a
/// new uniffi export or a native build.
#[test]
fn a_verb_the_client_never_heard_of() {
    let dir = std::env::temp_dir().join(format!("petros-generic-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let client = Peer::open(
        dir.join("peer.db").to_string_lossy().into_owned(),
        "alice".into(),
    )
    .expect("open");
    client.load_mutators(MODULE.to_vec()).expect("install");

    client
        .mutate(
            "AddSong".into(),
            r#"{"title":"Glue","artist":"Bicep"}"#.into(),
        )
        .unwrap();
    client
        .mutate(
            "AddSong".into(),
            r#"{"title":"Opal","artist":"Bicep"}"#.into(),
        )
        .unwrap();
    let items = client.library().unwrap();
    assert_eq!(items.len(), 2);
    assert!(items.iter().all(|i| !i.favorited));

    // Nothing in this crate mentions FavoriteAll. It reached `apply` because
    // the module knows the name, which is the point.
    client.mutate("FavoriteAll".into(), "{}".into()).unwrap();
    assert!(client.library().unwrap().iter().all(|i| i.favorited));

    // One entry, not one per row: the intent was "I am finished".
    assert_eq!(client.cursor(), 0, "still unconfirmed; these are pending");
    assert_eq!(client.pending_len(), 3);

    // An id argument is a string here and sixteen bytes on the wire.
    let id = client.library().unwrap()[0].id.clone();
    client
        .mutate("Unfavorite".into(), format!(r#"{{"id":"{id}"}}"#))
        .unwrap();
    assert!(!client.library().unwrap()[0].favorited);

    // A verb no module has ever defined is refused, not silently dropped.
    let unknown = client.mutate("Frobnicate".into(), "{}".into()).unwrap_err();
    assert!(format!("{unknown}").contains("Frobnicate"), "{unknown}");

    // So are arguments that are not an object, and ids that are not ids.
    assert!(client.mutate("AddSong".into(), "[1,2,3]".into()).is_err());
    assert!(client
        .mutate("Unfavorite".into(), r#"{"id":"not-a-uuid"}"#.into())
        .is_err());

    let _ = std::fs::remove_dir_all(&dir);
}
