//! The whole path, through the public client: a mutation authored here reaches
//! the module, the module writes the row, and swapping the module mid-session
//! changes what the next mutation does without disturbing the database.

use petros_todo_ffi::TodoClient;

const MODULE: &[u8] = petros_todo_ffi::app::BUNDLED;

/// The point of all of it: a mutation made through the public client runs the
/// module, not a linked `apply` — and swapping the module changes what the very
/// next mutation does, with the database and the connection carrying on.
#[test]
fn the_client_runs_the_module() {
    let dir = std::env::temp_dir().join(format!("petros-wasm-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("peer.db").to_string_lossy().into_owned();

    let client = TodoClient::open(db, "alice".into()).expect("open");

    // The generation is process-wide and other tests in this binary move it
    // too, so what matters is that a swap advances it, not what it reads.
    let installed = client
        .load_mutators(MODULE.to_vec())
        .expect("install the module");
    assert_eq!(client.mutators_generation(), installed);

    client
        .add("buy milk".into())
        .expect("the module accepted it");
    client.add("buy oats".into()).expect("second");
    // The row's id comes from the module, so it arrives through `list`.
    let oats = client.list().expect("list")[1].id.clone();
    client.set_done(oats, true).expect("toggle");

    let items = client.list().expect("list");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].text, "buy milk");
    assert_eq!(items[0].actor, "alice");
    assert_eq!((items[0].pos, items[1].pos), (1, 2));
    assert!(items[1].done);

    // The refusal came from inside the sandbox, through the engine, to here.
    let refused = client.add("   ".into()).expect_err("blank is refused");
    assert!(
        format!("{refused}").contains("a to-do needs some text"),
        "{refused}"
    );

    // A hot swap mid-session: same client, same database, new module.
    let swapped = client.load_mutators(MODULE.to_vec()).unwrap();
    assert!(
        swapped > installed,
        "the generation moves: {installed} -> {swapped}"
    );
    client.add("after the swap".into()).expect("still working");
    assert_eq!(client.list().unwrap().len(), 3);
    assert_eq!(client.list().unwrap()[2].pos, 3, "state survived the swap");

    let _ = std::fs::remove_dir_all(&dir);
}

/// The claim the generic entry point exists to make: a verb the module gained
/// can be authored without any Rust function naming it, and therefore without a
/// new uniffi export or a native build.
#[test]
fn a_verb_the_ffi_never_heard_of() {
    let dir = std::env::temp_dir().join(format!("petros-generic-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let client = TodoClient::open(
        dir.join("peer.db").to_string_lossy().into_owned(),
        "alice".into(),
    )
    .expect("open");
    client.load_mutators(MODULE.to_vec()).expect("install");

    client
        .mutate("Add".into(), r#"{"text":"buy milk"}"#.into())
        .unwrap();
    client
        .mutate("Add".into(), r#"{"text":"buy oats"}"#.into())
        .unwrap();
    let items = client.list().unwrap();
    assert_eq!(items.len(), 2);
    assert!(items.iter().all(|i| !i.done));

    // Nothing in `crates/ffi` mentions MarkAllDone. It reached `apply` because
    // the module knows the name, which is the point.
    client.mutate("MarkAllDone".into(), "{}".into()).unwrap();
    assert!(client.list().unwrap().iter().all(|i| i.done));

    // One entry, not one per row: the intent was "I am finished".
    assert_eq!(client.cursor(), 0, "still unconfirmed; these are pending");
    assert_eq!(client.pending_len(), 3);

    // An id argument is a string here and sixteen bytes on the wire.
    let id = client.list().unwrap()[0].id.clone();
    client
        .mutate("SetDone".into(), format!(r#"{{"id":"{id}","done":false}}"#))
        .unwrap();
    assert!(!client.list().unwrap()[0].done);

    // A verb no module has ever defined is refused, not silently dropped.
    let unknown = client.mutate("Frobnicate".into(), "{}".into()).unwrap_err();
    assert!(format!("{unknown}").contains("Frobnicate"), "{unknown}");

    // So are arguments that are not an object, and ids that are not ids.
    assert!(client.mutate("Add".into(), "[1,2,3]".into()).is_err());
    assert!(client
        .mutate(
            "SetDone".into(),
            r#"{"id":"not-a-uuid","done":true}"#.into()
        )
        .is_err());

    let _ = std::fs::remove_dir_all(&dir);
}
