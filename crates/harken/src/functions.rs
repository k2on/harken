//! Every mutation and every query, each written once.
//!
//! A mutation is an *intent* — "put this at the end of the playlist", not "put
//! this at position 3" — and it is applied by every replica from the log, not
//! here. The body below runs on the server, in the iced client, and inside a
//! wasm sandbox on the phone, from the same source; `tests/conformance.rs`
//! drives every verb through both builds and compares the rows.
//!
//! # What the parameters mean
//!
//! `&mut Db` is the store. `NewId` and `Now` are the only non-determinism a
//! mutation gets, chosen once at the originating client and frozen in the log —
//! a body may not call a clock or invent an id, because two replicas would
//! choose differently and diverge. `Actor` is who authored the entry. Which is
//! which is decided by type, so there is no list to keep in step.
//!
//! # The SQL is checked
//!
//! `petros_sql::exec!` and `query!` prepare each statement against
//! `schema.sql` at build time with SQLite as the judge, so a misspelled column
//! is a compile error even though this runs inside a sandbox that has no
//! SQLite in it.

use petros_schema::prelude::*;

// `#[mutation]` and `#[query]` each emit a method on the peer a foreign caller
// talks to. UniFFI will not take a qualified self-type, so the name has to be
// in scope here rather than in what they generate.
#[cfg(feature = "foreign")]
use crate::Peer;

// Only the queries below use these, and a query is not built for the sandbox.
#[cfg(feature = "storage")]
use crate::schema::{id_of, Song};

// ------------------------------------------------------------------ mutations

/// Put a song in the library.
#[mutation]
pub fn add_song(
    db: &mut Db,
    id: NewId,
    added_ms: Now,
    actor: Actor,
    title: String,
    artist: String,
) -> Result {
    if title.trim().is_empty() {
        return Err("a song needs a title".into());
    }
    // The same entry arriving twice is a no-op, which is what makes redelivery
    // safe.
    if petros_sql::query!(db, "SELECT 1 AS \"found: Int\" FROM song WHERE id = ?", id)
        .first()
        .is_some()
    {
        return Ok(());
    }
    // `pos` is read out of current state: an intent, not a fact. It is what
    // makes the rebase visible when an entry lands underneath yours.
    let last = petros_sql::query!(
        db,
        "SELECT COALESCE(MAX(pos), 0) AS \"last: Int\" FROM song"
    )
    .first()
    .map(|r| r.last)
    .unwrap_or(0);
    let (title, artist) = (title.trim().to_string(), artist.trim().to_string());
    petros_sql::exec!(
        db,
        "INSERT INTO song (id, title, artist, pos, added_ms, actor)
         VALUES (?, ?, ?, ?, ?, ?)",
        id,
        title,
        artist,
        last + 1,
        added_ms,
        actor
    );
    Ok(())
}

/// Add a song to the favourites playlist, at the end.
///
/// Favouriting a song that is gone is a no-op rather than an error: an entry
/// earlier in the log may have removed it. So is favouriting one already there
/// — the playlist is a set with an order, and a song keeps the place it first
/// got.
#[mutation]
pub fn favorite(db: &mut Db, favorited_ms: Now, actor: Actor, id: Id) -> Result {
    if petros_sql::query!(db, "SELECT 1 AS \"found: Int\" FROM song WHERE id = ?", id)
        .first()
        .is_none()
    {
        return Ok(());
    }
    if petros_sql::query!(
        db,
        "SELECT 1 AS \"found: Int\" FROM favorite WHERE song_id = ?",
        id
    )
    .first()
    .is_some()
    {
        return Ok(());
    }
    let last = petros_sql::query!(
        db,
        "SELECT COALESCE(MAX(pos), 0) AS \"last: Int\" FROM favorite"
    )
    .first()
    .map(|r| r.last)
    .unwrap_or(0);
    petros_sql::exec!(
        db,
        "INSERT INTO favorite (song_id, pos, favorited_ms, actor) VALUES (?, ?, ?, ?)",
        id,
        last + 1,
        favorited_ms,
        actor
    );
    Ok(())
}

/// Take a song back out of the playlist. The song itself stays.
#[mutation]
pub fn unfavorite(db: &mut Db, id: Id) -> Result {
    petros_sql::exec!(db, "DELETE FROM favorite WHERE song_id = ?", id);
    Ok(())
}

/// Favourite everything in the library that is not already favourited.
///
/// One entry rather than one per song, so it covers songs another peer added in
/// the meantime — that is what makes it an intent. And one *statement*, which
/// is what checked SQL buys over a query builder: the alternative is a scan and
/// a write per song, and each of those is a boundary crossing on a phone.
#[mutation]
pub fn favorite_all(db: &mut Db, favorited_ms: Now, actor: Actor) -> Result {
    petros_sql::exec!(
        db,
        "INSERT INTO favorite (song_id, pos, favorited_ms, actor)
         SELECT s.id,
                (SELECT COALESCE(MAX(pos), 0) FROM favorite)
                  + ROW_NUMBER() OVER (ORDER BY s.pos, s.id),
                ?, ?
           FROM song s
          WHERE NOT EXISTS (SELECT 1 FROM favorite f WHERE f.song_id = s.id)
          ORDER BY s.pos, s.id",
        favorited_ms,
        actor
    );
    Ok(())
}

/// Remove a song from the library, and from the playlist with it.
#[mutation]
pub fn remove_song(db: &mut Db, id: Id) -> Result {
    petros_sql::exec!(db, "DELETE FROM favorite WHERE song_id = ?", id);
    petros_sql::exec!(db, "DELETE FROM song WHERE id = ?", id);
    Ok(())
}

// -------------------------------------------------------------------- queries

/// The whole library, in the order songs were added.
///
/// `ORDER BY` is explicit here as everywhere: SQLite's natural order is not a
/// contract, and two peers showing the same rows in different orders is a bug
/// that only appears on someone else's machine.
#[query]
pub fn library(db: &mut Db) -> Result<Vec<Song>> {
    // `f.pos` is aliased because `s.pos` is already called `pos`, and it is
    // nullable because the join is a left join — neither of which SQLite can
    // tell us, so both are said here.
    Ok(petros_sql::query!(
        db,
        "SELECT s.id, s.title, s.artist, s.pos, s.added_ms, s.actor,
                f.pos AS \"favorite_pos?: Int\"
           FROM song s LEFT JOIN favorite f ON f.song_id = s.id
          ORDER BY s.pos, s.id"
    )
    .into_iter()
    .map(|r| Song {
        id: id_of(&r.id),
        title: r.title,
        artist: r.artist,
        pos: r.pos,
        added_ms: r.added_ms,
        actor: r.actor,
        favorite_pos: r.favorite_pos,
    })
    .collect())
}

/// The favourites playlist, in playlist order.
#[query]
pub fn favorites(db: &mut Db) -> Result<Vec<Song>> {
    Ok(petros_sql::query!(
        db,
        "SELECT s.id, s.title, s.artist, s.pos, s.added_ms, s.actor,
                f.pos AS \"favorite_pos: Int\"
           FROM song s JOIN favorite f ON f.song_id = s.id
          ORDER BY f.pos, s.id"
    )
    .into_iter()
    .map(|r| Song {
        id: id_of(&r.id),
        title: r.title,
        artist: r.artist,
        pos: r.pos,
        added_ms: r.added_ms,
        actor: r.actor,
        favorite_pos: Some(r.favorite_pos),
    })
    .collect())
}

peer!(add_song, favorite, unfavorite, favorite_all, remove_song);
