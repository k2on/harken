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
    album    TEXT NOT NULL
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
