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
    /// where it sits in the favorites playlist if it is on it.
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

    /// A record, as a sidebar lists it: the name, whoever made it, and how
    /// many tracks are on it.
    ///
    /// Its own read model rather than a column on [`Item`], because an album
    /// is true of the *song* kind and nothing else — it lives on `song`, and
    /// the library list is kind-neutral on purpose. A screen that wants to
    /// browse by album is asking a song-shaped question and gets a
    /// song-shaped answer.
    Album => {
        name: String,
        /// The composer, here. Whoever made the first track on it: an album
        /// with two artists is one row under the first, which is a real
        /// limitation and the reason to key this by the pair if it ever bites.
        creator: String,
        tracks: i64,
        /// The cover, as `media.file` spells one: a path relative to the media
        /// root, or a whole URL. Empty when nobody has set one, which is the
        /// normal case — a client draws a square derived from the name then,
        /// and the derived one is a fallback rather than the answer.
        art: String,
    };

    /// What is true of a track as a *song*, for a client drawing those columns.
    ///
    /// Beside [`Item`] rather than folded into it, and that is the whole
    /// point: every field here is true of the song kind and lives on `song`,
    /// so putting them on the kind-neutral library row would put a join behind
    /// every list — the thing `media` exists to avoid. A screen that wants
    /// these asks for them and joins in memory; a screen that does not, does
    /// not pay. It was `TrackAlbum` when `album` was the only one.
    TrackDetail => {
        media_id: Id<tables::Media> => String { |id| id.to_string() },
        album: String,
        /// Where it sits in the album; 0 when nobody said.
        track: i64,
        /// The suite or book inside the work, when the work has them. Empty
        /// otherwise, which is most albums.
        part: String,
        /// `BWV 988`. The one name for a classical work that survives
        /// translation, so it is what two recordings of one piece agree on.
        catalogue: String,
        /// Who played it. `Item::creator` is who *wrote* it, and for this
        /// repertoire they are three hundred years apart.
        performer: String,
        /// The terms the *recording* was offered on, where they are not "none
        /// reserved" — `CC BY-SA 3.0`. Empty for almost everything.
        ///
        /// Beside the performer rather than folded into it, which is where it
        /// was: the demo used to write `"{performer} ({licence})"` into one
        /// column, so a recording nobody was credited on was credited to a
        /// person called `(CC BY-SA 3.0)`. It is a fact about the performance,
        /// it lives on `recording`, and a client draws it next to whoever gave
        /// it — because a credit nobody draws is a condition nobody met.
        licence: String,
        /// Beats per minute; 0 when nobody said. See `schema.sql` for what
        /// counts as somebody saying.
        bpm: i64,
    };

    /// Whoever made something, as a sidebar lists them.
    ///
    /// `media.creator`, so this one *is* kind-neutral: a podcast's show and a
    /// sermon's speaker are artists here too, and a new kind appears in this
    /// list without the query learning about it.
    Artist => {
        name: String,
        tracks: i64,
        /// The picture, on the same terms as [`Album::art`]. Empty when nobody
        /// has set one.
        art: String,
    };

    /// Somebody who wrote something, as a composer index lists them.
    ///
    /// Not the same question as [`Artist`], which is everyone `media.creator`
    /// names — for this repertoire that is the composer too, but for pop it is
    /// the performer and for a podcast it is the show. This one is exactly the
    /// people some `work` is *by*, so a library with no works has none of them
    /// and a client draws no Composers page at all.
    Composer => {
        name: String,
        /// "Bach, Johann Sebastian", or the name again when nobody said.
        sort_name: String,
        /// Years, 0 unknown.
        born: i64,
        died: i64,
        /// How many of their works this library has something of, which is the
        /// number a composer index is actually ranked by.
        works: i64,
        tracks: i64,
        art: String,
    };

    /// A composition, as a composer's page lists them.
    Work => {
        /// The derived key. A client carries it to ask for the recordings and
        /// never draws it — see `work_key` in `functions.rs`.
        id: String,
        title: String,
        composer: String,
        /// `BWV 988`. What disambiguates two works with one title.
        catalogue: String,
        /// "Symphony", "Concerto". Empty when nobody said.
        form: String,
        /// "Baroque". Empty when nobody said.
        period: String,
        /// How many performances of it this library holds, which is the fact a
        /// work page exists to show.
        recordings: i64,
        tracks: i64,
        art: String,
    };

    /// One performance, as a work's page lists them.
    Recording => {
        id: String,
        /// The credits, in billing order, joined — "Hermann Scherchen, London
        /// Symphony Orchestra". What tells two recordings of one work apart,
        /// and the reason this list is worth drawing.
        performers: String,
        /// The year it was recorded, which is not the year it was released.
        /// 0 unknown.
        recorded: i64,
        label: String,
        /// The terms it was offered on, where they are not "none reserved".
        /// Drawn beside the performers, because a credit nobody draws is a
        /// condition nobody met.
        licence: String,
        tracks: i64,
        art: String,
    };

    /// One person on a recording, and what they did on it.
    ///
    /// What a recording page lists under the performers line. `Recording` joins
    /// these into one string because a table has one column for "who";
    /// this is the same people with the roles kept, which is what an
    /// instrument or a conductor can be browsed by.
    Credit => {
        name: String,
        /// `orchestra`, `conductor`, `soloist`, `ensemble`, `choir`, `artist`.
        role: String,
        /// "Piano". Empty unless the role is `soloist`, and empty plenty of
        /// times then too — the source often does not say.
        instrument: String,
        /// Billing order. 0 marks the lumped string `add_song` falls back to
        /// when nobody has said who is who, and a real credit starts at 1.
        pos: i64,
    };

    /// A playlist. "Favorites" is one of these and nothing more — which
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
