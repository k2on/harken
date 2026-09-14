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
    let favs = favourites(&mut c);

    for (t, a) in [("Glue", "Bicep"), ("Opal", "Bicep"), ("Gosh", "Jamie xx")] {
        c.mutate(harken::add_song(
            t.into(),
            a.into(),
            String::new(),
            0,
            String::new(),
        ))
        .unwrap();
    }
    let all = harken::library(&mut c.store(), favs.clone()).unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(
        all.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(),
        vec!["Glue", "Opal", "Gosh"],
        "library order is s.pos, and pos counts up from MAX(pos)"
    );
    assert_eq!(all[0].creator, "Bicep");
    assert_eq!(all[0].user_id, "alice");
    assert!(all.iter().all(|s| !s.on_playlist()), "nothing hearted yet");
    assert!(harken::playlist(&mut c.store(), favs.clone())
        .unwrap()
        .is_empty());

    // Heart the third, then the first: the playlist is in the order they were
    // hearted, not the order they were added.
    let third = *all[2].id.as_uuid().as_bytes();
    let first = *all[0].id.as_uuid().as_bytes();
    c.mutate(harken::add_to_playlist(favs.clone(), third.to_vec()))
        .unwrap();
    c.mutate(harken::add_to_playlist(favs.clone(), first.to_vec()))
        .unwrap();

    let playlist = harken::playlist(&mut c.store(), favs.clone()).unwrap();
    assert_eq!(
        playlist
            .iter()
            .map(|s| s.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Gosh", "Glue"],
        "playlist order is f.pos"
    );
    assert_eq!(playlist[0].playlist_pos, Some(1));
    assert_eq!(playlist[1].playlist_pos, Some(2));

    // The library keeps its own order, and carries the playlist position on the
    // rows that have one. That is the left join, and the `?` that makes it an
    // Option.
    let all = harken::library(&mut c.store(), favs.clone()).unwrap();
    assert_eq!(
        all.iter().map(|s| s.playlist_pos).collect::<Vec<_>>(),
        vec![Some(2), None, Some(1)]
    );
    assert_eq!(all[0].id, playlist[1].id, "ids survive the blob round trip");

    // Unhearting takes it off the playlist and leaves the song.
    c.mutate(harken::remove_from_playlist(favs.clone(), first.to_vec()))
        .unwrap();
    assert_eq!(
        harken::library(&mut c.store(), favs.clone()).unwrap().len(),
        3
    );
    assert_eq!(
        harken::playlist(&mut c.store(), favs.clone())
            .unwrap()
            .len(),
        1
    );
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
    let favs = favourites(&mut client);

    let mut view = harken::library_view(&favs);
    let mut favorites = harken::playlist_count(&favs);
    {
        let mut store = client.store();
        view.hydrate(&mut store);
        favorites.hydrate(&mut store);
    }
    let _ = client.take_changes();
    // The rendered list, decoded once and spliced from then on.
    let mut rendered = harken::items_of(&view);

    fn shown(songs: &[harken::Item]) -> Vec<(String, bool, i64)> {
        songs
            .iter()
            .map(|s| (s.title.clone(), s.on_playlist(), s.pos))
            .collect()
    }

    let settle = |client: &mut petros::Client<harken::HarkenApp>,
                  view: &mut harken::LibraryView,
                  favorites: &mut harken::PlaylistCount,
                  rendered: &mut Vec<harken::Item>| {
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
                *rendered = harken::items_of(view);
            }
        }
        let read = harken::library(&mut client.store(), favs.clone()).unwrap();
        assert_eq!(shown(rendered), shown(&harken::items_of(view)), "spliced");
        assert_eq!(shown(rendered), shown(&read), "against a re-read");
        // The status line's number, against the list it used to count.
        assert_eq!(
            favorites.get(),
            read.iter().filter(|s| s.on_playlist()).count(),
            "the tally against a count of the rows"
        );
    };

    for title in ["Glue", "Apricots", "Opal"] {
        client
            .mutate(harken::add_song(
                title.into(),
                "Bicep".into(),
                String::new(),
                0,
                String::new(),
            ))
            .unwrap();
        settle(&mut client, &mut view, &mut favorites, &mut rendered);
    }

    let opal = harken::library(&mut client.store(), favs.clone()).unwrap()[2]
        .id
        .0
        .as_bytes()
        .to_vec();
    client
        .mutate(harken::add_to_playlist(favs.clone(), opal.clone()))
        .unwrap();
    settle(&mut client, &mut view, &mut favorites, &mut rendered);
    assert!(
        rendered[2].on_playlist(),
        "hearting reached the rendered list"
    );

    client
        .mutate(harken::remove_from_playlist(favs.clone(), opal))
        .unwrap();
    settle(&mut client, &mut view, &mut favorites, &mut rendered);
    assert!(!rendered[2].on_playlist());

    client
        .mutate(harken::add_all_to_playlist(favs.clone()))
        .unwrap();
    settle(&mut client, &mut view, &mut favorites, &mut rendered);
    assert!(rendered.iter().all(|s| s.on_playlist()));

    let glue = harken::library(&mut client.store(), favs.clone()).unwrap()[0]
        .id
        .0
        .as_bytes()
        .to_vec();
    client.mutate(harken::remove_media(glue)).unwrap();
    settle(&mut client, &mut view, &mut favorites, &mut rendered);
    assert_eq!(rendered.len(), 2);
}

