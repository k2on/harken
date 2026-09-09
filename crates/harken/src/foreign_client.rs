//! The Harken client, exported for foreign callers through UniFFI.
//!
//! This is a wrapper and nothing else. Every mutation and every query below
//! forwards into [`todo`], which is where they are defined once — so the Expo
//! client, the terminal examples and the server all run the same `apply`. There
//! is no second implementation to keep in step, because there is no second
//! implementation.
//!
//! # What crosses the boundary
//!
//! The engine stays sans-io here too. [`TodoClient::take_outgoing`] hands back
//! encoded frames and [`TodoClient::recv`] takes them, so the caller owns the
//! socket and nothing else: no async, no callbacks, no runtime, no thread. On
//! React Native that caller is a `WebSocket`, the same shape `transport/web.rs`
//! already uses in a browser.
//!
//! # Generating the bindings
//!
//! There is no UDL file. The `#[uniffi::export]` attributes below are the
//! interface definition, and `uniffi-bindgen-react-native` reads the metadata
//! back out of the compiled library:
//!
//! ```text
//! just bindings
//! ```

use std::sync::Mutex;

pub use crate::wasm_app::WasmHarken;

use crate::foreign::Song;
use crate::{favorites, library};
use petros::{decode, encode, AutoCtx, Client, MutationError, ServerMsg};

/// A mutation the server refused. Not a failure: a deterministic verdict every
/// replica would have reached identically.
#[derive(Debug, Clone, uniffi::Record)]
pub struct HarkenRejection {
    pub id: String,
    pub reason: String,
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum HarkenError {
    /// The app itself said no — an empty to-do, say. The caller should show
    /// this to a person; retrying it unchanged will fail the same way.
    #[error("{reason}")]
    Refused { reason: String },
    /// Anything else: a broken database, a frame that will not decode. A bug
    /// or a broken environment, not a judgement about the mutation.
    #[error("{message}")]
    Engine { message: String },
}

impl From<petros::Error> for HarkenError {
    fn from(e: petros::Error) -> Self {
        match e {
            petros::Error::Mutation(MutationError::Rejected(reason)) => {
                HarkenError::Refused { reason }
            }
            other => HarkenError::Engine {
                message: other.to_string(),
            },
        }
    }
}

/// A peer of an Petros server.
///
/// `Client` owns a SQLite connection, which is `Send` but not `Sync`, and a
/// read needs `&mut` like a write does — so every method here takes the lock.
/// That is not a concession to the boundary: the iced client serialises access
/// the same way, because the optimistic savepoint means there is only ever one
/// coherent view to read.
#[derive(uniffi::Object)]
pub struct HarkenClient {
    inner: Mutex<Client<WasmHarken>>,
}

impl HarkenClient {
    fn with<T>(
        &self,
        f: impl FnOnce(&mut Client<WasmHarken>) -> Result<T, HarkenError>,
    ) -> Result<T, HarkenError> {
        let mut guard = self.inner.lock().map_err(|_| HarkenError::Engine {
            message: "the client lock was poisoned by an earlier panic".into(),
        })?;
        f(&mut guard)
    }
}

#[uniffi::export]
impl HarkenClient {
    /// Open the peer's database, running Petros's migrations and the app's.
    ///
    /// `db_path` is a file the caller owns — on React Native, somewhere under
    /// the app's documents directory. `actor` is who this peer is; it is opaque
    /// to Petros and ends up on every row this peer authors.
    #[uniffi::constructor]
    pub fn open(db_path: String, actor: String) -> Result<Self, HarkenError> {
        let conn = petros::open_path(&db_path)?;
        let client = Client::<WasmHarken>::open(conn, actor, AutoCtx::system())?;
        Ok(HarkenClient {
            inner: Mutex::new(client),
        })
    }

    // ------------------------------------------------------------ mutations

    /// Author any mutation the loaded module understands.
    ///
    /// The one entry point that does not grow when the domain does. `kind` is a
    /// variant name and `args` is a JSON object of its fields; the module
    /// decides what both mean, fills in whatever is auto-generated, and applies
    /// it. Adding a verb is therefore a module rebuild and a call site — neither
    /// of which needs a native build, which is the whole reason the domain is a
    /// wasm module rather than a linked symbol.
    ///
    /// The typed methods below are conveniences over exactly this. They exist
    /// because a call site reads better with names, not because the engine
    /// needs them.
    ///
    /// ```text
    /// mutate("MarkAllDone", "{}")
    /// mutate("Add", r#"{"text": "buy milk"}"#)
    /// mutate("SetDone", r#"{"id": "67e55084-...", "done": true}"#)
    /// ```
    pub fn mutate(&self, kind: String, args: String) -> Result<(), HarkenError> {
        let payload = crate::from_json(&kind, &args)
            .map_err(|message| HarkenError::Refused { reason: message })?;
        self.with(|c| {
            c.mutate(payload.into())?;
            Ok(())
        })
    }

