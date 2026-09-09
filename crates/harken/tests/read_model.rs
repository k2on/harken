//! The read model, against rows a real `apply` produced.

use petros::{AutoCtx, Client};

#[test]
fn library_and_favorites_agree_with_what_apply_wrote() {
    let mut c = Client::<harken::HarkenApp>::open(
        petros::open_memory().unwrap(),
        "alice",
        AutoCtx::seeded(7),
    )
    .unwrap();

    for (t, a) in [("Glue", "Bicep"), ("Opal", "Bicep"), ("Gosh", "Jamie xx")] {
        c.mutate(harken::add_song(t.into(), a.into())).unwrap();
    }
    let all = harken::library(&mut c.store()).unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(
        all.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(),
        vec!["Glue", "Opal", "Gosh"],
        "library order is s.pos, and pos counts up from MAX(pos)"
    );
    assert_eq!(all[0].artist, "Bicep");
    assert_eq!(all[0].actor, "alice");
    assert!(all.iter().all(|s| !s.favorited()), "nothing hearted yet");
    assert!(harken::favorites(&mut c.store()).unwrap().is_empty());

    // Heart the third, then the first: the playlist is in the order they were
    // hearted, not the order they were added.
    let third = *all[2].id.as_uuid().as_bytes();
    let first = *all[0].id.as_uuid().as_bytes();
    c.mutate(harken::favorite(third.to_vec())).unwrap();
    c.mutate(harken::favorite(first.to_vec())).unwrap();

    let playlist = harken::favorites(&mut c.store()).unwrap();
    assert_eq!(
        playlist
            .iter()
            .map(|s| s.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Gosh", "Glue"],
        "playlist order is f.pos"
    );
    assert_eq!(playlist[0].favorite_pos, Some(1));
    assert_eq!(playlist[1].favorite_pos, Some(2));

    // The library keeps its own order, and carries the playlist position on the
    // rows that have one. That is the left join, and the `?` that makes it an
    // Option.
    let all = harken::library(&mut c.store()).unwrap();
    assert_eq!(
        all.iter().map(|s| s.favorite_pos).collect::<Vec<_>>(),
        vec![Some(2), None, Some(1)]
    );
    assert_eq!(all[0].id, playlist[1].id, "ids survive the blob round trip");

    // Unhearting takes it off the playlist and leaves the song.
    c.mutate(harken::unfavorite(first.to_vec())).unwrap();
    assert_eq!(harken::library(&mut c.store()).unwrap().len(), 3);
    assert_eq!(harken::favorites(&mut c.store()).unwrap().len(), 1);
}
