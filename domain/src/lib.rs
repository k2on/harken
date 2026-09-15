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
    // 2, because `song` grew the columns that say where a track sits in its
    // album — the number, the part, the catalogue, who played it, the tempo.
    // Every peer rebuilds its tables from the log on the next open; the log
    // itself did not move, and entries written before those arguments existed
    // replay with them at their defaults.
    schema_version: 2,
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