/// The same, for changes that arrive from *another* peer.
///
/// The test above mutates locally, so every change reaches the view through
/// the optimistic path. A browser watching a phone sees the other one: entries
/// confirmed by the server and applied in `recv`, with nothing pending. That
/// is the path the web client showed wrong — heart everything on the phone
/// and one heart fills; remove the top song and a nameless row appears at the
/// bottom — and no test walked it.
#[test]
fn a_peers_changes_reach_the_maintained_library() {
    use petros::Changes;
    use petros_testkit::Sim;

    // Client 0 is the phone; client 1 is the browser, holding a view.
    let mut sim = Sim::<harken::HarkenApp>::new(3, 2);
    let favs = favourites(sim.client(0));
    sim.settle();
    let mut view = harken::library_view(&favs);
    let mut favorites = harken::playlist_count(&favs);
    {
        let mut store = sim.client(1).store();
        view.hydrate(&mut store);
        favorites.hydrate(&mut store);
    }
    let _ = sim.client(1).take_changes();
    let mut rendered = harken::items_of(&view);

    fn shown(songs: &[harken::Item]) -> Vec<(String, bool, Option<i64>)> {
        songs
            .iter()
            .map(|s| (s.title.clone(), s.on_playlist(), s.playlist_pos))
            .collect()
    }

    // What the browser does after the wire brings something: apply what
    // changed to the views and splice the list, then check all three agree.
    let settle = |sim: &mut Sim<harken::HarkenApp>,
                  view: &mut harken::LibraryView,
                  favorites: &mut harken::PlaylistCount,
                  rendered: &mut Vec<harken::Item>| {
        sim.settle();
        let client = sim.client(1);
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
                *rendered = harken::items_of(view);
            }
        }
        let read = harken::library(&mut client.store(), favs.clone()).unwrap();
        assert_eq!(shown(rendered), shown(&harken::items_of(view)), "spliced");
        assert_eq!(shown(rendered), shown(&read), "against a re-read");
        assert_eq!(
            favorites.get(),
            read.iter().filter(|s| s.on_playlist()).count(),
            "the tally against a count of the rows"
        );
    };

    for title in ["Glue", "Apricots"] {
        sim.mutate(
            0,
            harken::add_song(
                title.into(),
                "Bicep".into(),
                String::new(),
                0,
                String::new(),
            ),
        );
    }
    settle(&mut sim, &mut view, &mut favorites, &mut rendered);
    assert_eq!(rendered.len(), 2, "both songs arrived");

    // Heart everything on the phone: both hearts fill in the browser.
    sim.mutate(0, harken::add_all_to_playlist(favs.clone()));
    settle(&mut sim, &mut view, &mut favorites, &mut rendered);
    assert!(
        rendered.iter().all(|s| s.on_playlist()),
        "every heart filled: {:?}",
        shown(&rendered)
    );

    // Unheart the top one there: it empties here, and only it.
    let glue = rendered[0].id.0.as_bytes().to_vec();
    sim.mutate(0, harken::remove_from_playlist(favs.clone(), glue.clone()));
    settle(&mut sim, &mut view, &mut favorites, &mut rendered);
    assert!(!rendered[0].on_playlist(), "the top heart emptied");
    assert!(rendered[1].on_playlist(), "the bottom one did not");

    // Remove the top one there: it goes here, and nothing nameless appears.
    sim.mutate(0, harken::remove_media(glue));
    settle(&mut sim, &mut view, &mut favorites, &mut rendered);
    assert_eq!(
        rendered
            .iter()
            .map(|s| s.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Apricots"]
    );
}

