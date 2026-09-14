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
//! choose differently and diverge. `ctx: &Ctx` is who authored the entry
//! (`ctx.user.id`, which the server verified) and under which login
//! (`ctx.session.id`). Which is which is decided by type, so there is no list
//! to keep in step.
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

use crate::schema::tables::{Media, Playlist, PlaylistItem, Song};

// Only the queries below use these, and a query is not built for the sandbox.
#[cfg(feature = "storage")]
use crate::schema::Item;

// ------------------------------------------------------------------ mutations

/// Put a song in the library.
///
/// Two rows: what every playable thing has, and what only a song has. A second
/// kind is another verb beside this one writing `media` the same way and its
/// own side table — nothing here changes for it.
#[mutation]
pub fn add_song(
    db: &mut Db,
    ctx: &Ctx,
    id: NewId<Media>,
    added_ms: Now,
    title: String,
    artist: String,
    album: String,
    duration_ms: i64,
    file: String,
) -> Result {
    if title.trim().is_empty() {
        return Err("a song needs a title".into());
    }
    // The same entry arriving twice is a no-op, which is what makes redelivery
    // safe.
    if db.exists::<Media>(&Media::key_of(&id)) {
        return Ok(());
    }
    // `pos` is read out of current state: an intent, not a fact. It is what
    // makes the rebase visible when an entry lands underneath yours.
    let last = last_pos(db);
    db.put(&Media {
        id,
        kind: crate::schema::kind::SONG.to_string(),
        title: title.trim().to_string(),
        creator: artist.trim().to_string(),
        duration_ms,
        file: file.trim().to_string(),
        pos: last + 1,
        added_ms,
        user_id: ctx.user.id.clone(),
    })?;
    db.put(&Song {
        media_id: id,
        album: album.trim().to_string(),
    })?;
    Ok(())
}

/// Make a playlist.
#[mutation]
pub fn create_playlist(
    db: &mut Db,
    ctx: &Ctx,
    id: NewId<Playlist>,
    created_ms: Now,
    name: String,
) -> Result {
    if name.trim().is_empty() {
        return Err("a playlist needs a name".into());
    }
    if db.exists::<Playlist>(&Playlist::key_of(&id)) {
        return Ok(());
    }
    let last = db
        .select(Playlist::all().order_by(Playlist::pos.desc()).limit(1))
        .first()
        .map(|p| p.pos)
        .unwrap_or(0);
    db.put(&Playlist {
        id,
        name: name.trim().to_string(),
        pos: last + 1,
        created_ms,
        user_id: ctx.user.id.clone(),
    })?;
    Ok(())
}

/// Put something on a playlist, at the end of it.
///
/// A playlist holds an item once — it is a set with an order — so adding one
/// already there keeps the place it first got. Adding to a playlist that is
/// gone, or an item that is gone, is a no-op rather than an error: an entry
/// earlier in the log may have removed either.
#[mutation]
pub fn add_to_playlist(
    db: &mut Db,
    ctx: &Ctx,
    added_ms: Now,
    playlist_id: Id<Playlist>,
    media_id: Id<Media>,
) -> Result {
    if !db.exists::<Playlist>(&Playlist::key_of(&playlist_id)) {
        return Ok(());
    }
    if !db.exists::<Media>(&Media::key_of(&media_id)) {
        return Ok(());
    }
    if db.exists::<PlaylistItem>(&PlaylistItem::key_of(&playlist_id, &media_id)) {
        return Ok(());
    }
    let last = last_playlist_pos(db, playlist_id);
    db.put(&PlaylistItem {
        playlist_id,
        media_id,
        pos: last + 1,
        added_ms,
        user_id: ctx.user.id.clone(),
    })?;
    Ok(())
}

/// Take something off a playlist. The item itself stays in the library.
#[mutation]
pub fn remove_from_playlist(db: &mut Db, playlist_id: Id<Playlist>, media_id: Id<Media>) -> Result {
    db.delete::<PlaylistItem>(&PlaylistItem::key_of(&playlist_id, &media_id))?;
    Ok(())
}

