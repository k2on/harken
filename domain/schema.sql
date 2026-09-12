-- The app's schema, and the single source of truth for it.
--
-- `App::migrate` runs this, and `petros-sql` prepares every statement in the
-- domain against it at build time — so a column renamed here is a compile error
-- at the call sites that use it, rather than a missing row on a device.
CREATE TABLE IF NOT EXISTS song (
    id       BLOB PRIMARY KEY NOT NULL,
    title    TEXT NOT NULL,
    artist   TEXT NOT NULL,
    pos      BIGINT NOT NULL,
    added_ms BIGINT NOT NULL,
    actor    TEXT NOT NULL
);

-- The favourites playlist is a playlist, not a flag: it has an order, and
-- "add to favourites" reads the end of it. That is what makes the rebase
-- visible — a favourite made offline lands after whatever arrived meanwhile.
-- The REFERENCES is not decoration. `tables!` reads it back with
-- `PRAGMA foreign_key_list` and generates both directions of the relationship
-- from it — `Song::favorite` and `Favorite::song` — so how the two tables meet
-- is written once, here, and never in a query.
CREATE TABLE IF NOT EXISTS favorite (
    song_id      BLOB PRIMARY KEY NOT NULL REFERENCES song(id),
    pos          BIGINT NOT NULL,
    favorited_ms BIGINT NOT NULL,
    actor        TEXT NOT NULL
);
