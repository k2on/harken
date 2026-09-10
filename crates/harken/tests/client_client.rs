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

    // The generation is process-wide and the other test in this binary loads a
    // module too, so between the call returning and the read below it can have
    // moved again. What is guaranteed is that it is at least what this install
    // was given — and further down, that a swap advances it. Asserting equality
    // here contradicted the comment above it and failed roughly one run in
    // four, on thread scheduling alone.
    let installed = client
        .load_mutators(MODULE.to_vec())
        .expect("install the module");
    assert!(
        client.mutators_generation() >= installed,
        "the generation does not go backwards: {installed}"
    );

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

/// The peer maintains the library and hands back only what moved.
///
/// The list used to cross the boundary whole on every change. This checks the
/// two agree — patch by patch, against `library()` through the same peer — and
/// that the peer settles on its own, because an app that has to remember to
/// update a view will forget on exactly the path nobody tested.
#[test]
fn the_peer_maintains_its_library() {
    use harken::PatchOp;

    let dir = std::env::temp_dir().join(format!("petros-update-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let client = Peer::open(
        dir.join("peer.db").to_string_lossy().into_owned(),
        "alice".into(),
    )
    .expect("open");
    client.load_mutators(MODULE.to_vec()).expect("install");

    // The list a caller holds, built only from what the peer sends.
    let mut held: Vec<harken::foreign::Song> = Vec::new();
    let settle = |client: &Peer, held: &mut Vec<harken::foreign::Song>| {
        let update = client.library_update().expect("update");
        if update.reset {
            *held = update.songs;
        } else {
            for patch in update.patches {
                match patch.op {
                    PatchOp::Insert => held.insert(patch.at as usize, patch.song.unwrap()),
                    PatchOp::Remove => {
                        held.remove(patch.at as usize);
                    }
                    PatchOp::Update => held[patch.at as usize] = patch.song.unwrap(),
                }
            }
        }
        let read = client.library().expect("library");
        let seen: Vec<(&str, bool)> = held
            .iter()
            .map(|s| (s.title.as_str(), s.favorited))
            .collect();
        let want: Vec<(&str, bool)> = read
            .iter()
            .map(|s| (s.title.as_str(), s.favorited))
            .collect();
        assert_eq!(seen, want);
        assert_eq!(
            update.favorites as usize,
            read.iter().filter(|s| s.favorited).count(),
            "the maintained count"
        );
    };

    // The first call is the reset: the peer hydrated at open and has never been
    // collected from.
    settle(&client, &mut held);
    assert!(held.is_empty());

    for title in ["Glue", "Apricots", "Opal"] {
        client.add_song(title.into(), "Bicep".into()).unwrap();
        settle(&client, &mut held);
    }
    assert_eq!(held.len(), 3);

    let opal = held[2].id.clone();
    client.favorite(opal.clone()).unwrap();
    settle(&client, &mut held);
    assert!(held[2].favorited, "hearting reached the held list");

    client.favorite_all().unwrap();
    settle(&client, &mut held);
    assert!(held.iter().all(|s| s.favorited));

    client.remove_song(opal).unwrap();
    settle(&client, &mut held);
    assert_eq!(held.len(), 2);

    // Nothing happened, so nothing crosses. This is the common case while a
    // socket is polled and it has to cost nothing.
    let idle = client.library_update().unwrap();
    assert!(!idle.reset);
    assert!(idle.patches.is_empty(), "an idle poll carries no rows");
    assert!(idle.songs.is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}