/// Put everything in the library on a playlist.
///
/// One entry rather than one per item, and that is the point: it is an
/// *intent*, so a replica replaying it covers whatever else was in the library
/// by then — including items another peer added while this was in flight. A
/// client sending N `AddToPlaylist` entries instead would freeze the list as
/// it looked when the button was pressed.
///
/// It was one `INSERT ... SELECT` with a window function when this was SQL. It
/// is a loop now, and that is the price of a write that says what it changed:
/// a statement that inserts a thousand rows produces one result and no record
/// of which rows they were, which is exactly what an incremental view cannot
/// work from.
#[mutation]
pub fn add_all_to_playlist(
    db: &mut Db,
    ctx: &Ctx,
    added_ms: Now,
    playlist_id: Id<Playlist>,
) -> Result {
    if !db.exists::<Playlist>(&Playlist::key_of(&playlist_id)) {
        return Ok(());
    }
    let mut pos = last_playlist_pos(db, playlist_id);
    // Ordered, because the positions it assigns go into the log and every
    // replica has to assign the same ones.
    let items = db.select(
        Media::all()
            .order_by(Media::pos.asc())
            .order_by(Media::id.asc()),
    );
    for item in items {
        if db.exists::<PlaylistItem>(&PlaylistItem::key_of(&playlist_id, &item.id)) {
            continue;
        }
        pos += 1;
        db.put(&PlaylistItem {
            playlist_id,
            media_id: item.id,
            pos,
            added_ms,
            user_id: ctx.user.id.clone(),
        })?;
    }
    Ok(())
}

/// Take something out of the library, and off every playlist holding it.
#[mutation]
pub fn remove_media(db: &mut Db, id: Id<Media>) -> Result {
    let on = db.select(
        PlaylistItem::all()
            .filter(PlaylistItem::media_id.eq(id))
            .order_by(PlaylistItem::playlist_id.asc()),
    );
    for item in on {
        db.delete::<PlaylistItem>(&PlaylistItem::key_of(&item.playlist_id, &item.media_id))?;
    }
    db.delete::<Song>(&Song::key_of(&id))?;
    db.delete::<Media>(&Media::key_of(&id))?;
    Ok(())
}

// The two helpers take `&mut impl Store` rather than `&mut Db`. Inside a
// `#[mutation]` the store is a type parameter — the same body runs over SQLite
// natively and over the host's store through the sandbox ABI — and `Db` is only
// the marker that stands for it in a signature the attribute rewrites.

/// The end of the library, which is where anything new goes — one order across
/// every kind, because the library is one list.
fn last_pos(db: &mut impl Store) -> i64 {
    db.select(Media::all().order_by(Media::pos.desc()).limit(1))
        .first()
        .map(|s| s.pos)
        .unwrap_or(0)
}

/// The end of one playlist. `MAX(pos)` within it, as a query that reads one row.
fn last_playlist_pos(db: &mut impl Store, playlist: Id<Playlist>) -> i64 {
    db.select(
        PlaylistItem::all()
            .filter(PlaylistItem::playlist_id.eq(playlist))
            .order_by(PlaylistItem::pos.desc())
            .limit(1),
    )
    .first()
    .map(|i| i.pos)
    .unwrap_or(0)
}

// -------------------------------------------------------------------- queries

/// The whole library, in the order things were added — every kind, one list.
///
/// `ORDER BY` is explicit here as everywhere: SQLite's natural order is not a
/// contract, and two peers showing the same rows in different orders is a bug
/// that only appears on someone else's machine.
#[query]
pub fn library(db: &mut Db, playlist_id: Id<Playlist>) -> Result<Vec<Item>> {
    // An item *with* its favourite, which is a tree rather than a join: one
    // that is not favourited is still a row, carrying nothing. That is the LEFT
    // JOIN, and it is the relationship's shape rather than a keyword.
    //
    // One table, whatever the kind. A screen that lists the library never joins
    // a side table, which is the point of `media` carrying what a list needs.
    let rows = db.select_with(library_query(), Media::playlist_item, items_on(playlist_id));
    Ok(rows.iter().map(item_of).collect())
}

