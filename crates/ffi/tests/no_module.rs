//! A peer with no module cannot mutate.
//!
//! Its own file on purpose. The loaded module is process-wide — `petros` calls
//! `Mutation::apply` during a rebase and hands it no context, so there is one
//! domain per process, exactly as there was when `apply` was a linked symbol.
//! That makes "nothing is loaded yet" a state only a fresh process can observe,
//! and cargo gives each test file one.

use harken_ffi::HarkenClient;

#[test]
fn mutating_without_a_module_is_refused_rather_than_ignored() {
    let dir = std::env::temp_dir().join(format!("petros-nomodule-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let client = HarkenClient::open(
        dir.join("peer.db").to_string_lossy().into_owned(),
        "alice".into(),
    )
    .expect("open");

    assert_eq!(client.mutators_generation(), 0);
    let refused = client
        .add_song("too early".into(), "nobody".into())
        .expect_err("no module is loaded");
    assert!(
        format!("{refused}").contains("no mutator module"),
        "the failure should say why: {refused}"
    );
    assert!(client.library().expect("library").is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}
