# harken

A self-hosted, local-first music system, and a **Petros app** — the sync engine
lives next door in `../petros` and arrives as a git dependency, patched back to
that working copy by a gitignored `.cargo/config.toml`.

Two clients, `clients/iced` and `clients/expo`, and one server. All three run
the same `apply`: the first two link it, the phone loads it as a module.

Three packages. One domain crate, one server, one desktop client — and the
phone's native half is a *feature* of the domain crate rather than a package,
because everything it exports was already defined there.

The domain is songs and a favourites playlist. Favourites is a real ordered
playlist rather than a flag, so "add to favourites" reads `MAX(pos) + 1` — which
is what makes the rebase visible: heart something while offline and it lands
after whatever arrived while you were away.

## Layout

```
crates/harken/           the domain — the ONLY apply
  schema.sql             the one description of the tables. `migrate` runs it,
                         `tables!` generates the row types from it, and its
                         foreign keys generate the relationships between them
  src/schema.rs          the model: the tables, and the view a client reads
  src/functions.rs       every mutation and every query, one definition each
  tests/conformance.rs   the native and wasm builds of `apply`, compared
  tests/converge.rs      the domain against a simulated fleet
  tests/read_model.rs    library() and favorites() against rows apply wrote
  src/lib.rs             …and, under `cfg(wasm32)`, the module's ABI
  src/foreign_client.rs  the client a foreign caller sees (feature `foreign`)
  src/wasm_app.rs        the App whose `apply` is a module (feature `foreign`)
crates/server/           axum, with one Petros handler mounted on it
clients/iced/            the desktop and browser client
  src/heart.rs           the heart, drawn as a path (see below)
  web/                   the browser shell `just web` serves
clients/expo/            the phone client; src/ is UI and a socket, nothing else
  modules/harken-native/ the turbo module — generated, gitignored, not authored
scripts/mutators.sh      builds the module and its TypeScript types
docs/decisions.md        what is true because this ships to a phone
```

The engine's own decisions — the rebase, the log, the wasm ABI, the schema
macro — are in `../petros/docs/decisions.md`. Read that first.

## Running it

```
just              # fmt, lint, test
nix build .#harken-server   # …and .#harken-iced, .#harken-web
just latency      # the measurements: fsync, the sandbox, the maintained view
just mutators     # rebuild the domain module and hand it to Metro (~0.35s)
just mutators-watch # …on every save. Leave it running beside `bun start`.
just serve        # the sync server…
just iced alice   # …a desktop peer…
just iced bob     # …and another, to watch them sync
just web          # …a browser peer, at localhost:8080
just bindings     # regenerate the Expo client's TS from crates/harken
just expo-android # …and a phone. Needs `nix develop .#android`.
```

`just mutators` runs the generator out of `../petros`, so that repository has to
be checked out beside this one.

## Never write domain logic in TypeScript

Every mutation and every query is in `crates/harken/src/functions.rs`, written
once, as an ordinary Rust function:

```rust
/// Put a song in the library.
#[mutation]
pub fn add_song(db: &mut Db, id: NewId, added_ms: Now, actor: Actor,
                title: String, artist: String) -> Result { … }
```

Which parameters the engine supplies is decided by *type*: `&mut Db` is the
store, `NewId` and `Now` are the only non-determinism a mutation gets — chosen
once at the originating client and frozen in the log — and `Actor` is who
authored the entry. Everything after them is a caller's argument, and appears in
the authoring function, the schema the module carries, and the generated
TypeScript. `peer!` at the bottom of the file wires dispatch.

The server and the iced peer link these; the phone runs the same source compiled
to wasm and interpreted by `petros-wasm-host`, because that is the only peer
where a rebuild costs four minutes instead of a third of a second.
`tests/conformance.rs` runs every verb through both builds and compares rows and
refusals, so the two cannot drift.

**There is no SQL in it, and no ORM.** Reads and writes are the same shape —
`db.select(query)` and `db.put(&row)` — over row types `tables!` generates by
asking SQLite what is in `schema.sql`. Rename a column there and every call site
using it stops compiling, reads as well as writes, even though this runs inside
a sandbox that has no SQLite in it.

A join is not a keyword either. `REFERENCES song(id)` in the DDL generates
`Song::favorite` and `Favorite::song`, and a read through one of them returns a
*tree* — a song with its favourites hanging off it — rather than a flat product.
Which direction you read decides whether a childless parent survives, so
`library()` gets the LEFT JOIN and `favorites()` the INNER one without either
word appearing.

The cost is that a bulk mutation is a loop: `favorite_all` was one
`INSERT ... SELECT` with a window function and is now a write per song, because
a statement that inserts a thousand rows reports one result and not which rows
they were — which is exactly what an incremental view cannot work from.
`just latency` measures it. A thousand songs is 35ms natively and 73ms through
the sandbox; a hundred is 2ms and 16ms.

Petros still uses Diesel internally; that is the engine's business. An app
declares `App::SCHEMA` and never names a database library.

Changing a mutation does **not** need a native build: `just mutators` rebuilds
the module and rewrites the base64 `.ts` Metro pushes. Changing the *engine*
does, and that is what EAS is for.

`scripts/mutators.sh` is the one thing that builds it, and `just mutators`, the
EAS hook and CI all call it — only one of them has `just`. The single difference
between them is where `petros-codegen` comes from: the checkout beside this one
when there is one, so an engine edit needs no commit, and the published branch
otherwise, because a build container has no sibling directory.

## The bindings are generated, and are a feature rather than a package

`petros::foreign_peer!` in `lib.rs` generates the peer a foreign caller talks
to — the object, the errors, the engine methods, and the `App` whose `apply`
arrives as a module. Every named method comes from the function it belongs to:
`#[mutation]` and `#[query]` each emit their own `#[uniffi::export] impl Peer`
block, and the doc comment written once in `functions.rs` reaches the generated
TypeScript.