/// The query `library` answers, written once so that running it and maintaining
/// it cannot drift apart.
#[cfg(feature = "storage")]
fn library_query() -> petros_schema::Query<Media> {
    Media::all()
        .order_by(Media::pos.asc())
        .order_by(Media::id.asc())
}

/// One playlist's entries, as a child pipeline.
///
/// Filtered to a single playlist, so a media row carries at most the one entry
/// saying where it sits on it — which is what a heart draws. There is no
/// favourites table and no favourites verb: a heart means "this is on the
/// playlist I am showing you", and which playlist that is belongs to the
/// client, not to the domain.
#[cfg(feature = "storage")]
fn items_on(playlist: Id<Playlist>) -> petros_schema::Query<PlaylistItem> {
    PlaylistItem::all().filter(PlaylistItem::playlist_id.eq(playlist))
}

/// `library`, maintained rather than re-run.
///
/// A client holds one of these and hands it what each mutation changed, instead
/// of reading the whole list back on every frame. Same query, same rows, same
/// order — [`library_query`] is the single definition of all three.
#[cfg(feature = "storage")]
pub type LibraryView = petros::ivm::View<Media>;

/// Build one. Hydrate it once against a store, then feed it
/// `Client::take_changes()`.
#[cfg(feature = "storage")]
pub fn library_view(playlist: Id<Playlist>) -> LibraryView {
    petros::ivm::View::related(library_query(), Media::playlist_item, items_on(playlist))
}

/// Read a maintained view the way `library` reads a fetched one.
///
/// Every row, decoded. For the steady state prefer [`patch`], which touches
/// only the rows that moved — this one is for a fresh hydrate.
#[cfg(feature = "storage")]
pub fn items_of(view: &LibraryView) -> Vec<Item> {
    view.with::<PlaylistItem>().iter().map(item_of).collect()
}

/// How many songs are on the playlist, maintained.
///
/// A screen shows this beside the list and would otherwise recount it on every
/// frame. A tally holds the number rather than the rows, so it costs nothing at
/// any library size — and it is the *favourites* that are counted, so it reads
/// the playlist table rather than filtering songs.
#[cfg(feature = "storage")]
pub type PlaylistCount = petros::ivm::Tally;

#[cfg(feature = "storage")]
pub fn playlist_count(playlist: Id<Playlist>) -> PlaylistCount {
    petros::ivm::Tally::of(items_on(playlist))
}

/// Bring a list a screen holds up to date with what a view just did.
///
/// Maintaining the query and then decoding every row again is still O(n), and
/// past a few hundred songs that decode is most of what is left. These are the
/// entries that moved, in the order they moved, so the rest are not touched.
#[cfg(feature = "storage")]
pub fn patch(items: &mut Vec<Item>, patches: &[petros::ivm::Patch]) {
    for change in patches {
        match change {
            petros::ivm::Patch::Insert { at, node } => {
                if let Some(item) = node.decode::<Media, PlaylistItem>().as_ref().map(item_of) {
                    items.insert(*at, item);
                }
            }
            petros::ivm::Patch::Remove { at } => {
                items.remove(*at);
            }
            petros::ivm::Patch::Update { at, node } => {
                if let Some(item) = node.decode::<Media, PlaylistItem>().as_ref().map(item_of) {
                    items[*at] = item;
                }
            }
        }
    }
}

/// Every playlist, in the order they were made.
#[query]
pub fn playlists(db: &mut Db) -> Result<Vec<crate::schema::Playlist>> {
    Ok(db
        .select(
            Playlist::all()
                .order_by(Playlist::pos.asc())
                .order_by(Playlist::id.asc()),
        )
        .into_iter()
        .map(|p| crate::schema::Playlist {
            id: p.id,
            name: p.name,
            pos: p.pos,
            created_ms: p.created_ms,
            user_id: p.user_id,
        })
        .collect())
}

