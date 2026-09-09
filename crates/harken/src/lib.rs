//! The music library: the tables, the rows, and the mutations that produce
//! them.
//!
//! [`domain`] holds `apply` and `fill_auto` and knows nothing about where it
//! runs. [`storage`] gives it a store backed by a real SQLite connection, which
//! is what the server and the iced client use — they are ordinary Rust programs
//! and a mutation is an ordinary function call.
//!
//! The phone is the exception. It loads the same domain compiled to wasm
//! so a new mutation reaches it over Metro without a
//! native build. Two builds of one source, held to that by
//! `tests/conformance.rs`, which runs the same mutations through both and
//! compares the rows.
//!
//! The `storage` feature is what the wasm build turns off: it has no SQLite of
//! its own, only a channel to the host's, so it wants the domain and none of
//! this.

pub mod domain;

#[cfg(feature = "storage")]
mod storage;
#[cfg(feature = "storage")]
pub use storage::*;

// The phone's client: the same domain, reached over UniFFI, with `apply`
// arriving as a module rather than linked. Behind a feature because nothing
// else wants uniffi in its graph — and off by default, so the desktop client
// and the server never build it.
// At the crate root because that is where it defines `UniFfiTag`, which every
// `#[derive(uniffi::…)]` in this crate resolves against.
#[cfg(feature = "ffi")]
uniffi::setup_scaffolding!();

#[cfg(feature = "ffi")]
pub mod ffi;
#[cfg(feature = "ffi")]
pub mod wasm_app;

// The wasm ABI: `apply` and `fill_auto` behind the entry points the interpreter
// calls, and a store made of imported functions.
//
// Only on wasm, and it is the whole of what a separate `harken-wasm` crate used
// to be. A separate crate bought nothing: what keeps SQLite and the engine out
// of the module is `--no-default-features`, a flag on the build rather than a
// property of a package.
//
// `//` and not `///` — a doc comment cannot attach to a macro invocation, and
// the warning for that only appears on the one target this is compiled for.
#[cfg(target_arch = "wasm32")]
petros_wasm_guest::export!(domain, domain::SCHEMA_TEXT);
