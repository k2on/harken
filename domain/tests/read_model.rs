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
    let favs = favorites(&mut c);

    for (t, a) in [("Glue", "Bicep"), ("Opal", "Bicep"), ("Gosh", "Jamie xx")] {
        c.mutate(harken::add_song(
            t.into(),
            a.into(),
            String::new(),
            0,
            String::new(),
            0,
            String::new(),
            String::new(),
            String::new(),
            0,
            String::new(),
            String::new(),
        ))
        .unwrap();
    }
    let all = harken::library(&mut c.store(), favs).unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(
        all.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(),
        vec!["Glue", "Opal", "Gosh"],
        "library order is s.pos, and pos counts up from MAX(pos)"
    );
    assert_eq!(all[0].creator, "Bicep");
    assert_eq!(all[0].user_id, "alice");
    assert!(all.iter().all(|s| !s.on_playlist()), "nothing hearted yet");
    assert!(harken::playlist(&mut c.store(), favs).unwrap().is_empty());

    // Heart the third, then the first: the playlist is in the order they were
    // hearted, not the order they were added.
    let third = all[2].id;
    let first = all[0].id;
    c.mutate(harken::add_to_playlist(favs, third)).unwrap();
    c.mutate(harken::add_to_playlist(favs, first)).unwrap();

    let playlist = harken::playlist(&mut c.store(), favs).unwrap();
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
    let all = harken::library(&mut c.store(), favs).unwrap();
    assert_eq!(
        all.iter().map(|s| s.playlist_pos).collect::<Vec<_>>(),
        vec![Some(2), None, Some(1)]
    );
    assert_eq!(all[0].id, playlist[1].id, "ids survive the blob round trip");

    // Unhearting takes it off the playlist and leaves the song.
    c.mutate(harken::remove_from_playlist(favs, first)).unwrap();
    assert_eq!(harken::library(&mut c.store(), favs).unwrap().len(), 3);
    assert_eq!(harken::playlist(&mut c.store(), favs).unwrap().len(), 1);
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
    let favs = favorites(&mut client);

    let mut view = harken::library_view(favs);
    let mut favorites = harken::playlist_count(favs);
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
        let read = harken::library(&mut client.store(), favs).unwrap();
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
                0,
                String::new(),
                String::new(),
                String::new(),
                0,
                String::new(),
                String::new(),
            ))
            .unwrap();
        settle(&mut client, &mut view, &mut favorites, &mut rendered);
    }

    let opal = harken::library(&mut client.store(), favs).unwrap()[2].id;
    client.mutate(harken::add_to_playlist(favs, opal)).unwrap();
    settle(&mut client, &mut view, &mut favorites, &mut rendered);
    assert!(
        rendered[2].on_playlist(),
        "hearting reached the rendered list"
    );

    client
        .mutate(harken::remove_from_playlist(favs, opal))
        .unwrap();
    settle(&mut client, &mut view, &mut favorites, &mut rendered);
    assert!(!rendered[2].on_playlist());

    client.mutate(harken::add_all_to_playlist(favs)).unwrap();
    settle(&mut client, &mut view, &mut favorites, &mut rendered);
    assert!(rendered.iter().all(|s| s.on_playlist()));

    let glue = harken::library(&mut client.store(), favs).unwrap()[0].id;
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
    let favs = favorites(sim.client(0));
    sim.settle();
    let mut view = harken::library_view(favs);
    let mut favorites = harken::playlist_count(favs);
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
        let read = harken::library(&mut client.store(), favs).unwrap();
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
                0,
                String::new(),
                String::new(),
                String::new(),
                0,
                String::new(),
                String::new(),
            ),
        );
    }
    settle(&mut sim, &mut view, &mut favorites, &mut rendered);
    assert_eq!(rendered.len(), 2, "both songs arrived");

    // Heart everything on the phone: both hearts fill in the browser.
    sim.mutate(0, harken::add_all_to_playlist(favs));
    settle(&mut sim, &mut view, &mut favorites, &mut rendered);
    assert!(
        rendered.iter().all(|s| s.on_playlist()),
        "every heart filled: {:?}",
        shown(&rendered)
    );

    // Unheart the top one there: it empties here, and only it.
    let glue = rendered[0].id;
    sim.mutate(0, harken::remove_from_playlist(favs, glue));
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
/// and every favorite together. A join sees each favorite twice there, once
/// because the song it hangs off is hydrated out of the store that already
/// holds it and once because the favorite's own entry is applied, and a
/// doubled favorite is what made the web client show hearts on the wrong
/// rows, keep a heart filled after an unheart, and grow a nameless row after a
/// remove. The fix is in `petros-ivm`; this is the scenario that exercised it.
#[test]
fn a_first_sync_delivers_songs_and_favorites_without_doubling() {
    use petros::Changes;
    use petros_testkit::Sim;

    // The phone builds a library and a playlist while the browser is cut off,
    // so none of it reaches the browser one entry at a time — it all lands in
    // one batch when the browser first connects.
    let mut sim = Sim::<harken::HarkenApp>::new(5, 2);
    let favs = favorites(sim.client(0));
    sim.partition(1);
    for (t, a) in [("Glue", "Bicep"), ("Opal", "Bicep"), ("Gosh", "Jamie xx")] {
        sim.mutate(
            0,
            harken::add_song(
                t.into(),
                a.into(),
                String::new(),
                0,
                String::new(),
                0,
                String::new(),
                String::new(),
                String::new(),
                0,
                String::new(),
                String::new(),
            ),
        );
        sim.step();
    }
    let songs: Vec<harken::Id<harken::tables::Media>> =
        harken::library(&mut sim.client(0).store(), favs)
            .unwrap()
            .iter()
            .map(|s| s.id)
            .collect();
    sim.mutate(0, harken::add_to_playlist(favs, songs[2]));
    sim.mutate(0, harken::add_to_playlist(favs, songs[0]));
    sim.step();

    // Now the browser opens: an empty database, a view hydrated against it,
    // and then the whole history arrives in one batch.
    let mut view = harken::library_view(favs);
    let mut favorites = harken::playlist_count(favs);
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
    // agrees with both, and that two songs are favorited and not, say, two
    // favorites doubled onto one.
    let shown = |songs: &[harken::Item]| -> Vec<(String, Option<i64>)> {
        songs
            .iter()
            .map(|s| (s.title.clone(), s.playlist_pos))
            .collect()
    };
    let read = harken::library(&mut sim.client(1).store(), favs).unwrap();
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
        "two favorites, counted once each — a doubled child would make this 4"
    );
    let mut positions: Vec<i64> = rendered.iter().filter_map(|s| s.playlist_pos).collect();
    positions.sort_unstable();
    assert_eq!(positions, vec![1, 2], "the two positions, no duplicate");

    // Unfavorite one the browser holds as hearted: its heart empties, rather
    // than a duplicate child surviving and keeping it filled. Which song that
    // is depends on the browser's own order, so it is read from there.
    let hearted = read.iter().find(|s| s.on_playlist()).unwrap().id;
    sim.mutate(0, harken::remove_from_playlist(favs, hearted));
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
    let read = harken::library(&mut sim.client(1).store(), favs).unwrap();
    let un = read.iter().position(|s| s.id == hearted).unwrap();
    assert!(
        !rendered[un].on_playlist(),
        "the heart emptied on one unheart"
    );
    assert_eq!(shown(&rendered), shown(&read), "still matches a re-read");
    assert_eq!(favorites.get(), 1, "one favorite left");
}

