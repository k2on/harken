//! The music library: the tables, the rows, and the mutations that produce
//! them.
//!
//! [`domain`] holds `apply` and `fill_auto` and knows nothing about where it
//! runs. [`storage`] gives it a store backed by a real SQLite connection, which
//! is what the server and the iced client use — they are ordinary Rust programs
//! and a mutation is an ordinary function call.
//!
//! The phone is the exception. It loads the same domain compiled to wasm
//! (`crates/harken-wasm`) so a new mutation reaches it over Metro without a
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
