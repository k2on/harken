//! The music library, in two files.
//!
//! [`schema`] is the model: what a row is, and what the tables are.
//! [`functions`] is every mutation and every query, each written once as an
//! ordinary Rust function. Everything else on this page is generated from them.
//!
//! The server and the iced client link these and call them directly. The phone
//! is the exception: it loads the same functions compiled to wasm, so a new
//! mutation reaches it over Metro without a native build. Two builds of one
//! source, held to that by `tests/conformance.rs`.
//!
//! The `storage` feature is what the wasm build turns off. It has no SQLite of
//! its own, only a channel to the host's, so it wants the mutations and none of
//! the rest — not the read model, not the engine, not the rows.

pub mod functions;
/// The listening session: one account, one thing playing, however many devices
/// are watching it.
///
/// Wire types and nothing else — no row is written here and none is read. It
/// is in this crate because this crate is the only vocabulary the server and
/// the clients already share, and behind `storage` because the module the
/// phone loads shares none of it.
#[cfg(feature = "storage")]
pub mod listening;
/// The model: the tables, and the view a client reads.
///
/// The tables are here in every build, because a mutation writes rows and the
/// sandbox runs mutations. The view is not — reading one is something only a
/// client does.
pub mod schema;

pub use functions::*;
#[cfg(feature = "storage")]
pub use schema::*;

/// An id, tagged with what it identifies: `Id<tables::Media>` is not an
/// `Id<tables::Playlist>` and will not be passed as one.
pub use petros_schema::Id;

// The payload, its `Mutation` impl and the `App`. Behind `storage` because they
// name the engine, and the wasm build has no engine — only a channel to one.
#[cfg(feature = "storage")]
petros::app!(HarkenApp {
    schema: crate::schema::SCHEMA,
    // 4 is a shape change and the largest one so far: `artist` became `person`,
    // `work`, `movement`, `recording` and `credit` arrived, and `song` gave up
    // `part`, `catalogue` and `performer` to them — none of which were facts
    // about a track. `migrate` runs `CREATE TABLE IF NOT EXISTS` and so does
    // nothing at all to a `song` that is already there, which is exactly what
    // this number is for: drop, recreate at the new shape, replay every entry.
    //
    // Nothing in the *log* was removed to do it. `AddSong` still carries
    // `part`, `catalogue` and `performer`, because a log is permanent and an
    // argument can never be withdrawn — what changed is where `apply` puts
    // them. An entry written before any of this replays as the work it always
    // described; `add_song` says how, and that reading is the reason the demo's
    // library grows works without one line of its seed moving.
    //
    // 3 was not a shape change — `playlist` has exactly the
    // columns it had. `create_playlist` changed its *meaning*: a name a person
    // already has is now a no-op, because every client makes a default
    // playlist before it has seen the log and a person on three devices ended
    // up with three "Favorites".
    //
    // An app's tables are a function of the log, and this changed the
    // function. Nothing rebuilds without being told to, so a peer that already
    // has the duplicates would keep them for ever while a fresh install
    // replayed the same log into one row — the two disagreeing about a
    // database neither of them is wrong about. Bumping this is what makes them
    // agree: drop, recreate, replay, and the first entry for each name wins
    // everywhere.
    //
    // The cost is one replay per peer, once, bounded by the log's length. The
    // phone does not pay it and does not get it either: `ForeignApp` leaves
    // `SCHEMA_VERSION` at 0 on purpose — a migration is the one thing that
    // should not arrive over the air — so a phone with duplicates keeps them
    // until it is reinstalled.
    //
    // 2 was `song` growing the columns that say where a track sits in its
    // album: the number, the part, the catalogue, who played it, the tempo.
    schema_version: 4,
    apply: crate::functions::apply,
    fill_auto: crate::functions::fill_auto,
});

// The wasm ABI: `apply` and `fill_auto` behind the entry points the interpreter
// calls, and a store made of imported functions.
//
// Only in the module build, and it is the whole of what a separate
// `harken-wasm` crate used to be. A separate crate bought nothing: what keeps
// SQLite and the engine out of the module is `--no-default-features`, a flag
// on the build rather than a property of a package.
//
// `wasm32` alone is not the condition: the browser client is wasm32 too, with
// `storage` on, and linking this there imports the host's functions from a
// wasm module named `petros` — which wasm-bindgen renders as
// `import … from "petros"` and the browser refuses as a bare specifier
// before a line of the app runs.
//
// `//` and not `///` — a doc comment cannot attach to a macro invocation, and
// the warning for that only appears on the one target this is compiled for.
#[cfg(all(target_arch = "wasm32", not(feature = "storage")))]
petros_wasm_guest::export!(functions);

// At the crate root because that is where it defines `UniFfiTag`, which every
// `#[derive(uniffi::…)]` in this crate resolves against.
#[cfg(feature = "foreign")]
uniffi::setup_scaffolding!();

// The peer a foreign caller talks to: the same functions, reached over UniFFI,
// with `apply` arriving as a module rather than linked. Everything but the
// module itself is generated — the verbs and queries by their own attributes,
// the rest by this.
#[cfg(feature = "foreign")]
petros::foreign_peer!(Peer {
    schema: crate::schema::SCHEMA,
    module: include_bytes!("../../target/wasm32-unknown-unknown/mutators/harken.wasm"),
    // The peer maintains the library rather than re-reading it, and settles it
    // after every mutation and every frame from the server. `library_update`
    // then carries only what moved.
    views: crate::functions::Views,
});
