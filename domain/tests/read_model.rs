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

/// The maintained library has to say exactly what the read one says, and the
/// list a screen holds has to say exactly what the view says.
///
/// This is what the iced client depends on: it hydrates once at boot, is told
/// what changed after that, and splices its own `Vec<Song>` from the patches.
/// If any of the three drift the screen is wrong and nothing else notices.
#[test]
fn the_maintained_library_agrees_with_the_read_one() {
    use petros::Changes;

    let mut client = petros::Client::<harken::HarkenApp>::open(
        petros::open_memory().unwrap(),
        "alice",
        petros::AutoCtx::seeded(7),
    )
    .unwrap();

    let mut view = harken::library_view();
    let mut favorites = harken::favorite_count();
    {
        let mut store = client.store();
        view.hydrate(&mut store);
        favorites.hydrate(&mut store);
    }
    let _ = client.take_changes();
    // The rendered list, decoded once and spliced from then on.
    let mut rendered = harken::songs_of(&view);

    fn shown(songs: &[harken::Song]) -> Vec<(String, bool, i64)> {
        songs
            .iter()
            .map(|s| (s.title.clone(), s.favorited(), s.pos))
            .collect()
    }

    let settle = |client: &mut petros::Client<harken::HarkenApp>,
                  view: &mut harken::LibraryView,
                  favorites: &mut harken::FavoriteCount,
                  rendered: &mut Vec<harken::Song>| {
        match client.take_changes() {
            Changes::Applied(changes) => {
                let patches = {
                    let mut store = client.store();
                    favorites.apply(&mut store, &changes);
                    view.apply(&mut store, &changes)
                };
                harken::patch(rendered, &patches);
            }
            Changes::Rebuilt => {
                {
                    let mut store = client.store();
                    view.hydrate(&mut store);
                    favorites.hydrate(&mut store);
                }
                *rendered = harken::songs_of(view);
            }
        }
        let read = harken::library(&mut client.store()).unwrap();
        assert_eq!(shown(rendered), shown(&harken::songs_of(view)), "spliced");
        assert_eq!(shown(rendered), shown(&read), "against a re-read");
        // The status line's number, against the list it used to count.
        assert_eq!(
            favorites.get(),
            read.iter().filter(|s| s.favorited()).count(),
            "the tally against a count of the rows"
        );
    };

    for title in ["Glue", "Apricots", "Opal"] {
        client
            .mutate(harken::add_song(title.into(), "Bicep".into()))
            .unwrap();
        settle(&mut client, &mut view, &mut favorites, &mut rendered);
    }

    let opal = harken::library(&mut client.store()).unwrap()[2]
        .id
        .0
        .as_bytes()
        .to_vec();
    client.mutate(harken::favorite(opal.clone())).unwrap();
    settle(&mut client, &mut view, &mut favorites, &mut rendered);
    assert!(
        rendered[2].favorited(),
        "hearting reached the rendered list"
    );

    client.mutate(harken::unfavorite(opal)).unwrap();
    settle(&mut client, &mut view, &mut favorites, &mut rendered);
    assert!(!rendered[2].favorited());

    client.mutate(harken::favorite_all()).unwrap();
    settle(&mut client, &mut view, &mut favorites, &mut rendered);
    assert!(rendered.iter().all(|s| s.favorited()));

    let glue = harken::library(&mut client.store()).unwrap()[0]
        .id
        .0
        .as_bytes()
        .to_vec();
    client.mutate(harken::remove_song(glue)).unwrap();
    settle(&mut client, &mut view, &mut favorites, &mut rendered);
    assert_eq!(rendered.len(), 2);
}