/// One playlist's contents, in playlist order.
#[query]
pub fn playlist(db: &mut Db, id: Id<Playlist>) -> Result<Vec<Item>> {
    // Read from the other end: favourites, each carrying its item. A favourite
    // whose item is gone carries nothing and is dropped, which is the INNER
    // JOIN — and `remove_song` deletes both, so it should not arise.
    let rows = db.select_with(
        items_on(id)
            .order_by(PlaylistItem::pos.asc())
            .order_by(PlaylistItem::media_id.asc()),
        PlaylistItem::media,
        Media::all(),
    );
    Ok(rows
        .iter()
        .filter_map(|r| {
            let media = r.one()?;
            Some(Item {
                id: media.id,
                kind: media.kind.clone(),
                title: media.title.clone(),
                creator: media.creator.clone(),
                duration_ms: media.duration_ms,
                file: media.file.clone(),
                pos: media.pos,
                added_ms: media.added_ms,
                user_id: media.user_id.clone(),
                playlist_pos: Some(r.row.pos),
            })
        })
        .collect())
}

/// One playable row and its place in the playlist, as a client reads it.
#[cfg(feature = "storage")]
fn item_of(row: &With<Media, PlaylistItem>) -> Item {
    Item {
        id: row.row.id,
        kind: row.row.kind.clone(),
        title: row.row.title.clone(),
        creator: row.row.creator.clone(),
        duration_ms: row.row.duration_ms,
        file: row.row.file.clone(),
        pos: row.row.pos,
        added_ms: row.row.added_ms,
        user_id: row.row.user_id.clone(),
        playlist_pos: row.one().map(|i| i.pos),
    }
}

peer!(
    add_song,
    create_playlist,
    add_to_playlist,
    add_all_to_playlist,
    remove_from_playlist,
    remove_media
);

// ---------------------------------------------------------- what a peer keeps

/// What the peer maintains beside its client.
///
/// `petros::foreign_peer!` hydrates this at open and settles it after every
/// mutation and every message from the server, so nothing here depends on a
/// caller remembering to ask.
#[cfg(feature = "storage")]
pub struct Views {
    library: LibraryView,
    count: PlaylistCount,
    /// Which playlist a heart means. Nil until a caller says, because the
    /// domain has no favourites of its own — a client chooses the playlist it
    /// is showing membership for.
    playlist: Id<Playlist>,
    /// What the library did since the caller last collected. A foreign caller
    /// polls rather than being pushed to, so this has to accumulate.
    pending: Vec<petros::ivm::Patch>,
    /// Set by a rebase, which no sequence of patches describes. The caller
    /// takes the whole list again and starts over.
    reset: bool,
}

#[cfg(feature = "storage")]
impl Views {
    /// Show membership of this playlist from now on.
    ///
    /// Rebuilds the maintained view against it and asks the caller to take the
    /// whole list again, because no sequence of patches turns one playlist's
    /// memberships into another's.
    pub fn use_playlist(&mut self, id: Id<Playlist>, store: &mut petros::backend::SqliteStore<'_>) {
        self.library = library_view(id);
        self.count = playlist_count(id);
        self.playlist = id;
        self.library.hydrate(store);
        self.count.hydrate(store);
        self.pending.clear();
        self.reset = true;
    }

    /// Which playlist a heart currently means.
    pub fn playlist(&self) -> Id<Playlist> {
        self.playlist
    }
}

#[cfg(feature = "storage")]
impl petros::Views for Views {
    fn build() -> Self {
        Views {
            library: library_view(Id::nil()),
            count: playlist_count(Id::nil()),
            playlist: Id::nil(),
            pending: Vec::new(),
            reset: true,
        }
    }

    fn hydrate(&mut self, store: &mut petros::backend::SqliteStore<'_>) {
        self.library.hydrate(store);
        self.count.hydrate(store);
        // Anything already collected describes a database that no longer
        // exists, and anything not yet collected describes one the caller never
        // saw. Both are wrong; the whole list is not.
        self.pending.clear();
        self.reset = true;
    }

    fn apply(
        &mut self,
        store: &mut petros::backend::SqliteStore<'_>,
        changes: &[petros_schema::Change],
    ) {
        self.count.apply(store, changes);
        let patches = self.library.apply(store, changes);
        self.pending.extend(patches);
    }
}

// ------------------------------------------------------- across the boundary

