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

-- A record: a thing, rather than a string somebody typed on a track.
--
-- It was the second of those until now, with a separate `artwork` table keyed
-- by `(subject, name)` to hang a picture on — because a cover has to belong to
-- something, and there was nothing for it to belong to. This is that something.
-- `song.album_name` points at a row here, and the cover is a *column* on it
-- rather than a row in a table of covers about things that are not rows.
--
-- **Keyed by the name and not by an id**, which is the decision everything else
-- follows from. Two reasons, and the first is mechanical: `fill_auto` hands a
-- mutation exactly one uuid, so no single verb can mint an album and a song in
-- the same entry, and splitting it into two verbs means a client authoring a
-- reference to a row it has not seen confirmed. The second is the rebase. Two
-- peers each adding "Water Music" offline would author two rows with two ids,
-- one of which loses — and every song pointing at the loser is left pointing at
-- nothing. A name is the thing both peers already agree on without being told,
-- which is what makes `add_song` able to write this row itself.
--
-- So there is no `create_album`. A row appears because a song named it.
CREATE TABLE IF NOT EXISTS album (
    name     TEXT PRIMARY KEY NOT NULL,
    -- Who published it, and when. Facts about the *release* — which is what an
    -- album is, and what makes it a different thing from the `recording` below.
    -- Empty and 0 are "nobody said".
    label    TEXT NOT NULL,
    released BIGINT NOT NULL,
    -- Where the cover is, spelt exactly as `media.file` is: a path relative to
    -- the media root, or a whole URL. The bytes are not the log's business, and
    -- a client joins the path to its own server — the rule that already exists,
    -- rather than a second one for pictures.
    --
    -- Empty is "nobody said", which is the normal case: a client draws the
    -- square it derives from the name, and that is an answer rather than a
    -- placeholder.
    art      TEXT NOT NULL,
    added_ms BIGINT NOT NULL,
    user_id  TEXT NOT NULL
);

-- Anybody who made something: a composer, a conductor, an orchestra, a
-- soloist, a band, the host of a podcast. It was `artist`, which was the wrong
-- name for two of those.
--
-- **One table and not two**, though Apple Music Classical browses Composers
-- separately from Artists — because Bernstein is both and a person is not a
-- role. Which role somebody played is on `credit`, per recording, where it
-- actually varies.
--
-- **Nothing references this from `media`**, and that is deliberate.
-- `media.creator` is the one kind-neutral name for whoever made a thing — a
-- song's artist, a classical work's composer, a sermon's speaker, a podcast's
-- show — and a foreign key from it into a table of people would be naming some
-- of those wrongly. So `creator` stays the *line a list draws*, this is what is
-- known about somebody, and they are joined by name where there is a row.
CREATE TABLE IF NOT EXISTS person (
    name      TEXT PRIMARY KEY NOT NULL,
    -- "Bach, Johann Sebastian". What an index is ordered by, which is not what
    -- a page is titled. Empty means nobody said and a client falls back to
    -- `name`, rather than this guessing at where a surname starts — "Academy of
    -- St Martin in the Fields" has none and "van Beethoven" is not "van".
    sort_name TEXT NOT NULL,
    -- Years, 0 unknown. What makes a composer index readable at a glance and
    -- what a period is checked against.
    born      BIGINT NOT NULL,
    died      BIGINT NOT NULL,
    art       TEXT NOT NULL,
    added_ms  BIGINT NOT NULL,
    user_id   TEXT NOT NULL
);

