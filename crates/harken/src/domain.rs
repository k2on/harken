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
//! The SQL is written out rather than built with Diesel, and that is not a
//! preference: the wasm build has no SQLite and no Diesel, only a channel to
//! the host's. Reads go through the ORM — see [`crate::storage`] — because they
//! never cross that boundary.

use petros_schema::cbor::{set, Value};

// The contract, not a copy of it: the same three methods the wasm module
// imports and the same escaping both builds go through.
pub use petros_schema::{lit, Host, Lit};

petros_schema::mutations! {
    /// Put a song in the library.
    AddSong { title: Text, artist: Text } auto { id: Id, added_ms: Integer } => |host, actor| {
        if title.trim().is_empty() {
            return Err("a song needs a title".into());
        }
        // The same entry arriving twice is a no-op, which is what makes
        // redelivery safe.
        if host.query_exists(&format!(
            "SELECT 1 FROM song WHERE id = {}",
            lit(Lit::Blob(&id))
        )) {
            return Ok(());
        }
        let last = host.query_int("SELECT COALESCE(MAX(pos), 0) FROM song");
        host.exec(&format!(
            "INSERT INTO song (id, title, artist, pos, added_ms, actor) \
             VALUES ({}, {}, {}, {}, {}, {})",
            lit(Lit::Blob(&id)),
            lit(Lit::Text(title.trim())),
            lit(Lit::Text(artist.trim())),
            lit(Lit::Int(last + 1)),
            lit(Lit::Int(added_ms)),
            lit(Lit::Text(actor)),
        ));
        Ok(())
    }

    /// A whole album at once: five tracks from one seed.
    ///
    /// The ids are already in the payload — `fill_auto` put them there at the
    /// originating client — so this is as deterministic as any other apply.
    AddAlbum {} auto { tracks: Array, added_ms: Integer } => |host, actor| {
        // Read the end of the library once, then count up. Re-reading between
        // inserts would give the same answer and cost five more round trips.
        let mut pos = host.query_int("SELECT COALESCE(MAX(pos), 0) FROM song");
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
            if host.query_exists(&format!(
                "SELECT 1 FROM song WHERE id = {}",
                lit(Lit::Blob(&id))
            )) {
                continue;
            }
            pos += 1;
            host.exec(&format!(
                "INSERT INTO song (id, title, artist, pos, added_ms, actor) \
                 VALUES ({}, {}, {}, {}, {}, {})",
                lit(Lit::Blob(&id)),
                lit(Lit::Text(title.trim())),
                lit(Lit::Text(artist.trim())),
                lit(Lit::Int(pos)),
                lit(Lit::Int(added_ms)),
                lit(Lit::Text(actor)),
            ));
        }
        Ok(())
    }

    /// Add a song to the favourites playlist, at the end.
    ///
    /// Favouriting a song that is gone is a no-op rather than an error: an
    /// entry earlier in the log may have removed it. So is favouriting one that
    /// is already there — the playlist is a set with an order, and a song holds
    /// the position it first got.
    Favorite { id: Id } auto { favorited_ms: Integer } => |host, actor| {
        if !host.query_exists(&format!(
            "SELECT 1 FROM song WHERE id = {}",
            lit(Lit::Blob(&id))
        )) {
            return Ok(());
        }
        if host.query_exists(&format!(
            "SELECT 1 FROM favorite WHERE song_id = {}",
            lit(Lit::Blob(&id))
        )) {
            return Ok(());
        }
        // "Put it at the end of the playlist", read from current state. An
        // entry that lands underneath yours moves you down, which is the rebase
        // made visible.
        let last = host.query_int("SELECT COALESCE(MAX(pos), 0) FROM favorite");
        host.exec(&format!(
            "INSERT INTO favorite (song_id, pos, favorited_ms, actor) \
             VALUES ({}, {}, {}, {})",
            lit(Lit::Blob(&id)),
            lit(Lit::Int(last + 1)),
            lit(Lit::Int(favorited_ms)),
            lit(Lit::Text(actor)),
        ));
        Ok(())
    }

    /// Take a song back out of the playlist. The song itself stays.
    Unfavorite { id: Id } => |host, actor| {
        let _ = actor;
        host.exec(&format!(
            "DELETE FROM favorite WHERE song_id = {}",
            lit(Lit::Blob(&id))
        ));
        Ok(())
    }

    /// Favourite everything in the library that is not already favourited.
    ///
    /// One entry rather than one per song, so it covers songs another peer
    /// added in the meantime. That is what makes it an intent.
    FavoriteAll {} auto { favorited_ms: Integer } => |host, actor| {
        host.exec(&format!(
            "INSERT INTO favorite (song_id, pos, favorited_ms, actor) \
             SELECT s.id, \
                    (SELECT COALESCE(MAX(pos), 0) FROM favorite) \
                        + ROW_NUMBER() OVER (ORDER BY s.pos, s.id), \
                    {}, {} \
               FROM song s \
              WHERE NOT EXISTS (SELECT 1 FROM favorite f WHERE f.song_id = s.id) \
              ORDER BY s.pos, s.id",
            lit(Lit::Int(favorited_ms)),
            lit(Lit::Text(actor)),
        ));
        Ok(())
    }

    /// Remove a song from the library, and from the playlist with it.
    RemoveSong { id: Id } => |host, actor| {
        let _ = actor;
        host.exec(&format!(
            "DELETE FROM favorite WHERE song_id = {}",
            lit(Lit::Blob(&id))
        ));
        host.exec(&format!(
            "DELETE FROM song WHERE id = {}",
            lit(Lit::Blob(&id))
        ));
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
                        (Value::Text("title".into()), Value::Text(format!("Track {n}"))),
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