/// A playlist to hang hearts on. There is no favorites table: a heart means
/// membership of whichever playlist a client shows, so every test makes one.
fn favorites<A: petros::App>(client: &mut petros::Client<A>) -> harken::Id<harken::tables::Playlist>
where
    A::Mutation: From<petros_schema::cbor::Value>,
{
    client
        .mutate(harken::create_playlist("Favorites".into()))
        .unwrap();
    harken::playlists(&mut client.store()).unwrap()[0].id
}

/// A person cannot end up with two playlists of one name, however many
/// devices they sign in on.
///
/// This is the bug the rule exists for, in miniature. Every client makes a
/// default playlist on its first run, and it has to do that *before* it has
/// seen the log — the list is empty because nothing has synced yet, not
/// because nobody has one. So a phone, a laptop and a browser tab each
/// authored a "Favorites" with a fresh id, and every one of them landed.
///
/// Authoring twice on one client is exactly what that reduces to, because it
/// is what replay does: the log merges and both entries run through one
/// `apply` against one database. That reduction is the whole argument for the
/// check being in `apply` rather than in the clients — no client could have
/// caught it, since each of them was right about what it could see.
#[test]
fn one_person_cannot_have_two_playlists_of_one_name() {
    let mut c = Client::<harken::HarkenApp>::open(
        petros::open_memory().unwrap(),
        "alice",
        AutoCtx::seeded(7),
    )
    .unwrap();

    c.mutate(harken::create_playlist("Favorites".into()))
        .unwrap();
    c.mutate(harken::create_playlist("Favorites".into()))
        .unwrap();
    // …and the shape a second client's would arrive in if somebody typed it.
    c.mutate(harken::create_playlist("  Favorites  ".into()))
        .unwrap();

    let names = |c: &mut petros::Client<harken::HarkenApp>| {
        harken::playlists(&mut c.store())
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect::<Vec<_>>()
    };
    assert_eq!(names(&mut c), ["Favorites"], "one, whoever asked twice");

    // It is the name that collides and not the verb: another name is another
    // playlist, and the numbering carries on past the one that was refused.
    c.mutate(harken::create_playlist("Gym".into())).unwrap();
    assert_eq!(names(&mut c), ["Favorites", "Gym"]);

    // Case is not folded, deliberately. A name is what somebody typed, and
    // deciding that "favorites" is the same word as "Favorites" is deciding
    // what they meant — which `add_song`'s file check does not do either.
    c.mutate(harken::create_playlist("favorites".into()))
        .unwrap();
    assert_eq!(names(&mut c), ["Favorites", "Gym", "favorites"]);
}

