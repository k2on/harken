//! The music domain: `apply`, `fill_auto`, and nothing that knows where it runs.
//!
//! A library of songs, and a favourites playlist that is a real ordered
//! playlist rather than a flag — so "add this to favourites" reads the end of
//! the list and puts the song after it, which is an intent and not a fact. That
//! is what makes the rebase visible: favourite a song while offline, come back
//! to find someone else favourited two, and yours lands after theirs.
//!
//! The verbs are declared and dispatched by one macro, so an argument's name is
//! written once. Everything `apply` can reach is [`Host`]: three methods, no
//! clock, no randomness, no network, no filesystem. On wasm the sandbox
//! enforces that, because the module imports nothing else; natively the trait
//! is the only argument `apply` gets.
//!
//! This is compiled twice — linked into the server and the iced peer, and to
//! wasm for the phone, which loads it as a file it can replace without a
//! rebuild. `tests/conformance.rs` drives the same mutations through both and
//! compares the rows, because "two builds of one source" is a claim.
//!
//! The SQL is real SQL, and it is checked: `petros_sql::exec!` and
//! `petros_sql::query!` prepare each statement against `schema.sql` at build
//! time with SQLite as the judge. So `FavoriteAll` is one statement rather than
//! a scan and a write per song — which on a phone is a boundary crossing per
//! song — and a misspelled column is a compile error even though this code runs
//! inside a sandbox with no SQLite in it.

use petros_schema::cbor::{set, Value};

petros_schema::mutations! {
    /// Put a song in the library.
    AddSong { title: Text, artist: Text } auto { id: Id, added_ms: Integer } => |host, actor| {
        if title.trim().is_empty() {
            return Err("a song needs a title".into());
        }
        // The same entry arriving twice is a no-op, which is what makes
        // redelivery safe.
        let already = petros_sql::query_one!(
            host, "SELECT 1 AS \"found: Int\" FROM song WHERE id = ?", id);
        if already.is_some() {
            return Ok(());
        }
        // `pos` is read out of current state: "put it at the end", an intent,
        // not "put it at 3", a fact.
        let last = petros_sql::query_one!(
            host, "SELECT COALESCE(MAX(pos), 0) AS \"last: Int\" FROM song")
            .map(|r| r.last).unwrap_or(0);
        let (title, artist) = (title.trim().to_string(), artist.trim().to_string());
        petros_sql::exec!(
            host,
            "INSERT INTO song (id, title, artist, pos, added_ms, actor)
             VALUES (?, ?, ?, ?, ?, ?)",
            id, title, artist, last + 1, added_ms, actor
        );
        Ok(())
    }

    /// A whole album at once: five tracks from one seed.
    ///
    /// The ids are already in the payload — `fill_auto` put them there at the
    /// originating client — so this is as deterministic as any other apply.
    AddAlbum {} auto { tracks: Array, added_ms: Integer } => |host, actor| {
        // Read the end of the library once, then count up. Re-reading between
        // inserts would give the same answer and cost five more round trips.
        let mut pos = petros_sql::query_one!(
            host, "SELECT COALESCE(MAX(pos), 0) AS \"last: Int\" FROM song")
            .map(|r| r.last).unwrap_or(0);
        for track in tracks {
            let Some(id) = petros_schema::cbor::field(track, "id")
                .and_then(petros_schema::cbor::as_bytes)
            else {
                return Err("a track has no id".into());
            };
            let title = petros_schema::cbor::opt_text(track, "title");
            let artist = petros_schema::cbor::opt_text(track, "artist");
            if title.trim().is_empty() {
                continue;
            }
            let already = petros_sql::query_one!(
                host, "SELECT 1 AS \"found: Int\" FROM song WHERE id = ?", id);
            if already.is_some() {
                continue;
            }
            pos += 1;
            let (title, artist) = (title.trim().to_string(), artist.trim().to_string());
            petros_sql::exec!(
                host,
                "INSERT INTO song (id, title, artist, pos, added_ms, actor)
                 VALUES (?, ?, ?, ?, ?, ?)",
                id, title, artist, pos, added_ms, actor
            );
        }
        Ok(())
    }

    /// Add a song to the favourites playlist, at the end.
    ///
    /// Favouriting a song that is gone is a no-op rather than an error: an entry
    /// earlier in the log may have removed it. So is favouriting one already
    /// there — the playlist is a set with an order, and a song keeps the place
    /// it first got.
    Favorite { id: Id } auto { favorited_ms: Integer } => |host, actor| {
        let song = petros_sql::query_one!(
            host, "SELECT 1 AS \"found: Int\" FROM song WHERE id = ?", id);
        if song.is_none() {
            return Ok(());
        }
        let already = petros_sql::query_one!(
            host, "SELECT 1 AS \"found: Int\" FROM favorite WHERE song_id = ?", id);
        if already.is_some() {
            return Ok(());
        }
        let last = petros_sql::query_one!(
            host, "SELECT COALESCE(MAX(pos), 0) AS \"last: Int\" FROM favorite")
            .map(|r| r.last).unwrap_or(0);
        petros_sql::exec!(
            host,
            "INSERT INTO favorite (song_id, pos, favorited_ms, actor) VALUES (?, ?, ?, ?)",
            id, last + 1, favorited_ms, actor
        );
        Ok(())
    }

    /// Take a song back out of the playlist. The song itself stays.
    Unfavorite { id: Id } => |host, actor| {
        let _ = actor;
        petros_sql::exec!(host, "DELETE FROM favorite WHERE song_id = ?", id);
        Ok(())
    }

    /// Favourite everything in the library that is not already favourited.
    ///
    /// One entry rather than one per song, so it covers songs another peer added
    /// in the meantime — that is what makes it an intent. And one *statement*,
    /// which is what checked SQL buys: a typed query builder would make this a
    /// scan and a write per song, and each of those is a boundary crossing on a
    /// phone.
    FavoriteAll {} auto { favorited_ms: Integer } => |host, actor| {
        petros_sql::exec!(
            host,
            "INSERT INTO favorite (song_id, pos, favorited_ms, actor)
             SELECT s.id,
                    (SELECT COALESCE(MAX(pos), 0) FROM favorite)
                      + ROW_NUMBER() OVER (ORDER BY s.pos, s.id),
                    ?, ?
               FROM song s
              WHERE NOT EXISTS (SELECT 1 FROM favorite f WHERE f.song_id = s.id)
              ORDER BY s.pos, s.id",
            favorited_ms, actor
        );
        Ok(())
    }

    /// Remove a song from the library, and from the playlist with it.
    RemoveSong { id: Id } => |host, actor| {
        let _ = actor;
        petros_sql::exec!(host, "DELETE FROM favorite WHERE song_id = ?", id);
        petros_sql::exec!(host, "DELETE FROM song WHERE id = ?", id);
        Ok(())
    }
}

