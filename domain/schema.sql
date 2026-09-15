-- The app's schema, and the single source of truth for it.
--
-- `App::migrate` runs this, and `petros-sql` prepares every statement in the
-- domain against it at build time — so a column renamed here is a compile error
-- at the call sites that use it, rather than a missing row on a device.

-- Everything playable, whatever kind it is. A song, a podcast episode and a
-- sermon differ in what is *said about* them, not in what it takes to put one
-- in a list and play it: a title, whoever made it, how long it runs, and the
-- file it lives in. Those are here, once, so the library screen is one query
-- over one table and does not grow a join per kind.
--
-- `kind` is the discriminator and the name of the side table that carries the
-- rest. A client can render a mixed list from this row alone.
CREATE TABLE IF NOT EXISTS media (
    id          BLOB PRIMARY KEY NOT NULL,
    -- 'song' today; 'episode' and 'sermon' are new rows, not new columns.
    kind        TEXT NOT NULL,
    title       TEXT NOT NULL,
    -- Whoever made it: the artist of a song, the speaker of a sermon, the show
    -- a podcast episode belongs to. Kind-neutral on purpose — the precise fact
    -- lives on the side table, and this is what a list renders.
    creator     TEXT NOT NULL,
    duration_ms BIGINT NOT NULL,
    -- The file in the media store, which is not the log's business: the bytes
    -- travel over HTTP and only the name of them is synced.
    file        TEXT NOT NULL,
    pos         BIGINT NOT NULL,
    added_ms    BIGINT NOT NULL,
    user_id     TEXT NOT NULL
);

-- What is true of a song and of nothing else. A second kind is a table like
-- this one and a verb beside `add_song`; no row here moves and no column above
-- changes, which is what makes the shape worth having before there is a second
-- kind to prove it.
CREATE TABLE IF NOT EXISTS song (
    media_id BLOB PRIMARY KEY NOT NULL REFERENCES media(id),
    album    TEXT NOT NULL,
    -- Where it sits in the album. 0 is "not known", which is honest for a
    -- single somebody typed in and for a file with no tag — and sorts first,
    -- which is where an unplaced track belongs.
    track    BIGINT NOT NULL,
    -- The division of the work this belongs to, when a work has them: a suite
    -- inside Water Music, a book of the Well-Tempered Clavier, an act. Empty
    -- when the album is undivided, which is most of them.
    --
    -- Not folded into `album`, because they answer different questions: the
    -- album is what you put on, and this is where you are inside it. A client
    -- that groups by album still groups correctly when a work has three
    -- suites, and one that shows the division can.
    part     TEXT NOT NULL,
    -- The catalogue number: `BWV 988`, `K. 525`, `Op. 23`. Empty for music
    -- nobody catalogued.
    --
    -- This is the only *stable* name a classical work has. Titles are
    -- translated, transliterated and abbreviated differently by every
    -- publisher; the number is the same in every language and every edition,
    -- which makes it the thing to match on when two recordings are the same
    -- piece.
    catalogue TEXT NOT NULL,
    -- Who played it, which is not who wrote it. `media.creator` carries the
    -- composer, because that is the line a kind-neutral list draws under a
    -- title — and for three hundred years of music the performer is a
    -- different person, sometimes several of them on one album.
    --
    -- Empty when unknown rather than repeating the composer: saying nothing is
    -- better than saying something false, and a client can tell them apart.
    performer TEXT NOT NULL,
    -- Beats per minute, 0 when unknown.
    --
    -- Two honest sources, and neither is a machine listening to the audio. A
    -- tagged file states it in `TBPM` and the scanner reads it. Failing that,
    -- the *tempo marking* is one: `Allegro` and `Adagio` are instructions
    -- about speed, written by the composer, and the conventional metronome
    -- ranges for them are what a performer reads. Taking the middle of the
    -- range for the marking is an approximation and is stored as one.
    --
    -- What is not allowed here is a number with no source. 0 means nobody
    -- said, and a movement titled `Variatio 7 a 1 ovvero 2 Clav` is a
    -- movement nobody said a tempo for.
    bpm      BIGINT NOT NULL
);

-- A playlist. "Favourites" is one of these and nothing special: a heart in a
-- client means "this is in the favourites playlist", not a flag on the row.
-- That is why there is no `favorite` table — there was one, and it was a
-- playlist wearing a different name, which meant a second playlist could never
-- reuse any of it.
CREATE TABLE IF NOT EXISTS playlist (
    id         BLOB PRIMARY KEY NOT NULL,
    name       TEXT NOT NULL,
    pos        BIGINT NOT NULL,
    created_ms BIGINT NOT NULL,
    user_id    TEXT NOT NULL
);

-- What is on a playlist, and in what order. `pos` is read from the end of the
-- playlist when something is added, which is what makes the rebase visible: an
-- addition made offline lands after whatever arrived meanwhile.
--
-- Keyed by the pair, because a playlist holds an item once — it is a set with
-- an order. The two REFERENCES generate `Playlist::playlist_item` and
-- `Media::playlist_item`, so both directions of both relationships come from
-- the DDL and never from a query.
CREATE TABLE IF NOT EXISTS playlist_item (
    playlist_id BLOB NOT NULL REFERENCES playlist(id),
    media_id    BLOB NOT NULL REFERENCES media(id),
    pos         BIGINT NOT NULL,
    added_ms    BIGINT NOT NULL,
    user_id     TEXT NOT NULL,
    PRIMARY KEY (playlist_id, media_id)
);