/// …and the rule is about a *person*, not about the library.
///
/// One log is one library and several people may be in it, so Bob's
/// "Favorites" is not Alice's. A check that looked only at the name would
/// leave whoever signed in second without the playlist their client just made
/// for them, which is a worse bug than the one it fixed and would only appear
/// on a server with two accounts on it.
#[test]
fn two_people_each_get_a_playlist_of_one_name() {
    use petros_testkit::Sim;

    let mut sim = Sim::<harken::HarkenApp>::new(11, 2);
    sim.mutate(0, harken::create_playlist("Favorites".into()));
    sim.mutate(1, harken::create_playlist("Favorites".into()));
    sim.settle();

    for i in 0..2 {
        let lists = harken::playlists(&mut sim.client(i).store()).unwrap();
        let mut whose: Vec<&str> = lists.iter().map(|p| p.user_id.as_str()).collect();
        whose.sort_unstable();
        assert_eq!(whose, ["c0", "c1"], "one each, and every peer sees both");
        assert!(lists.iter().all(|p| p.name == "Favorites"));
    }
}

/// The sidebar's three lists, and the lists they select.
///
/// The interesting one is albums: `album` lives on the `song` side table, not
/// on `media`, precisely so the library list stays kind-neutral — so this is
/// the query that has to reach across a relationship to answer at all.
#[test]
fn albums_and_artists_group_the_library_and_select_it_back() {
    let mut c = Client::<harken::HarkenApp>::open(
        petros::open_memory().unwrap(),
        "alice",
        AutoCtx::seeded(11),
    )
    .unwrap();
    let favs = favorites(&mut c);

    for (title, artist, album) in [
        ("Für Elise", "Beethoven", "Bagatelles"),
        ("Symphony No. 5 - I", "Beethoven", "Symphony No. 5"),
        ("Symphony No. 5 - III", "Beethoven", "Symphony No. 5"),
        ("Clair de lune", "Debussy", "Suite bergamasque"),
    ] {
        c.mutate(harken::add_song(
            title.into(),
            artist.into(),
            album.into(),
            0,
            String::new(),
            0,
            String::new(),
            String::new(),
            String::new(),
            0,
            String::new(),
            String::new(),
        ))
        .unwrap();
    }

    // Albums, by name, each counting its own tracks and naming who made it.
    let albums = harken::albums(&mut c.store()).unwrap();
    assert_eq!(
        albums
            .iter()
            .map(|a| (a.name.as_str(), a.creator.as_str(), a.tracks))
            .collect::<Vec<_>>(),
        vec![
            ("Bagatelles", "Beethoven", 1),
            ("Suite bergamasque", "Debussy", 1),
            ("Symphony No. 5", "Beethoven", 2),
        ],
        "one row per album, in name order, with the tracks counted"
    );

    let artists = harken::artists(&mut c.store()).unwrap();
    assert_eq!(
        artists
            .iter()
            .map(|a| (a.name.as_str(), a.tracks))
            .collect::<Vec<_>>(),
        vec![("Beethoven", 3), ("Debussy", 1)]
    );

    // Selecting one gives back the library's own rows, in library order.
    let symphony = harken::album(&mut c.store(), favs, "Symphony No. 5".into()).unwrap();
    assert_eq!(
        symphony
            .iter()
            .map(|i| i.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Symphony No. 5 - I", "Symphony No. 5 - III"]
    );
    let beethoven = harken::artist(&mut c.store(), favs, "Beethoven".into()).unwrap();
    assert_eq!(beethoven.len(), 3);
    assert!(harken::album(&mut c.store(), favs, "Nocturnes".into())
        .unwrap()
        .is_empty());

    // Read against a playlist, like the library list is: a track's heart shows
    // through the album view rather than being lost by it.
    let id = symphony[0].id;
    c.mutate(harken::add_to_playlist(favs, id)).unwrap();
    let symphony = harken::album(&mut c.store(), favs, "Symphony No. 5".into()).unwrap();
    assert!(
        symphony[0].on_playlist(),
        "the heart survives the album view"
    );
    assert!(!symphony[1].on_playlist());
}

/// An album comes back in the work's order, which is not the library's.
///
/// Every other list here is `media.pos` — the order things were added — and an
/// album is the one that is not, because a track number only means the
/// sequence it is drawn beside if the rows are in that sequence. Added
/// deliberately scrambled, and across two parts, so that library order and
/// work order cannot agree by accident.
#[test]
fn an_album_is_in_the_works_order_and_not_the_librarys() {
    let mut c = Client::<harken::HarkenApp>::open(
        petros::open_memory().unwrap(),
        "alice",
        AutoCtx::seeded(13),
    )
    .unwrap();
    let favs = favorites(&mut c);

    // Second suite before the first, and neither in track order.
    for (title, part, track) in [
        ("Gigue", "Suite No. 3 in G major", 21),
        ("Alla Hornpipe", "Suite No. 2 in D major", 12),
        ("Bourree", "Suite No. 3 in G major", 19),
        ("Overture", "Suite No. 2 in D major", 11),
    ] {
        c.mutate(harken::add_song(
            title.into(),
            "Handel".into(),
            "Water Music".into(),
            0,
            String::new(),
            track,
            part.into(),
            String::new(),
            String::new(),
            0,
            String::new(),
            String::new(),
        ))
        .unwrap();
    }
    // A movement nobody numbered, to show where 0 goes: after the numbered
    // ones in its part, not ahead of track 1.
    c.mutate(harken::add_song(
        "Air".into(),
        "Handel".into(),
        "Water Music".into(),
        0,
        String::new(),
        0,
        "Suite No. 2 in D major".into(),
        String::new(),
        String::new(),
        0,
        String::new(),
        String::new(),
    ))
    .unwrap();

    let water = harken::album(&mut c.store(), favs, "Water Music".into()).unwrap();
    assert_eq!(
        water.iter().map(|i| i.title.as_str()).collect::<Vec<_>>(),
        vec!["Overture", "Alla Hornpipe", "Air", "Bourree", "Gigue"],
        "part first, then track, and an untagged track last in its part"
    );

    // The library itself is untouched by any of that: it is still the order
    // the songs arrived in.
    let all = harken::library(&mut c.store(), favs).unwrap();
    assert_eq!(
        all.iter().map(|i| i.title.as_str()).collect::<Vec<_>>(),
        vec!["Gigue", "Alla Hornpipe", "Bourree", "Overture", "Air"]
    );
}

/// Which playlists a track is on, which is what makes the phone's sheet a
/// toggle rather than a one-way door.
#[test]
fn a_track_knows_which_playlists_it_is_on() {
    let mut c = Client::<harken::HarkenApp>::open(
        petros::open_memory().unwrap(),
        "alice",
        AutoCtx::seeded(17),
    )
    .unwrap();
    let favs = favorites(&mut c);
    c.mutate(harken::create_playlist("Evening".into())).unwrap();
    let evening = harken::playlists(&mut c.store()).unwrap()[1].id;

    for t in ["Air", "Gigue"] {
        c.mutate(harken::add_song(
            t.into(),
            "Handel".into(),
            String::new(),
            0,
            String::new(),
            0,
            String::new(),
            String::new(),
            String::new(),
            0,
            String::new(),
            String::new(),
        ))
        .unwrap();
    }
    let all = harken::library(&mut c.store(), favs).unwrap();
    let (air, gigue) = (all[0].id, all[1].id);

    let names = |c: &mut Client<harken::HarkenApp>, id| {
        harken::playlists_of(&mut c.store(), id)
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect::<Vec<_>>()
    };
    assert!(names(&mut c, air).is_empty(), "on nothing to begin with");

    c.mutate(harken::add_to_playlist(evening, air)).unwrap();
    c.mutate(harken::add_to_playlist(favs, air)).unwrap();
    assert_eq!(
        names(&mut c, air),
        vec!["Favorites", "Evening"],
        "in the order the playlists were made, not the order it joined them"
    );
    assert!(names(&mut c, gigue).is_empty(), "and only this track's");

    c.mutate(harken::remove_from_playlist(favs, air)).unwrap();
    assert_eq!(names(&mut c, air), vec!["Evening"], "taking it off shows");
}

/// A rescan is a no-op, and that is decided in `apply` rather than by whatever
/// is scanning.
///
/// The id is chosen fresh at the originating client, so a second scan of the
/// same directory authors a *different* id for a file the library already has:
/// the id check cannot see it. Every peer replaying the log has to reach the
/// same answer, which is why the file check lives beside it.
#[test]
fn the_same_file_twice_is_one_song() {
    let mut c = Client::<harken::HarkenApp>::open(
        petros::open_memory().unwrap(),
        "library",
        AutoCtx::seeded(5),
    )
    .unwrap();
    let favs = favorites(&mut c);

    let add = |title: &str, file: &str| {
        harken::add_song(
            title.into(),
            "Beethoven".into(),
            "Bagatelles".into(),
            0,
            file.into(),
            0,
            String::new(),
            String::new(),
            String::new(),
            0,
            String::new(),
            String::new(),
        )
    };
    c.mutate(add("Für Elise", "beethoven/fur-elise.mp3"))
        .unwrap();
    // The same path again — a rescan, or a watcher that fired twice.
    c.mutate(add("Für Elise", "beethoven/fur-elise.mp3"))
        .unwrap();
    // …even under a different title, because the path is what identifies it.
    c.mutate(add("Fur Elise (again)", "beethoven/fur-elise.mp3"))
        .unwrap();
    assert_eq!(
        harken::library(&mut c.store(), favs).unwrap().len(),
        1,
        "one file is one song, however many times it is offered"
    );

    // A different path is a different song, even with the same title.
    c.mutate(add("Für Elise", "beethoven/fur-elise-live.mp3"))
        .unwrap();
    assert_eq!(harken::library(&mut c.store(), favs).unwrap().len(), 2);

    // And an empty file is not a path: two hand-typed songs are two songs,
    // which is what the whole non-empty guard is for.
    c.mutate(add("Untitled", "")).unwrap();
    c.mutate(add("Untitled", "")).unwrap();
    assert_eq!(
        harken::library(&mut c.store(), favs).unwrap().len(),
        4,
        "no file means no collision"
    );
}

/// A cover reaches `albums()` and `artists()`, carried by the song that names
/// them — and the rules for what a second entry does to one.
///
/// Two things this is holding down. The first is that the verb is *dispatched*
/// at all: `#[mutation]` writes the authoring function and the schema section,
/// so a verb left out of `peer!` compiles, type-checks at every call site and
/// appears in `mutations.txt`, and is then refused at apply time as an unknown
/// mutation. `set_artwork` shipped that way for its whole life, and what it
/// looked like was covers that never loaded — which is why every `mutate` here
/// is unwrapped rather than discarded.
///
/// The second is `art_to_write`, which is the one last-write-wins rule in the
/// domain and has three cases that a reading cannot tell apart.
#[test]
fn artwork_reaches_the_lists_it_is_drawn_on() {
    let mut c = Client::<harken::HarkenApp>::open(
        petros::open_memory().unwrap(),
        "alice",
        AutoCtx::seeded(7),
    )
    .unwrap();

    // One track, carrying a cover for its record and one for its composer.
    let handel = |file: &str, album_art: &str, artist_art: &str| {
        harken::add_song(
            "Alla Hornpipe".into(),
            "George Frideric Handel".into(),
            "Water Music".into(),
            0,
            file.into(),
            1,
            String::new(),
            String::new(),
            String::new(),
            0,
            album_art.into(),
            artist_art.into(),
        )
    };

    c.mutate(handel(
        "music/a.mp3",
        "https://example.com/thames.jpg",
        "https://example.com/denner.jpg",
    ))
    .expect("add_song has to be a verb this build can apply");

    let albums = harken::albums(&mut c.store()).unwrap();
    assert_eq!(albums.len(), 1);
    assert_eq!(albums[0].art, "https://example.com/thames.jpg");

    let artists = harken::artists(&mut c.store()).unwrap();
    assert_eq!(artists.len(), 1);
    assert_eq!(artists[0].art, "https://example.com/denner.jpg");

    // A second track of the same record with *no* picture leaves the one that
    // is there. This is the case the scanner is in on every rescan, and
    // getting it wrong wipes a library's covers one track at a time.
    c.mutate(handel("music/b.mp3", "", "")).unwrap();
    assert_eq!(
        harken::albums(&mut c.store()).unwrap()[0].art,
        "https://example.com/thames.jpg",
        "an entry with no picture must not clear one"
    );

    // A picture replaces a picture, which is the opposite of what `add_song`
    // does with a file — because somebody who picks a better cover means the
    // newer one, and the log being ordered is what makes "newer" a fact.
    c.mutate(handel("music/c.mp3", "https://example.com/better.jpg", ""))
        .unwrap();
    assert_eq!(
        harken::albums(&mut c.store()).unwrap()[0].art,
        "https://example.com/better.jpg"
    );

    // And a record with no cover at all is still a record: the row exists
    // because a song pointed at it, and `albums()` answers with the empty
    // string a client draws its derived square for.
    c.mutate(harken::add_song(
        "Clair de lune".into(),
        "Claude Debussy".into(),
        "Suite bergamasque".into(),
        0,
        "music/d.mp3".into(),
        3,
        String::new(),
        String::new(),
        String::new(),
        0,
        String::new(),
        String::new(),
    ))
    .unwrap();
    let albums = harken::albums(&mut c.store()).unwrap();
    assert_eq!(albums.len(), 2);
    let bergamasque = albums.iter().find(|a| a.name == "Suite bergamasque");
    assert_eq!(bergamasque.map(|a| a.art.as_str()), Some(""));
}
