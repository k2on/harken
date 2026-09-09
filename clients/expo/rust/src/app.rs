//! The Petros app whose `apply` is a wasm module — the phone's, and only the
//! phone's.
//!
//! This lives with the client rather than in `petros-wasm-host`, because it is
//! the one part of running a module that knows what domain is being run: the
//! payload type, the `App` impl, and the module bytes this build ships with.
//!
//! Every other peer links `todo` and calls `apply` directly; see
//! `docs/decisions.md` for why the indirection is confined to here. What it
//! buys is the thing only this peer wants: a domain it can replace over Metro,
//! or one day over the air, without a native build.
//!
//! `petros` calls `Mutation::apply` during a rebase and hands it no context of
//! ours, so the module lives in a process-wide slot and this looks it up. One
//! domain per process is the same assumption a linked `apply` makes; this only
//! makes it replaceable while the process runs.

use ciborium::value::Value;
use petros::{ActorId, App, AutoCtx, Mutation, MutationError, Transaction};
use serde::{Deserialize, Serialize};

use petros_wasm_host::MUTATORS;

/// The module this build was compiled against.
///
/// A peer with no Metro attached — the server, the terminal examples — wants
/// exactly this and nothing else, so it is baked in rather than found at
/// runtime. `just mutators` is what puts it there.
pub const BUNDLED: &[u8] =
    include_bytes!("../../../../target/wasm32-unknown-unknown/mutators/harken.wasm");

/// Install [`BUNDLED`]. What a peer with no Metro attached calls at startup.
pub fn load_bundled() -> Result<u64, String> {
    petros_wasm_host::load(BUNDLED)
}

/// One mutation, as the bytes the log stores.
///
/// Wire-identical to [`harken::Payload`] — both are `#[serde(transparent)]` over
/// the same CBOR value — and separate only because a crate may not implement a
/// foreign trait for a foreign type. `conformance.rs` checks they agree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Payload(pub Value);

impl From<harken::Payload> for Payload {
    fn from(p: harken::Payload) -> Self {
        Payload(p.0)
    }
}

impl Payload {
    fn bytes(&self) -> Result<Vec<u8>, MutationError> {
        let mut out = Vec::new();
        ciborium::into_writer(&self.0, &mut out)
            .map_err(|e| MutationError::rejected(format!("could not re-encode a mutation: {e}")))?;
        Ok(out)
    }
}

fn missing() -> MutationError {
    // Not a rejection about the mutation: "this peer has not loaded a module"
    // is a fact about this peer only. It travels as one because that is the
    // only channel `apply` has, which is a wart worth knowing about.
    MutationError::rejected("no mutator module is loaded")
}

impl Mutation for Payload {
    fn fill_auto(&mut self, ctx: &mut AutoCtx) {
        let Ok(guard) = MUTATORS.read() else { return };
        let Some(mutators) = guard.as_ref() else {
            return;
        };
        let Ok(payload) = self.bytes() else { return };
        if let Ok(filled) = mutators.fill_auto(&payload, ctx) {
            if let Ok(value) = ciborium::from_reader::<Value, _>(filled.as_slice()) {
                self.0 = value;
            }
        }
    }

    fn apply(&self, tx: &mut Transaction, actor: &ActorId) -> Result<(), MutationError> {
        let guard = MUTATORS.read().map_err(|_| missing())?;
        let mutators = guard.as_ref().ok_or_else(missing)?;
        let payload = self.bytes()?;
        match mutators.apply(tx.conn(), &payload, actor.as_str()) {
            // The module said no: a deterministic verdict, and the entry will
            // never be in the log.
            Ok(Err(reason)) => Err(MutationError::rejected(reason)),
            Ok(Ok(())) => Ok(()),
            // The host or the module broke — a trap, a bad query. That says
            // nothing about the mutation, so it must not be a rejection.
            Err(e) => Err(MutationError::rejected(format!("the mutator failed: {e}"))),
        }
    }
}

/// The app, for a peer whose `apply` arrives as a file.
pub struct WasmHarken;

impl App for WasmHarken {
    type Mutation = Payload;

    // One schema, and the linked crate's. Migrations are the one thing that
    // should not arrive over the air, so they stay where every peer can see
    // them — this peer replaces `apply`, not the shape of the database.
    const SCHEMA: &'static str = <harken::HarkenApp as App>::SCHEMA;
}

/// Author a mutation by name, through the same encoder every peer uses.
pub fn from_json(kind: &str, args_json: &str) -> Result<Payload, String> {
    harken::from_json(kind, args_json).map(Payload::from)
}
