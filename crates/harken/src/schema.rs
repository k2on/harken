//! The model: what a row is, and what the tables are.
//!
//! Two descriptions and they are held together. `schema.sql` is what `migrate`
//! runs and what `petros-sql` prepares every statement in `functions.rs`
//! against at build time — rename a column there and the call sites using it
//! stop compiling. The rows below are what those statements read back.

use petros::Id;

petros_schema::row! {
    /// A song, and where it sits in the favourites playlist if it is on it.
    Song => {
        /// Sixteen bytes in SQLite and in the log. The canonical 8-4-4-4-12
        /// string on the far side, because that is what a foreign caller can
        /// hold, compare and use as a list key.
        id: Id => String { |id| id.to_string() },
        title: String,
        artist: String,
        /// Recomputed on every replay from `MAX(pos) + 1`, which is what makes
        /// the rebase visible: a song added while offline moves down as
        /// confirmed entries land underneath it.
        pos: i64,
        added_ms: i64,
        actor: String,
        /// `Some(n)` if favourited, and `n` is its place in the playlist.
        ///
        /// `-1` on the far side rather than an optional: positions start at 1,
        /// so the sentinel is unambiguous and the record stays flat across the
        /// boundary.
        favorite_pos: Option<i64> => i64 { |p| p.unwrap_or(-1) },
    } and {
        /// Derivable from `favorite_pos`, and carried anyway so the sentinel's
        /// meaning stays on this side of the boundary rather than in the
        /// TypeScript reading it.
        favorited: bool = |row| row.favorite_pos.is_some(),
    };
}

impl Song {
    pub fn favorited(&self) -> bool {
        self.favorite_pos.is_some()
    }
}

/// The one description of this app's tables.
///
/// Petros runs it on every open, and `petros-sql` prepares every statement in
/// this crate against it at build time. There is no second copy to drift from.
pub const SCHEMA: &str = include_str!("../schema.sql");

/// Sixteen bytes out of a BLOB column. A row whose id is not sixteen bytes did
/// not come from a mutation, and there is nothing useful to do with it.
pub(crate) fn id_of(bytes: &[u8]) -> Id {
    Id(petros::uuid::Uuid::from_slice(bytes).unwrap_or(petros::uuid::Uuid::nil()))
}
