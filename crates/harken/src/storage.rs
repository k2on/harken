//! Everything that needs a real SQLite: the tables, the rows, the Diesel host
//! and the `petros::App` the native peers run.
//!
//! The read model goes through Diesel's DSL, with `check_for_backend` verifying
//! at compile time that each model still matches its table. Writes go through
//! checked SQL instead — `apply` is compiled to wasm as well as linked, and the
//! wasm build has no SQLite and no Diesel, only a channel to the host's. What
//! covers that half is `petros-sql`, which prepares every statement against
//! `schema.sql` at build time.
//!
//! Behind the `storage` feature, because the wasm build wants
//! [`domain`](crate::domain) and none of this.

use ciborium::value::Value;
use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::sqlite::Sqlite as SqliteBackend;
use petros::backend::SqliteStore;
use petros::{ActorId, App, AutoCtx, Connection, Id, Mutation, MutationError, Transaction};
use serde::{Deserialize, Serialize};

diesel::table! {
    song (id) {
        id -> Binary,
        title -> Text,
        artist -> Text,
        pos -> BigInt,
        added_ms -> BigInt,
        actor -> Text,
    }
}

diesel::table! {
    favorite (song_id) {
        song_id -> Binary,
        pos -> BigInt,
        favorited_ms -> BigInt,
        actor -> Text,
    }
}

diesel::joinable!(favorite -> song (song_id));
diesel::allow_tables_to_appear_in_same_query!(song, favorite);

/// One row of the library. Read-only: rows are produced by `apply`, never by
/// this crate.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = song, check_for_backend(SqliteBackend))]
pub struct SongRow {
    pub id: Id,
    pub title: String,
    pub artist: String,
    pub pos: i64,
    pub added_ms: i64,
    pub actor: String,
}

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

fn song_of((row, favorite_pos): (SongRow, Option<i64>)) -> Song {
    Song {
        id: row.id,
        title: row.title,
        artist: row.artist,
        pos: row.pos,
        added_ms: row.added_ms,
        actor: row.actor,
        favorite_pos,
    }
}

/// The whole library, in the order songs were added. Always ordered explicitly.
pub fn library(conn: &mut Connection) -> petros::Result<Vec<Song>> {
    Ok(song::table
        .left_join(favorite::table)
        .select((SongRow::as_select(), favorite::pos.nullable()))
        .order((song::pos.asc(), song::id.asc()))
        .load::<(SongRow, Option<i64>)>(conn)?
        .into_iter()
        .map(song_of)
        .collect())
}

/// The favourites playlist, in playlist order.
pub fn favorites(conn: &mut Connection) -> petros::Result<Vec<Song>> {
    Ok(song::table
        .inner_join(favorite::table)
        .select((SongRow::as_select(), favorite::pos))
        .order((favorite::pos.asc(), song::id.asc()))
        .load::<(SongRow, i64)>(conn)?
        .into_iter()
        .map(|(row, pos)| song_of((row, Some(pos))))
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

    fn migrate(conn: &mut Connection) -> petros::Result<()> {
        conn.batch_execute(SCHEMA)?;
        Ok(())
    }
}

/// The one description of this app's tables.
///
/// `migrate` runs it, and `petros-sql` prepares every statement in the domain
/// against it at build time. There is no second copy to drift from.
pub const SCHEMA: &str = include_str!("../schema.sql");

// ------------------------------------------------------------------ authoring

/// Build a mutation from a verb name and its arguments, without knowing what
/// either means.
///
/// The payload is just `{ "t": kind, ...args }`. Auto-filled fields are not
/// this function's problem: `fill_auto` appends whatever the verb needs
/// afterwards, so `Add` here is `{"text": "..."}` and the id and the timestamp
/// arrive later, chosen by the domain.
///
/// One convention, and it is protocol rather than domain: a field named `id`
/// holding a canonical uuid becomes the sixteen-byte string the log uses.
pub fn from_value(kind: &str, args: serde_json::Value) -> Result<Payload, String> {
    let serde_json::Value::Object(args) = args else {
        return Err("the arguments should be a json object".into());
    };
    let mut fields = vec![(Value::Text("t".into()), Value::Text(kind.to_string()))];
    for (name, value) in args {
        let is_id = name == "id" || name.ends_with("_id");
        fields.push((Value::Text(name), json_to_cbor(value, is_id)?));
    }
    Ok(Payload(Value::Map(fields)))
}

/// As [`from_value`], for a caller that has the arguments as JSON text — which
/// is every foreign one, since it has no CBOR encoder.
pub fn from_json(kind: &str, args_json: &str) -> Result<Payload, String> {
    let args: serde_json::Value = if args_json.trim().is_empty() {
        serde_json::Value::Object(Default::default())
    } else {
        serde_json::from_str(args_json).map_err(|e| format!("the arguments are not json: {e}"))?
    };
    from_value(kind, args)
}

fn json_to_cbor(value: serde_json::Value, is_id: bool) -> Result<Value, String> {
    use serde_json::Value as J;
    Ok(match value {
        J::Null => Value::Null,
        J::Bool(b) => Value::Bool(b),
        J::Number(n) => match n.as_i64() {
            Some(i) => Value::Integer(i.into()),
            // `docs/decisions.md`: no floats anywhere near the log.
            None => return Err(format!("{n} is not an integer")),
        },
        J::String(s) if is_id => Value::Bytes(
            petros::uuid::Uuid::parse_str(&s)
                .map_err(|e| format!("not an id: {e}"))?
                .as_bytes()
                .to_vec(),
        ),
        J::String(s) => Value::Text(s),
        J::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|v| json_to_cbor(v, false))
                .collect::<Result<_, _>>()?,
        ),
        J::Object(entries) => Value::Map(
            entries
                .into_iter()
                .map(|(k, v)| {
                    let is_id = k == "id" || k.ends_with("_id");
                    Ok((Value::Text(k), json_to_cbor(v, is_id)?))
                })
                .collect::<Result<Vec<_>, String>>()?,
        ),
    })
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
