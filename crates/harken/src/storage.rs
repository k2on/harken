//! Everything that needs a real SQLite: the read model and the `petros::App`
//! the linked peers run.
//!
//! Reads and writes go through the same thing, and that is the point. Every
//! statement in this crate — the queries here and the mutations in
//! [`domain`](crate::domain) — is prepared against `schema.sql` at build time by
//! `petros-sql`, with SQLite as the judge. Rename a column there and both halves
//! stop compiling.
//!
//! There is no ORM. There was one, for reads only, which meant the schema was
//! described twice — once as `table!` and once as DDL — and checked two
//! different ways, one of which could not reach the write path at all because
//! `apply` compiles to wasm and has no database in it.
//!
//! Behind the `storage` feature, because the wasm build wants
//! [`domain`](crate::domain) and none of this.

use ciborium::value::Value;
use petros::backend::SqliteStore;
use petros::{ActorId, App, AutoCtx, Connection, Id, Mutation, MutationError, Transaction};
use serde::{Deserialize, Serialize};

/// A song, and where it sits in the favourites playlist if it is on it.
#[derive(Debug, Clone)]
pub struct Song {
    pub id: Id,
    pub title: String,
    pub artist: String,
    pub pos: i64,
    pub added_ms: i64,
    pub actor: String,
    /// `Some(n)` if favourited, and `n` is its place in the playlist. Recomputed
    /// on every replay, which is what makes the rebase visible: favourite
    /// something offline and it lands after whatever arrived while you were
    /// away.
    pub favorite_pos: Option<i64>,
}

impl Song {
    pub fn favorited(&self) -> bool {
        self.favorite_pos.is_some()
    }
}

/// Sixteen bytes out of a BLOB column. A row whose id is not sixteen bytes did
/// not come from `apply`, and there is nothing useful to do with it.
fn id_of(bytes: &[u8]) -> Id {
    Id(petros::uuid::Uuid::from_slice(bytes).unwrap_or(petros::uuid::Uuid::nil()))
}

/// The whole library, in the order songs were added.
///
/// `ORDER BY` is explicit here as everywhere: SQLite's natural order is not a
/// contract, and two peers showing the same rows in different orders is a bug
/// that only appears on someone else's machine.
pub fn library(conn: &mut Connection) -> petros::Result<Vec<Song>> {
    let mut store = SqliteStore(conn);
    // `f.pos` is aliased because `s.pos` is already called `pos`, and it is
    // nullable because the join is a left join — neither of which SQLite can
    // tell us, so both are said here.
    let rows = petros_sql::query!(
        store,
        "SELECT s.id, s.title, s.artist, s.pos, s.added_ms, s.actor,
                f.pos AS \"favorite_pos?: Int\"
           FROM song s LEFT JOIN favorite f ON f.song_id = s.id
          ORDER BY s.pos, s.id"
    );
    Ok(rows
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
pub fn favorites(conn: &mut Connection) -> petros::Result<Vec<Song>> {
    let mut store = SqliteStore(conn);
    let rows = petros_sql::query!(
        store,
        "SELECT s.id, s.title, s.artist, s.pos, s.added_ms, s.actor,
                f.pos AS \"favorite_pos: Int\"
           FROM song s JOIN favorite f ON f.song_id = s.id
          ORDER BY f.pos, s.id"
    );
    Ok(rows
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

// ---------------------------------------------------------------- the writes

/// One mutation, as the bytes the log stores.
///
/// Not a Rust enum mirroring the variants, and that is deliberate: a peer that
/// has never heard of a variant still carries it through the log intact, and
/// applies it as soon as it has a build that knows what it means.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Payload(pub Value);

impl Mutation for Payload {
    fn fill_auto(&mut self, ctx: &mut AutoCtx) {
        let uuid = ctx.uuid().as_uuid().as_bytes().to_vec();
        crate::domain::fill_auto(&mut self.0, uuid, ctx.now_ms());
    }

    fn apply(&self, tx: &mut Transaction, actor: &ActorId) -> Result<(), MutationError> {
        crate::domain::apply(&mut SqliteStore(tx.conn()), &self.0, actor.as_str())
            .map_err(MutationError::rejected)
    }
}

/// The app: Petros's tables plus these two.
pub struct HarkenApp;

impl App for HarkenApp {
    type Mutation = Payload;
    const SCHEMA: &'static str = SCHEMA;
}

/// The one description of this app's tables.
///
/// Petros runs it on every open, and `petros-sql` prepares every statement in
/// this crate against it at build time. There is no second copy to drift from,
/// which is what an ORM's `table!` would have been.
pub const SCHEMA: &str = include_str!("../schema.sql");

// ------------------------------------------------------------------ authoring

/// Author a mutation, as [`petros_schema::author`] does it, wrapped in this
/// app's payload type.
///
/// The conversion is protocol rather than domain — the verb goes in `t`, and a
/// field named `id` or ending `_id` becomes the sixteen bytes the log uses — so
/// it lives in the schema crate, beside the macro that declares the verbs and
/// the generator that emits the TypeScript calling them.
pub fn from_value(kind: &str, args: serde_json::Value) -> Result<Payload, String> {
    petros_schema::author::from_value(kind, args).map(Payload)
}

/// As [`from_value`], for a caller that has the arguments as JSON text — which
/// is every foreign one, since it has no CBOR encoder.
pub fn from_json(kind: &str, args_json: &str) -> Result<Payload, String> {
    petros_schema::author::from_json(kind, args_json).map(Payload)
}

// The verbs the Rust peers spell out. Conveniences over [`from_value`], not a
// second encoder. The `expect`s cannot fire: the only fallible step is parsing
// a uuid, and these format one rather than taking it from a caller.

pub fn add_song(title: &str, artist: &str) -> Payload {
    from_value(
        "AddSong",
        serde_json::json!({ "title": title, "artist": artist }),
    )
    .expect("text is always encodable")
}

pub fn add_album() -> Payload {
    from_value("AddAlbum", serde_json::json!({})).expect("no arguments to encode")
}

pub fn favorite(id: &[u8; 16]) -> Payload {
    let id = petros::uuid::Uuid::from_bytes(*id).to_string();
    from_value("Favorite", serde_json::json!({ "id": id })).expect("a formatted uuid always parses")
}

pub fn unfavorite(id: &[u8; 16]) -> Payload {
    let id = petros::uuid::Uuid::from_bytes(*id).to_string();
    from_value("Unfavorite", serde_json::json!({ "id": id }))
        .expect("a formatted uuid always parses")
}

pub fn favorite_all() -> Payload {
    from_value("FavoriteAll", serde_json::json!({})).expect("no arguments to encode")
}

pub fn remove_song(id: &[u8; 16]) -> Payload {
    let id = petros::uuid::Uuid::from_bytes(*id).to_string();
    from_value("RemoveSong", serde_json::json!({ "id": id }))
        .expect("a formatted uuid always parses")
}