/// A second peer's whole history, delivered in one batch, has to land in the
/// maintained view exactly once.
///
/// This is the browser's first sync: it opens an empty database, hydrates the
/// view against it, then the server hands over everything at once — every song
/// and every favourite together. A join sees each favourite twice there, once
/// because the song it hangs off is hydrated out of the store that already
/// holds it and once because the favourite's own entry is applied, and a
/// doubled favourite is what made the web client show hearts on the wrong
/// rows, keep a heart filled after an unheart, and grow a nameless row after a
/// remove. The fix is in `petros-ivm`; this is the scenario that exercised it.
#[test]
fn a_first_sync_delivers_songs_and_favourites_without_doubling() {
    use petros::Changes;
    use petros_testkit::Sim;

    // The phone builds a library and a playlist while the browser is cut off,
    // so none of it reaches the browser one entry at a time — it all lands in
    // one batch when the browser first connects.
    let mut sim = Sim::<harken::HarkenApp>::new(5, 2);
    let favs = favourites(sim.client(0));
    sim.partition(1);
    for (t, a) in [("Glue", "Bicep"), ("Opal", "Bicep"), ("Gosh", "Jamie xx")] {
        sim.mutate(
            0,
            harken::add_song(t.into(), a.into(), String::new(), 0, String::new()),
        );
        sim.step();
    }
    let songs: Vec<Vec<u8>> = harken::library(&mut sim.client(0).store(), favs.clone())
        .unwrap()
        .iter()
        .map(|s| s.id.0.as_bytes().to_vec())
        .collect();
    sim.mutate(0, harken::add_to_playlist(favs.clone(), songs[2].clone()));
    sim.mutate(0, harken::add_to_playlist(favs.clone(), songs[0].clone()));
    sim.step();

    // Now the browser opens: an empty database, a view hydrated against it,
    // and then the whole history arrives in one batch.
    let mut view = harken::library_view(&favs);
    let mut favorites = harken::playlist_count(&favs);
    {
        let mut store = sim.client(1).store();
        view.hydrate(&mut store);
        favorites.hydrate(&mut store);
    }
    let _ = sim.client(1).take_changes();
    let mut rendered = harken::items_of(&view);

    sim.settle();
    let client = sim.client(1);
    match client.take_changes() {
        Changes::Applied(changes) => {
            let patches = {
                let mut store = client.store();
                favorites.apply(&mut store, &changes);
                view.apply(&mut store, &changes)
            };
            harken::patch(&mut rendered, &patches);
        }
        Changes::Rebuilt => {
            {
                let mut store = client.store();
                view.hydrate(&mut store);
                favorites.hydrate(&mut store);
            }
            rendered = harken::items_of(&view);
        }
    }

    // The browser's own confirmed order is the truth for it — the network
    // reordered the phone's entries on the way, which is the rebase and not a
    // bug — so the test compares the spliced list to a re-read rather than to
    // any fixed order. What must hold is that the two agree, that the view
    // agrees with both, and that two songs are favourited and not, say, two
    // favourites doubled onto one.
    let shown = |songs: &[harken::Item]| -> Vec<(String, Option<i64>)> {
        songs
            .iter()
            .map(|s| (s.title.clone(), s.playlist_pos))
            .collect()
    };
    let read = harken::library(&mut sim.client(1).store(), favs.clone()).unwrap();
    assert_eq!(read.len(), 3);
    assert_eq!(shown(&rendered), shown(&read), "spliced vs a re-read");
    assert_eq!(
        shown(&rendered),
        shown(&harken::items_of(&view)),
        "vs the view"
    );
    assert_eq!(
        favorites.get(),
        2,
        "two favourites, counted once each — a doubled child would make this 4"
    );
    let mut positions: Vec<i64> = rendered.iter().filter_map(|s| s.playlist_pos).collect();
    positions.sort_unstable();
    assert_eq!(positions, vec![1, 2], "the two positions, no duplicate");

    // Unfavourite one the browser holds as hearted: its heart empties, rather
    // than a duplicate child surviving and keeping it filled. Which song that
    // is depends on the browser's own order, so it is read from there.
    let hearted = read
        .iter()
        .find(|s| s.on_playlist())
        .unwrap()
        .id
        .0
        .as_bytes()
        .to_vec();
    sim.mutate(
        0,
        harken::remove_from_playlist(favs.clone(), hearted.clone()),
    );
    sim.settle();
    let client = sim.client(1);
    match client.take_changes() {
        Changes::Applied(changes) => {
            let patches = {
                let mut store = client.store();
                favorites.apply(&mut store, &changes);
                view.apply(&mut store, &changes)
            };
            harken::patch(&mut rendered, &patches);
        }
        Changes::Rebuilt => {
            {
                let mut store = client.store();
                view.hydrate(&mut store);
                favorites.hydrate(&mut store);
            }
            rendered = harken::items_of(&view);
        }
    }
    let read = harken::library(&mut sim.client(1).store(), favs.clone()).unwrap();
    let un = read
        .iter()
        .position(|s| s.id.0.as_bytes() == hearted.as_slice())
        .unwrap();
    assert!(
        !rendered[un].on_playlist(),
        "the heart emptied on one unheart"
    );
    assert_eq!(shown(&rendered), shown(&read), "still matches a re-read");
    assert_eq!(favorites.get(), 1, "one favourite left");
}

/// A playlist to hang hearts on. There is no favourites table: a heart means
/// membership of whichever playlist a client shows, so every test makes one.
fn favourites<A: petros::App>(client: &mut petros::Client<A>) -> Vec<u8>
where
    A::Mutation: From<petros_schema::cbor::Value>,
{
    client
        .mutate(harken::create_playlist("Favourites".into()))
        .unwrap();
    harken::playlists(&mut client.store()).unwrap()[0]
        .id
        .0
        .as_bytes()
        .to_vec()
}