/// What the library did, as a foreign caller hears it.
///
/// The list used to cross whole on every change, which is the expensive half on
/// a phone: maintaining the query saves the SQL and the bridge charges for the
/// rows regardless. This carries the rows that moved.
///
/// `reset` is the rebase, and it is not a failure — it is the case no sequence
/// of patches can describe, because the optimistic view was rolled back and a
/// rollback reports nothing. Then `items` is the whole list and `patches` is
/// empty; otherwise `items` is empty and `patches` is what to splice.
#[cfg(feature = "foreign")]
#[derive(uniffi::Record)]
pub struct LibraryUpdate {
    pub reset: bool,
    pub items: Vec<crate::schema::foreign::Item>,
    pub patches: Vec<ItemPatch>,
    /// How many items are on the playlist being shown, maintained rather than
    /// counted on the far side.
    pub on_playlist: u32,
}

/// One entry of the list moving. Positions are valid in sequence: apply them in
/// order to a list that started equal and it ends equal.
#[cfg(feature = "foreign")]
#[derive(uniffi::Record)]
pub struct ItemPatch {
    pub op: PatchOp,
    pub at: u32,
    /// Absent for a removal, which needs only the position.
    pub item: Option<crate::schema::foreign::Item>,
}

#[cfg(feature = "foreign")]
#[derive(uniffi::Enum)]
pub enum PatchOp {
    Insert,
    Remove,
    Update,
}

#[cfg(feature = "foreign")]
impl Views {
    /// Collect what has happened since the last call, and start accumulating
    /// again.
    ///
    /// `foreign` rather than `storage`, so it builds the boundary record
    /// directly. It was a tuple of four things and a mapping step, because the
    /// storage build has no boundary types — but `foreign` implies `storage`,
    /// so the split bought nothing and cost a shape clippy was right to dislike.
    fn take_update(&mut self) -> LibraryUpdate {
        let on_playlist = self.count.get() as u32;
        if std::mem::take(&mut self.reset) {
            self.pending.clear();
            return LibraryUpdate {
                reset: true,
                items: items_of(&self.library)
                    .into_iter()
                    .map(Into::into)
                    .collect(),
                patches: Vec::new(),
                on_playlist,
            };
        }
        let item_of_node = |node: &petros_schema::Tree| {
            node.decode::<Media, PlaylistItem>()
                .as_ref()
                .map(item_of)
                .map(Into::into)
        };
        LibraryUpdate {
            reset: false,
            items: Vec::new(),
            patches: std::mem::take(&mut self.pending)
                .into_iter()
                .map(|patch| match patch {
                    petros::ivm::Patch::Insert { at, node } => ItemPatch {
                        op: PatchOp::Insert,
                        at: at as u32,
                        item: item_of_node(&node),
                    },
                    petros::ivm::Patch::Remove { at } => ItemPatch {
                        op: PatchOp::Remove,
                        at: at as u32,
                        item: None,
                    },
                    petros::ivm::Patch::Update { at, node } => ItemPatch {
                        op: PatchOp::Update,
                        at: at as u32,
                        item: item_of_node(&node),
                    },
                })
                .collect(),
            on_playlist,
        }
    }
}

#[cfg(feature = "foreign")]
#[uniffi::export]
impl Peer {
    /// What the library did since you last asked.
    ///
    /// Poll this after a mutation or a frame from the server. The peer keeps the
    /// view up to date on its own, so an empty answer means nothing moved and
    /// the screen need not redraw.
    pub fn library_update(&self) -> ::core::result::Result<LibraryUpdate, crate::PeerError> {
        self.views(|views| ::core::result::Result::Ok(views.take_update()))
    }

    /// Show membership of this playlist: what a heart in the library means.
    ///
    /// The domain has no favourites of its own, so a client says which playlist
    /// it is drawing hearts for. The next `library_update` is a reset, because
    /// no sequence of patches turns one playlist's memberships into another's.
    pub fn show_playlist(&self, id: String) -> ::core::result::Result<(), crate::PeerError> {
        let playlist: Id<Playlist> = id.parse().map_err(|_| crate::PeerError::Refused {
            reason: ::std::format!("{id} is not an id"),
        })?;
        self.views_with_store(|store, views| {
            views.use_playlist(playlist, store);
            ::core::result::Result::Ok(())
        })
    }
}