    /// Put a song in the library.
    ///
    /// Refused if the title is blank — checked here, against the view the
    /// caller is actually looking at, so an intent that is invalid never
    /// reaches the pending queue or the wire.
    ///
    /// Returns nothing on purpose. The row's id is generated inside the module
    /// by `fill_auto` and belongs to the log, not to this call; read it from
    /// the next [`library`](Self::library), which is where every other caller
    /// gets it.
    pub fn add_song(&self, title: String, artist: String) -> Result<(), HarkenError> {
        self.mutate(
            "AddSong".into(),
            serde_json::json!({ "title": title, "artist": artist }).to_string(),
        )
    }

    /// The heart, on.
    pub fn favorite(&self, id: String) -> Result<(), HarkenError> {
        self.mutate(
            "Favorite".into(),
            serde_json::json!({ "id": id }).to_string(),
        )
    }

    /// The heart, off. The song stays in the library.
    pub fn unfavorite(&self, id: String) -> Result<(), HarkenError> {
        self.mutate(
            "Unfavorite".into(),
            serde_json::json!({ "id": id }).to_string(),
        )
    }

    pub fn remove_song(&self, id: String) -> Result<(), HarkenError> {
        self.mutate(
            "RemoveSong".into(),
            serde_json::json!({ "id": id }).to_string(),
        )
    }

    // -------------------------------------------------------------- queries

    /// The whole library: confirmed replayed, then this peer's pending
    /// mutations on top. Always ordered explicitly.
    pub fn library(&self) -> Result<Vec<Song>, HarkenError> {
        self.with(|c| Ok(library(c.conn())?.into_iter().map(Song::from).collect()))
    }

    /// Just the favourites, in playlist order.
    pub fn favorites(&self) -> Result<Vec<Song>, HarkenError> {
        self.with(|c| Ok(favorites(c.conn())?.into_iter().map(Song::from).collect()))
    }

    /// How much of the server's log has been applied.
    pub fn cursor(&self) -> u64 {
        self.inner.lock().map(|c| c.cursor()).unwrap_or(0)
    }

    /// How many of this peer's own mutations no server has confirmed yet.
    pub fn pending_len(&self) -> u32 {
        self.inner
            .lock()
            .map(|mut c| c.pending_len() as u32)
            .unwrap_or(0)
    }

    // ---------------------------------------------------------- hot reload

    /// Install a mutator module, replacing whatever was running.
    ///
    /// This is the whole hot-reload path on this side: Metro pushes the new
    /// bytes, the app calls this, and the next mutation runs the new `apply`.
    /// Returns the generation, which moves on every successful swap.
    ///
    /// A module that does not export the ABI is rejected here rather than at
    /// the first mutation, so a bad push fails loudly and the old one keeps
    /// running.
    pub fn load_mutators(&self, wasm: Vec<u8>) -> Result<u64, HarkenError> {
        petros_wasm_host::load(&wasm).map_err(|message| HarkenError::Engine { message })
    }

    /// Which module is running, or zero if none has been installed yet.
    pub fn mutators_generation(&self) -> u64 {
        petros_wasm_host::generation()
    }

    // ---------------------------------------------------------- the transport

    /// Announce a fresh connection: ask for everything since our cursor and
    /// re-offer everything still pending. Safe to repeat.
    pub fn connected(&self) -> Result<(), HarkenError> {
        self.with(|c| Ok(c.connected()?))
    }

    /// Drain the outbox as encoded frames, ready to put on a socket.
    ///
    /// A caller with no connection should drain and discard rather than let the
    /// queue grow: reconnecting re-offers everything still pending, and the
    /// server dedupes what it has already seen.
    pub fn take_outgoing(&self) -> Result<Vec<Vec<u8>>, HarkenError> {
        self.with(|c| {
            c.take_outgoing()
                .iter()
                .map(|msg| encode(msg).map_err(HarkenError::from))
                .collect()
        })
    }

    /// Hand one frame from the server to the engine.
    pub fn recv(&self, frame: Vec<u8>) -> Result<(), HarkenError> {
        let msg: ServerMsg<crate::wasm_app::Payload> = decode(&frame)?;
        self.with(|c| Ok(c.recv(msg)?))
    }

    /// Drain mutations the server refused.
    pub fn take_rejections(&self) -> Vec<HarkenRejection> {
        let Ok(mut client) = self.inner.lock() else {
            return Vec::new();
        };
        client
            .take_rejections()
            .into_iter()
            .map(|r| HarkenRejection {
                id: r.id.to_string(),
                reason: r.reason,
            })
            .collect()
    }
}
