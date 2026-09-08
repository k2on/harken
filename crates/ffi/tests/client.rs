//! The whole path, through the public client: a mutation authored here reaches
//! the module, the module writes the row, and swapping the module mid-session
//! changes what the next mutation does without disturbing the database.

use exo_todo_ffi::TodoClient;

const MODULE: &[u8] = exo_mutators::BUNDLED;

/// The point of all of it: a mutation made through the public client runs the
/// module, not a linked `apply` — and swapping the module changes what the very
/// next mutation does, with the database and the connection carrying on.
#[test]
fn the_client_runs_the_module() {
    let dir = std::env::temp_dir().join(format!("exo-wasm-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("peer.db").to_string_lossy().into_owned();

    let client = TodoClient::open(db, "alice".into()).expect("open");

    // Nothing works before a module is installed, and it says so.
    assert!(client.add("too early".into()).is_err());

    let generation = client
        .load_mutators(MODULE.to_vec())
        .expect("install the module");
    assert_eq!(generation, 1);
    assert_eq!(client.mutators_generation(), 1);

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
    assert_eq!(client.load_mutators(MODULE.to_vec()).unwrap(), 2);
    client.add("after the swap".into()).expect("still working");
    assert_eq!(client.list().unwrap().len(), 3);
    assert_eq!(client.list().unwrap()[2].pos, 3, "state survived the swap");

    let _ = std::fs::remove_dir_all(&dir);
}
