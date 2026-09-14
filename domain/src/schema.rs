//! The model: what a row is, and what the tables are.
//!
//! One description. `schema.sql` is what `migrate` runs, and `tables!` asks
//! SQLite what is in it — the columns, their types, the keys, and the foreign
//! keys — so the row types and the relationships between them are generated
//! rather than written. Rename a column there and the call sites using it stop
//! compiling.

#[cfg(feature = "storage")]
use petros_schema::Id;

/// The tables, generated from `schema.sql`: a row type each, plus a typed
/// constant per column and per relationship.
///
/// In a module of their own because these are the *tables* and [`Item`] below
/// is the view a client reads — a different shape, one row of `media` with the
/// playlist position that lives on another table folded in.
pub mod tables {
    petros_sql::tables!();
}

// The view a client reads: one playable thing, whatever kind it is, with where
// it sits in the playlist folded in. Behind `storage`, like the read model that
// produces it — the sandbox applies mutations and never reads a row back.
#[cfg(feature = "storage")]
petros_schema::row! {
    /// Something playable — a song today, an episode or a sermon later — and
    /// where it sits in the favourites playlist if it is on it.
    ///
    /// Deliberately kind-neutral: a list renders this and nothing else, so a
    /// new kind reaches every client without a screen learning about it. What
    /// is true of one kind only lives on that kind's own table.
    Item => {
        /// Sixteen bytes in SQLite and in the log, and typed: an `Id<Media>`
        /// cannot be passed where a playlist's is wanted. The canonical
        /// 8-4-4-4-12 string on the far side, because that is what a foreign
        /// caller can hold, compare and use as a list key.
        id: Id<tables::Media> => String { |id| id.to_string() },
        /// `"song"` today. What a client switches on when it wants to show a
        /// kind differently, and the name of the table carrying the rest.
        kind: String,
        title: String,
        /// Whoever made it: a song's artist, a sermon's speaker, an episode's
        /// show. The precise fact is on the side table; this is the line a
        /// list draws under the title.
        creator: String,
        duration_ms: i64,
        /// The name of the file in the media store. The bytes never enter the
        /// log — a client fetches them over HTTP from the server.
        file: String,
        /// Recomputed on every replay from `MAX(pos) + 1`, which is what makes
        /// the rebase visible: something added while offline moves down as
        /// confirmed entries land underneath it.
        pos: i64,
        added_ms: i64,
        /// Who added it: the identity the server verified, which is
        /// `ctx.user.id` inside a mutation. A name from an identity provider
        /// rather than a row id, so it is text and not a tagged id.
        user_id: String,
        /// `Some(n)` if it is on the playlist this was read against, and `n`
        /// is its place on it. Which playlist that is belongs to the client:
        /// a heart means "on the one I am showing you".
        ///
        /// `-1` on the far side rather than an optional: positions start at 1,
        /// so the sentinel is unambiguous and the record stays flat across the
        /// boundary.
        playlist_pos: Option<i64> => i64 { |p| p.unwrap_or(-1) },
    } and {
        /// Derivable from `playlist_pos`, and carried anyway so the sentinel's
        /// meaning stays on this side of the boundary rather than in the
        /// TypeScript reading it.
        on_playlist: bool = |row| row.playlist_pos.is_some(),
    };

    /// A playlist. "Favourites" is one of these and nothing more — which
    /// playlist a heart stands for is the client's choice, not the domain's.
    Playlist => {
        id: Id<tables::Playlist> => String { |id| id.to_string() },
        name: String,
        pos: i64,
        created_ms: i64,
        user_id: String,
    };
}

#[cfg(feature = "storage")]
impl Item {
    /// On the playlist this item was read against.
    pub fn on_playlist(&self) -> bool {
        self.playlist_pos.is_some()
    }
}

/// The kinds of thing that can be in the library. One today.
///
/// A `&str` rather than an enum because it is a column in the log: a peer that
/// has never heard of a kind still carries the row, lists it, and plays it —
/// everything a list needs is on `media` — and only a screen that wants to
/// treat that kind specially has to know the name.
///
/// Not behind `storage`: `apply` writes the column, and `apply` is exactly what
/// the sandbox build has and the storage build does not.
pub mod kind {
    pub const SONG: &str = "song";
}

/// The one description of this app's tables.
///
/// Petros runs it on every open, and `petros-sql` prepares every statement in
/// this crate against it at build time. There is no second copy to drift from.
pub const SCHEMA: &str = include_str!("../schema.sql");

// An id no longer needs recovering from a column. `tables!` types a key column
// as the `Id<Media>` or `Id<Playlist>` it is, so a read model carries the row's
// id rather than a conversion of it — and the conversion was the place a
// media id could quietly become a playlist one.
