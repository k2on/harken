//! What the server is made of, beside the binary that starts it.
//!
//! There is one thing in here, and it is here rather than in `main.rs` for one
//! reason: a binary has no library target, so nothing can test it. The scanner
//! is the part of this server with behaviour worth asserting — a directory
//! becomes songs, a rescan adds none — so it is a module a test can reach.
//!
//! Everything else the server does is wiring: mounting a handler, reading an
//! environment variable, printing a line. That stays in `main.rs`.

pub mod library;
