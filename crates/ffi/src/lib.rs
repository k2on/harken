//! The to-do client, exported for foreign callers through UniFFI.
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
//! just ffi-bindings
//! ```

use std::sync::Mutex;

use exo::{decode, encode, AutoCtx, Client, Id, MutationError, ServerMsg};
use todo::{list, Todo, TodoApp};

uniffi::setup_scaffolding!();

/// One row of the materialised view.
#[derive(Debug, Clone, uniffi::Record)]
pub struct TodoItem {
    /// The canonical 8-4-4-4-12 form. Sixteen bytes on the wire and in SQLite;
    /// a string here because that is what a foreign caller can hold, compare
    /// and use as a list key.
    pub id: String,
    pub text: String,
    pub done: bool,
    /// Recomputed on every replay from `MAX(pos) + 1`, which is what makes the
    /// rebase visible: an item added while offline moves down as confirmed
    /// entries land underneath it.
    pub pos: i64,
    pub created_ms: i64,
    pub actor: String,
}

impl From<todo::Item> for TodoItem {
    fn from(item: todo::Item) -> Self {
        TodoItem {
            id: item.id.to_string(),
            text: item.text,
            done: item.done,
            pos: item.pos,
            created_ms: item.created_ms,
            actor: item.actor,
        }
    }
}

/// A mutation the server refused. Not a failure: a deterministic verdict every
/// replica would have reached identically.
#[derive(Debug, Clone, uniffi::Record)]
pub struct TodoRejection {
    pub id: String,
    pub reason: String,
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum TodoError {
    /// The app itself said no — an empty to-do, say. The caller should show
    /// this to a person; retrying it unchanged will fail the same way.
    #[error("{reason}")]
    Refused { reason: String },
    /// Anything else: a broken database, a frame that will not decode. A bug
    /// or a broken environment, not a judgement about the mutation.
    #[error("{message}")]
    Engine { message: String },
}

impl From<exo::Error> for TodoError {
    fn from(e: exo::Error) -> Self {
        match e {
            exo::Error::Mutation(MutationError::Rejected(reason)) => TodoError::Refused { reason },
            other => TodoError::Engine {
                message: other.to_string(),
            },
        }
    }
}

fn parse_id(id: &str) -> Result<Id, TodoError> {
    exo::uuid::Uuid::parse_str(id)
        .map(Id::from)
        .map_err(|e| TodoError::Engine {
            message: format!("not an id: {e}"),
        })
}

/// A peer of an Exo server.
///
/// `Client` owns a SQLite connection, which is `Send` but not `Sync`, and
/// Diesel needs `&mut` even to read — so every method here takes the lock. That
/// is not a concession to the FFI: the Rust examples serialise access the same
/// way, because the optimistic savepoint means there is only ever one coherent
/// view to read.
#[derive(uniffi::Object)]
pub struct TodoClient {
    inner: Mutex<Client<TodoApp>>,
}

impl TodoClient {
    fn with<T>(
        &self,
        f: impl FnOnce(&mut Client<TodoApp>) -> Result<T, TodoError>,
    ) -> Result<T, TodoError> {
        let mut guard = self.inner.lock().map_err(|_| TodoError::Engine {
            message: "the client lock was poisoned by an earlier panic".into(),
        })?;
        f(&mut guard)
    }
}

#[uniffi::export]
impl TodoClient {
    /// Open the peer's database, running Exo's migrations and the app's.
    ///
    /// `db_path` is a file the caller owns — on React Native, somewhere under
    /// the app's documents directory. `actor` is who this peer is; it is opaque
    /// to Exo and ends up on every row this peer authors.
    #[uniffi::constructor]
    pub fn open(db_path: String, actor: String) -> Result<Self, TodoError> {
        let conn = exo::open_path(&db_path)?;
        let client = Client::<TodoApp>::open(conn, actor, AutoCtx::system())?;
        Ok(TodoClient {
            inner: Mutex::new(client),
        })
    }

    // ------------------------------------------------------------ mutations

    /// Add a to-do. Returns the new entry's id.
    ///
    /// Refused if the text is blank — checked here, against the view the caller
    /// is actually looking at, so an intent that is invalid never reaches the
    /// pending queue or the wire.
    pub fn add(&self, text: String) -> Result<String, TodoError> {
        self.with(|c| Ok(c.mutate(Todo::add(&text))?.to_string()))
    }

    pub fn set_done(&self, id: String, done: bool) -> Result<(), TodoError> {
        let id = parse_id(&id)?;
        self.with(|c| {
            c.mutate(Todo::SetDone { id, done })?;
            Ok(())
        })
    }

    pub fn remove(&self, id: String) -> Result<(), TodoError> {
        let id = parse_id(&id)?;
        self.with(|c| {
            c.mutate(Todo::Remove { id })?;
            Ok(())
        })
    }

    // -------------------------------------------------------------- queries

    /// The materialised view: confirmed replayed, then this peer's pending
    /// mutations on top. Always ordered explicitly.
    pub fn list(&self) -> Result<Vec<TodoItem>, TodoError> {
        self.with(|c| Ok(list(c.conn())?.into_iter().map(TodoItem::from).collect()))
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

    // ---------------------------------------------------------- the transport

    /// Announce a fresh connection: ask for everything since our cursor and
    /// re-offer everything still pending. Safe to repeat.
    pub fn connected(&self) -> Result<(), TodoError> {
        self.with(|c| Ok(c.connected()?))
    }

    /// Drain the outbox as encoded frames, ready to put on a socket.
    ///
    /// A caller with no connection should drain and discard rather than let the
    /// queue grow: reconnecting re-offers everything still pending, and the
    /// server dedupes what it has already seen.
    pub fn take_outgoing(&self) -> Result<Vec<Vec<u8>>, TodoError> {
        self.with(|c| {
            c.take_outgoing()
                .iter()
                .map(|msg| encode(msg).map_err(TodoError::from))
                .collect()
        })
    }

    /// Hand one frame from the server to the engine.
    pub fn recv(&self, frame: Vec<u8>) -> Result<(), TodoError> {
        let msg: ServerMsg<Todo> = decode(&frame)?;
        self.with(|c| Ok(c.recv(msg)?))
    }

    /// Drain mutations the server refused.
    pub fn take_rejections(&self) -> Vec<TodoRejection> {
        let Ok(mut client) = self.inner.lock() else {
            return Vec::new();
        };
        client
            .take_rejections()
            .into_iter()
            .map(|r| TodoRejection {
                id: r.id.to_string(),
                reason: r.reason,
            })
            .collect()
    }
}