/// Hoist the non-deterministic arguments in. Runs exactly once, at the
/// originating client; from here the values are frozen in the log forever.
///
/// The caller supplies the uuid and the clock, because those are the two things
/// it is allowed to have. Which fields they belong in is decided here, so that
/// knowledge lives with the mutation rather than with the engine.
pub fn fill_auto(mutation: &mut Value, uuid: Vec<u8>, now_ms: i64) {
    match petros_schema::cbor::field(mutation, "t")
        .and_then(petros_schema::cbor::as_text)
        .as_deref()
    {
        Some("AddSong") => {
            set(mutation, "id", Value::Bytes(uuid));
            set(mutation, "added_ms", Value::Integer(now_ms.into()));
        }
        Some("Favorite") | Some("FavoriteAll") => {
            set(mutation, "favorited_ms", Value::Integer(now_ms.into()));
        }
        // Five songs out of one seed.
        //
        // The titles are just "Track 1".."Track 5", but the *ids* cannot be:
        // they have to be unique and `apply` may not invent them, because the
        // only thing it can reach is [`Host`]. So the one uuid is expanded here,
        // in the single place non-determinism is allowed, and the log freezes it.
        Some("AddAlbum") => {
            let mut seed = Seed::from(&uuid);
            let album = ALBUMS[(seed.next() % ALBUMS.len() as u64) as usize];
            let tracks = (1..=TRACKS)
                .map(|n| {
                    Value::Map(vec![
                        (Value::Text("id".into()), Value::Bytes(seed.id())),
                        (
                            Value::Text("title".into()),
                            Value::Text(format!("Track {n}")),
                        ),
                        (Value::Text("artist".into()), Value::Text(album.into())),
                    ])
                })
                .collect();
            set(mutation, "tracks", Value::Array(tracks));
            set(mutation, "added_ms", Value::Integer(now_ms.into()));
        }
        _ => {}
    }
}

const TRACKS: usize = 5;

/// Artists to attribute a generated album to. Chosen from the seed, so the
/// choice is frozen in the log like every other generated value.
const ALBUMS: [&str; 4] = ["Bicep", "Floating Points", "Jamie xx", "Caribou"];

/// xorshift128+, seeded from the uuid the caller supplied.
///
/// Deliberately not a good random number generator — it is a *deterministic
/// expansion* of one non-deterministic seed, which is the only shape the log
/// can hold. The unpredictability is the uuid; everything after it is a pure
/// function of that, which is why replaying the entry reproduces the rows.
struct Seed(u64, u64);

impl Seed {
    fn from(bytes: &[u8]) -> Self {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(0x1000_0000_01b3);
        }
        Seed(h | 1, h.rotate_left(31) | 1)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        let y = self.1;
        self.0 = y;
        x ^= x << 23;
        x ^= x >> 17;
        x ^= y ^ (y >> 26);
        self.1 = x;
        x.wrapping_add(y)
    }

    fn id(&mut self) -> Vec<u8> {
        let (a, b) = (self.next(), self.next());
        let mut out = Vec::with_capacity(16);
        out.extend_from_slice(&a.to_be_bytes());
        out.extend_from_slice(&b.to_be_bytes());
        out
    }
}
