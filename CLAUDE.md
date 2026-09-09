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
                         and petros-sql checks every statement against it
  domain.rs              `mutations!` — verbs, arguments and bodies in one
                         declaration; plus fill_auto
  storage.rs             the read model — checked SQL, same as the writes
  tests/conformance.rs   the native and wasm builds of `apply`, compared
  tests/converge.rs      the domain against a simulated fleet
  tests/read_model.rs    library() and favorites() against rows apply wrote
  src/lib.rs             …and, under `cfg(wasm32)`, the module's ABI
  src/ffi.rs             the phone's client over UniFFI  (feature `ffi`)
  src/wasm_app.rs        the App whose `apply` is a module (feature `ffi`)
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
just mutators     # rebuild the domain module and hand it to Metro (~0.35s)
just mutators-watch # …on every save. Leave it running beside `bun start`.
just serve        # the sync server…
just iced alice   # …a desktop peer…
just iced bob     # …and another, to watch them sync
just web          # …a browser peer, at localhost:8080
just ffi-bindings # regenerate the Expo client's TS from crates/harken
just expo-android # …and a phone. Needs `nix develop .#android`.
```

`just mutators` runs the generator out of `../petros`, so that repository has to
be checked out beside this one.

## Never write domain logic in TypeScript

`apply` is in `crates/harken/src/domain.rs`. The server and the iced peer link
it; the phone runs the same source compiled to wasm and interpreted by
`petros-wasm-host`, because that is the only peer where a rebuild costs four
minutes instead of a third of a second. `tests/conformance.rs` runs every verb
through both builds and compares rows and refusals, so the two cannot drift.

The SQL in it is real SQL and it is checked: `petros_sql::exec!` and
`petros_sql::query!` prepare each statement against `schema.sql` at build time,
with SQLite as the judge. Rename a column there and every call site using it
stops compiling — the reads in `storage.rs` as well as the writes in
`domain.rs`. That is why `FavoriteAll` can be one `INSERT ... SELECT` with a
window function while still running inside a sandbox that has no SQLite in it.

**There is no ORM here, and no `diesel` anywhere in this repository.** There was
one for reads only, which meant the schema was described twice — as `table!` and
as DDL — and checked two different ways, neither of which could reach `apply`.
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

## The FFI is a feature, not a package

`src/ffi.rs` exports the phone's client, and `ubrn.config.yaml` builds this crate
with `cargoExtras: [--features, ffi]`. Off by default, so `cargo tree -e normal`
finds no uniffi and no wasmi in the desktop client, the server, or the module.

One duplicate survives and cannot be removed: `ffi::FfiSong` is a near-copy of
`Song`, because `Song.id` is a `petros::Id` and teaching UniFFI to carry it needs
`impl FfiConverter for Id` — a foreign trait on a foreign type, which the orphan
rule refuses. The cost is one `From` impl the compiler checks: add a field to
`Song` and it stops compiling until the field is carried across.

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

## Not verified

Android and iOS have never been built end to end from this repository — no
machine here can run either toolchain — so `.github/workflows/expo.yml` is
written but has not had a green run. What *is* verified is everything up to that
line: the Rust builds, the bindings generate, and the app typechecks.
