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
//! # There is no SQL here
//!
//! Reads and writes are the same shape: `db.select(query)` and `db.put(&row)`,
//! over row types generated from `schema.sql`. A misspelled column is a compile
//! error rather than a missing row on a device, and a write says what it
//! changed — which is what an incrementally maintained query will need.

use petros_schema::prelude::*;

// `#[mutation]` and `#[query]` each emit a method on the peer a foreign caller
// talks to. UniFFI will not take a qualified self-type, so the name has to be
// in scope here rather than in what they generate.
#[cfg(feature = "foreign")]
use crate::Peer;

use crate::schema::tables::{Favorite, Song as SongRow};

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
    if db.exists::<SongRow>(&SongRow::key_of(&id)) {
        return Ok(());
    }
    // `pos` is read out of current state: an intent, not a fact. It is what
    // makes the rebase visible when an entry lands underneath yours.
    let last = last_pos(db);
    db.put(&SongRow {
        id: id.to_vec(),
        title: title.trim().to_string(),
        artist: artist.trim().to_string(),
        pos: last + 1,
        added_ms,
        actor: actor.to_string(),
    })?;
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
    if !db.exists::<SongRow>(&SongRow::key_of(&id)) {
        return Ok(());
    }
    if db.exists::<Favorite>(&Favorite::key_of(&id)) {
        return Ok(());
    }
    let last = last_favorite_pos(db);
    db.put(&Favorite {
        song_id: id.to_vec(),
        pos: last + 1,
        favorited_ms,
        actor: actor.to_string(),
    })?;
    Ok(())
}

/// Take a song back out of the playlist. The song itself stays.
#[mutation]
pub fn unfavorite(db: &mut Db, id: Id) -> Result {
    db.delete::<Favorite>(&Favorite::key_of(&id))?;
    Ok(())
}

/// Favourite everything in the library that is not already favourited.
///
/// One entry rather than one per song, so it covers songs another peer added in
/// the meantime — that is what makes it an intent, and why it cannot be the
/// client sending N of them.
///
/// It was one `INSERT ... SELECT` with a window function when this was SQL. It
/// is a loop now, and that is the price of a write that says what it changed:
/// a statement that inserts a thousand rows produces one result and no record
/// of which rows they were, which is exactly what an incremental view cannot
/// work from.
#[mutation]
pub fn favorite_all(db: &mut Db, favorited_ms: Now, actor: Actor) -> Result {
    let mut pos = last_favorite_pos(db);
    // Ordered, because the positions it assigns go into the log and every
    // replica has to assign the same ones.
    let songs = db.select(
        SongRow::all()
            .order_by(SongRow::pos.asc())
            .order_by(SongRow::id.asc()),
    );
    for song in songs {
        if db.exists::<Favorite>(&Favorite::key_of(&song.id)) {
            continue;
        }
        pos += 1;
        db.put(&Favorite {
            song_id: song.id,
            pos,
            favorited_ms,
            actor: actor.to_string(),
        })?;
    }
    Ok(())
}

/// Remove a song from the library, and from the playlist with it.
#[mutation]
pub fn remove_song(db: &mut Db, id: Id) -> Result {
    db.delete::<Favorite>(&Favorite::key_of(&id))?;
    db.delete::<SongRow>(&SongRow::key_of(&id))?;
    Ok(())
}

// The two helpers take `&mut impl Store` rather than `&mut Db`. Inside a
// `#[mutation]` the store is a type parameter — the same body runs over SQLite
// natively and over the host's store through the sandbox ABI — and `Db` is only
// the marker that stands for it in a signature the attribute rewrites.

/// The end of the library, which is where a new song goes.
fn last_pos(db: &mut impl Store) -> i64 {
    db.select(SongRow::all().order_by(SongRow::pos.desc()).limit(1))
        .first()
        .map(|s| s.pos)
        .unwrap_or(0)
}

/// The end of the playlist. `MAX(pos)`, as a query that reads one row.
fn last_favorite_pos(db: &mut impl Store) -> i64 {
    db.select(Favorite::all().order_by(Favorite::pos.desc()).limit(1))
        .first()
        .map(|f| f.pos)
        .unwrap_or(0)
}

// -------------------------------------------------------------------- queries

/// The whole library, in the order songs were added.
///
/// `ORDER BY` is explicit here as everywhere: SQLite's natural order is not a
/// contract, and two peers showing the same rows in different orders is a bug
/// that only appears on someone else's machine.
#[query]
pub fn library(db: &mut Db) -> Result<Vec<Song>> {
    // A song *with* its favourite, which is a tree rather than a join: a song
    // that is not favourited is still a row, carrying nothing. That is the LEFT
    // JOIN, and it is the relationship's shape rather than a keyword.
    let rows = db.select_with(library_query(), SongRow::favorite, Favorite::all());
    Ok(rows.iter().map(song_of).collect())
}

/// The query `library` answers, written once so that running it and maintaining
/// it cannot drift apart.
#[cfg(feature = "storage")]
fn library_query() -> petros_schema::Query<SongRow> {
    SongRow::all()
        .order_by(SongRow::pos.asc())
        .order_by(SongRow::id.asc())
}

/// `library`, maintained rather than re-run.
///
/// A client holds one of these and hands it what each mutation changed, instead
/// of reading the whole list back on every frame. Same query, same rows, same
/// order — [`library_query`] is the single definition of all three.
#[cfg(feature = "storage")]
pub type LibraryView = petros::ivm::View<SongRow>;

/// Build one. Hydrate it once against a store, then feed it
/// `Client::take_changes()`.
#[cfg(feature = "storage")]
pub fn library_view() -> LibraryView {
    petros::ivm::View::related(library_query(), SongRow::favorite, Favorite::all())
}

/// Read a maintained view the way `library` reads a fetched one.
#[cfg(feature = "storage")]
pub fn songs_of(view: &LibraryView) -> Vec<Song> {
    view.with::<Favorite>().iter().map(song_of).collect()
}

/// The favourites playlist, in playlist order.
#[query]
pub fn favorites(db: &mut Db) -> Result<Vec<Song>> {
    // Read from the other end: favourites, each carrying its song. A favourite
    // whose song is gone carries nothing and is dropped, which is the INNER
    // JOIN — and `remove_song` deletes both, so it should not arise.
    let rows = db.select_with(
        Favorite::all()
            .order_by(Favorite::pos.asc())
            .order_by(Favorite::song_id.asc()),
        Favorite::song,
        SongRow::all(),
    );
    Ok(rows
        .iter()
        .filter_map(|r| {
            let song = r.one()?;
            Some(Song {
                id: id_of(&song.id),
                title: song.title.clone(),
                artist: song.artist.clone(),
                pos: song.pos,
                added_ms: song.added_ms,
                actor: song.actor.clone(),
                favorite_pos: Some(r.row.pos),
            })
        })
        .collect())
}

/// A song row and its place in the playlist, as a client reads it.
#[cfg(feature = "storage")]
fn song_of(row: &With<SongRow, Favorite>) -> Song {
    Song {
        id: id_of(&row.row.id),
        title: row.row.title.clone(),
        artist: row.row.artist.clone(),
        pos: row.row.pos,
        added_ms: row.row.added_ms,
        actor: row.row.actor.clone(),
        favorite_pos: row.one().map(|f| f.pos),
    }
}

peer!(add_song, favorite, unfavorite, favorite_all, remove_song);