-- The composition: the thing somebody wrote, as distinct from any performance
-- of it and from any record it was released on.
--
-- This is the entity classical needs and pop hides. The industry has always had
-- both — an ISWC identifies a work and an ISRC identifies a recording — but pop
-- works have one movement and nobody quotes their catalogue numbers, so the two
-- collapse into "a track" and nothing is lost. In classical they do not: one
-- work has dozens of recordings, and a page that cannot say so is a page that
-- lists the Goldberg Variations twice and calls them two albums.
--
-- **Optional for a track**, because most music is not of a named composition.
-- `song.movement_id` is NULL and `recording.work_id` is NULL for a pop single,
-- which is the true statement rather than a hole.
CREATE TABLE IF NOT EXISTS work (
    -- Derived from the composer and the catalogue number, and **never shown**.
    -- See `work_key` in `functions.rs` for the rule and why it is a key rather
    -- than an id: `fill_auto` hands a mutation one uuid, so no verb can mint a
    -- work and a song in the same entry, and two peers minting "BWV 988"
    -- offline would author two rows with one of them losing the rebase. A
    -- catalogue number is the one name every peer already agrees on without
    -- being told, which is the entire reason catalogue numbers exist.
    id        TEXT PRIMARY KEY NOT NULL,
    composer  TEXT NOT NULL REFERENCES person(name),
    -- "Goldberg Variations". The nickname where there is one, because that is
    -- what anybody types; `catalogue` is what disambiguates.
    title     TEXT NOT NULL,
    -- `BWV 988`, `K. 525`, `Op. 23`. The only *stable* name a classical work
    -- has: titles are translated, transliterated and abbreviated differently by
    -- every publisher, and the number is the same in every language and every
    -- edition. It was a column on `song`, repeated once per movement.
    catalogue TEXT NOT NULL,
    -- `Op. 67`, when the catalogue is something else. Empty otherwise.
    opus      TEXT NOT NULL,
    -- "C Minor", in Apple's English forms. Not `key`, which is a SQL word.
    key_sig   TEXT NOT NULL,
    -- "Symphony", "Concerto", "Opera", "String Quartet". A Browse axis in every
    -- classical service, and the one classical calls a genre.
    form      TEXT NOT NULL,
    -- "Baroque", "Classical", "Romantic". The other Browse axis, and a fact
    -- about the work rather than about its composer: Beethoven wrote in two.
    period    TEXT NOT NULL,
    composed  BIGINT NOT NULL,
    art       TEXT NOT NULL,
    added_ms  BIGINT NOT NULL,
    user_id   TEXT NOT NULL
);

-- The work's own ordered parts, independent of anybody's recording of them.
--
-- This is what makes two recordings of the Goldbergs line up variation by
-- variation, which is the whole reason it is a table rather than a number on
-- the track: a movement is a fact about the composition, and a track number is
-- a fact about a release.
CREATE TABLE IF NOT EXISTS movement (
    -- The work's key and the movement number, joined. Derived, never shown.
    id       TEXT PRIMARY KEY NOT NULL,
    work_id  TEXT NOT NULL REFERENCES work(id),
    -- Where it sits in the work. 1-based; two movements of one work never
    -- share one.
    no       BIGINT NOT NULL,
    -- "Variatio 12 Canone alla Quarta", "I. Allegro con brio".
    title    TEXT NOT NULL,
    -- The division of the work this belongs to, when a work has them: a suite
    -- inside Water Music, a book of the Well-Tempered Clavier, an act of an
    -- opera. Empty when the work is undivided, which is most of them. It was a
    -- column on `song`; it is a fact about the composition and belongs here.
    part     TEXT NOT NULL,
    added_ms BIGINT NOT NULL,
    user_id  TEXT NOT NULL
);

-- One performance, captured. The entity this whole shape exists for.
--
-- **Every track has one**, pop included, and that is deliberate: a recording is
-- "this audio, as a performance", which every track is — only some are of a
-- work somebody catalogued. Making it optional instead would mean credits hang
-- off recordings for classical and off nothing for pop, so "who played this"
-- would be two different questions depending on the genre, and one browse page
-- would mean two things. One extra row per pop track is the price and it is
-- nothing.
--
-- It is *not* the album. An album is a release and may carry several
-- recordings — a Brandenburg set where Nos. 1 and 4 come from different
-- sessions is one album and two recordings — and one recording may appear on
-- several releases. Collapsing them is exactly what makes a classical library
-- unbrowsable.
CREATE TABLE IF NOT EXISTS recording (
    -- What it is a recording *of*, and by whom. Derived; see `recording_key`.
    id       TEXT PRIMARY KEY NOT NULL,
    -- NULL when this is not a performance of a named composition, which is most
    -- music. See `work` above.
    work_id  TEXT REFERENCES work(id),
    -- The year it was made, which is not the year the album came out — a 1958
    -- session reissued in 2011 is both, and only one of them says anything
    -- about how it sounds. 0 unknown.
    recorded BIGINT NOT NULL,
    venue    TEXT NOT NULL,
    label    TEXT NOT NULL,
    -- The terms the recording was offered on, where they are not "none
    -- reserved": the demo's CC BY credits live here. It was in `song.performer`
    -- because there was nowhere else, and a licence is a fact about a
    -- performance rather than about a piece of music or a person.
    licence  TEXT NOT NULL,
    art      TEXT NOT NULL,
    added_ms BIGINT NOT NULL,
    user_id  TEXT NOT NULL
);