`ubrn.config.yaml` builds this crate with `cargoExtras: [--features, foreign]`.
Off by default, so `cargo tree -e normal` finds no uniffi and no wasmi in the
desktop client, the server, or the wasm module.

There are two `Song` structs — the row, and `foreign::Song` for the boundary —
because `Song.id` is a `petros::Id` and UniFFI cannot be taught a foreign type
without `impl FfiConverter for Id`, which the orphan rule refuses. There is one
declaration: `petros_schema::row!` in `schema.rs` emits both, the `From`
between them, and the `#[cfg]` on the far half. Neither file mentions bindings;
the doc comments reach the generated TypeScript.

## The heart is a path on the desktop and a character on the phone

Fira Sans, which iced embeds, has no U+2665, U+2661 or U+2764 in its cmap — a
text heart lays out fine and draws nothing at all. So `clients/iced/src/heart.rs` draws
it with two cubics down each side, filled when the song is on the playlist and
stroked when it is not. React Native uses the system font, which has the glyph,
so the Expo screen just writes `♥`.

## Traps in the client toolchain

- **The NDK is x86_64-only.** Google publishes no aarch64-linux host toolchain,
  so on an ARM Linux box the NDK's `clang` cannot execute. Build Android on
  x86_64 or in CI; iOS needs Xcode, so a macOS runner.
- **nix does not supply the Android SDK**, on purpose: gradle installs missing
  components into the SDK directory and the store is read-only. Bring your own
  and export `ANDROID_HOME`; `nix develop .#android` adds `cargo-ndk` and a JDK.
- **Expo Go cannot load this app.** It calls into Rust, so it needs a
  development build. `ios/` and `android/` are generated by `expo prebuild`.
- **`uniffi` is pinned to `=0.31`** because `uniffi-bindgen-react-native` pins
  it. The generator and the runtime must agree on the metadata format.
- **`nix develop` sets `TMPDIR`.** The demo databases go to
  `std::env::temp_dir()`, so a server started inside `nix develop --command`
  does not share a database with one started under direnv.

## The desktop client maintains its list; the phone does not yet

`clients/iced` holds a `petros::ivm::View`, hydrates it once at boot, and
splices its `Vec<Song>` from the patches the view reports. A tap costs the rows
that moved rather than the whole library — flat, where re-reading grew:
`just latency` prints the numbers and `docs/decisions.md` explains them.

The Expo client still calls `library()` on every change, and that is deliberate
rather than unfinished. Its cost is the list crossing the UniFFI bridge, not the
query, so maintaining the query alone would save the cheap half. The engine side
is ready — a module's writes report what they changed — but the bridge has not
been measured on a device, and designing for it unmeasured is how the desktop's
O(n) decode was missed the first time.

## Verified on a device: sub-10ms mutations, held under spamming

Android builds end to end and a mutation stays under ten milliseconds however
many are made in a row. That is the number every performance decision in
`../petros/docs/decisions.md` was aiming at, and it is worth writing down where
it came from, because it was three separate problems:

- **200–300ms** was `synchronous` defaulting to FULL. Every local write commits
  its intent durably on its own, and in WAL mode that fsynced on each one.
- **390ms at forty pending** was a thread spawned and a wasm module instantiated
  per apply. One worker thread and one instance for the module's life now.
- **Growing with the backlog** was the optimistic view being rolled back and
  replayed on every mutation. Intents live in their own file and a mutation
  applies forward into an open savepoint, so the cost stopped depending on how
  much is pending — which is what "fast offline indefinitely" actually requires.

Holding under spamming is the part that matters. A fast first tap is easy; a
tap that costs the same as the four hundredth is the property.

## Not verified

iOS has never been built from this repository — no machine here can run the
toolchain — so the `ios` job in `.github/workflows/expo.yml` is written but has
not had a green run. It runs on `macos-15`, which has Xcode; nothing suggests it
fails, only that nobody has watched it pass.

The JavaScript half of the bridge is unmeasured too. What crosses is measured —
`just latency` prints it, and `docs/decisions.md` explains why it is seventy
bytes rather than seventy kilobytes — but what React Native then does with those
values on a device is not.

Verified another way, because the local `.cargo/config.toml` hides it: with the
patch moved aside, the whole suite builds and passes against the *pinned* engine
from git, which is what CI and EAS actually resolve.
