//! What the server is made of, beside the binary that starts it.
//!
//! Two things are in here, and they are here rather than in `main.rs` for one
//! reason: a binary has no library target, so nothing can test it. These are
//! the parts of this server with behaviour worth asserting — a directory
//! becomes songs and a rescan adds none; a command pauses the device that is
//! actually playing rather than the one that asked — so each is a module a
//! test can reach.
//!
//! Everything else the server does is wiring: mounting a handler, reading an
//! environment variable, printing a line. That stays in `main.rs`.

pub mod assistant;
pub mod library;
pub mod listening;