-- Who played it, and as what.
--
-- Apple marks orchestra, conductor, soloist, ensemble and choir all as Primary:
-- what differs between them is the role, not the rank. `role` is the Browse
-- axis ("conductors", "orchestras", "choirs") and `instrument` is the other
-- one, so a soloist can be found by what they play.
--
-- **On the recording and not on the track**, because a conductor is a fact
-- about a performance. Per track, two movements of one recording could disagree
-- about their own conductor — which is the same argument that keeps a cover off
-- `song`, one level up.
--
-- A pop track's credits are here too, with `role` of `artist`. That is what
-- turns "Kendrick Lamar feat. Pharrell" from a string a list draws into two
-- people a library can be browsed by.
CREATE TABLE IF NOT EXISTS credit (
    recording_id TEXT NOT NULL REFERENCES recording(id),
    person_name  TEXT NOT NULL REFERENCES person(name),
    -- `artist`, `composer`, `conductor`, `orchestra`, `ensemble`, `choir`,
    -- `soloist`. Text and not an enum, for the reason `media.kind` is: it is a
    -- column in a permanent log, and a peer that has never heard of a role
    -- still carries the row and lists it.
    role         TEXT NOT NULL,
    -- "Piano", "Violin". Empty unless the role is `soloist`.
    instrument   TEXT NOT NULL,
    -- Billing order, which is information: the first name on a classical record
    -- is not an alphabetical accident.
    pos          BIGINT NOT NULL,
    added_ms     BIGINT NOT NULL,
    user_id      TEXT NOT NULL,
    -- One person can hold two roles on one recording — Bernstein conducting and
    -- playing — so the role is part of the key.
    PRIMARY KEY (recording_id, person_name, role)
);

-- What is true of a song and of nothing else: where it sits on a release, and
-- which movement of which performance it is.
--
-- Three columns left here for the tables above. `catalogue` is a fact about the
-- work and was repeated once per movement; `part` is a fact about the work too;
-- `performer` was one lumped string standing in for a list of people with
-- roles. None of them were facts about a track.
CREATE TABLE IF NOT EXISTS song (
    media_id     BLOB PRIMARY KEY NOT NULL REFERENCES media(id),
    -- The record this is on, or NULL for a song that is not on one.
    --
    -- `_name` because the row it names is `album`, and `tables!` names a
    -- relationship after the table it points at — so a column called `album`
    -- and the `Song::album` it generates would be the same name twice, which
    -- the macro refuses by name. Same for the two `_id`s below.
    --
    -- **Nullable, and the foreign key is why.** Petros enforces these, so a
    -- song not on any album cannot carry `''` here unless there is an album
    -- called `''` for it to point at — and inventing one would put a nameless
    -- card on the albums page for every single somebody typed in. NULL is what
    -- "no record" actually is, and `albums()` skips it without a special case.
    album_name   TEXT REFERENCES album(name),
    -- Which disc of the release. 1 for almost everything; a boxed set is the
    -- case that needs it, and classical is made of boxed sets.
    disc         BIGINT NOT NULL,
    -- Where it sits **on the release**, which is not where it sits in the work.
    -- A compilation puts the Moonlight's first movement at track 9; the work
    -- still says it is movement 1. `movement.no` is that other number, and
    -- conflating the two is what makes an album page and a work page disagree.
    --
    -- 0 is "not known", which is honest for a single somebody typed in and for
    -- a file with no tag — and sorts first, which is where an unplaced track
    -- belongs.
    track        BIGINT NOT NULL,
    -- The performance this is part of. **Not nullable**: every track is a
    -- recording of something, and that is what lets `credit` be the one answer
    -- to "who played this" for every genre. See `recording`.
    recording_id TEXT NOT NULL REFERENCES recording(id),
    -- Which movement of the work, or NULL when the recording is not of a named
    -- work — which is most music. A pop single is a song with no movement, and
    -- that is a true statement rather than a gap.
    movement_id  TEXT REFERENCES movement(id),
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
    bpm          BIGINT NOT NULL
);

-- A playlist. "Favorites" is one of these and nothing special: a heart in a
-- client means "this is in the favorites playlist", not a flag on the row.
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
