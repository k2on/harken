# Decisions

A short ADR log. One paragraph per decision, newest last. These are the choices
that are not obvious from reading the code.

Harken is a Petros app. Everything about the sync engine itself — the rebase,
the log, the wasm ABI, the schema macro — is recorded in `../petros/docs/
decisions.md`, and this file holds only what is true because this app ships to
a phone.

## The Expo client calls Rust; it does not reimplement it

The obvious way to put a to-do list on a phone is to write one in TypeScript and
teach it the wire format. It was tried here first, and it was wrong: two
`apply`s in two languages is two definitions of what a mutation *means*, and the
first time they disagree — about `MAX(pos) + 1`, about whether a trimmed empty
string is refused, about what happens to an edit whose row a confirmed entry
removed — the replicas diverge silently and neither side is obviously at fault.
Every invariant at the top of `CLAUDE.md` is a property of one implementation,
not of two that intend to match.

So `crates/ffi` exports the client over UniFFI and
`uniffi-bindgen-react-native` generates the TypeScript. The generated files are
gitignored rather than committed, so nobody can hand-edit them and wonder why
the next build reverts it, and `just ffi-bindings` regenerates and then runs
`tsc` — which turns "the app still calls the API the Rust used to have" into a
compile error instead of a crash on a device.

There is no UDL file. `#[uniffi::export]` on the Rust *is* the interface
definition; the generator reads the metadata back out of the compiled library.
One place to change, and it is the place that already had to be right.

## The socket stayed in JavaScript

`crates/ffi` exposes the sans-io client and nothing else: `take_outgoing()`
hands back encoded frames, `recv()` takes them, and the caller owns the
transport. React Native then does what a browser does in `transport/web.rs`,
for the same reason it did there — the platform already has a WebSocket.

The alternative was to run `petros::transport::ws` on a thread inside the FFI and
hand JavaScript nothing but a `connect(url)`. It would have been thinner at the
call site and worse everywhere else: a `tungstenite` and a TLS stack in the
mobile binary, a uniffi callback interface to push changes back up, a thread to
manage across backgrounding, and `wss://` reimplemented next to the platform
trust store that already does it. Frames here are tens of bytes a few times a
second; React Native base64s binary frames across its bridge, and at this volume
that is not where the time goes. If it ever is — a media sync, say — the
transport is a page of code and moving it is a local change, which is what
sans-io bought in the first place.

## Native projects are generated, not committed

The Expo app uses Continuous Native Generation: there is no `ios/` or
`android/` in the tree, `expo prebuild` makes them, and the turbo module is a
workspace package React Native autolinks. This is why the client cannot run in
Expo Go — Expo Go ships a fixed set of native modules and ours is not one of
them — and a development build is not a limitation to work around but the
consequence of calling into Rust at all.

## Where each platform can be built, and where it cannot

Google publishes the NDK as prebuilt binaries for `linux-x86_64` and nothing
else. On an ARM Linux machine `nix develop .#android` resolves perfectly and
then its `clang` will not execute — binfmt routes it to qemu, which has no
x86-64 loader to give it. Apple's linker only exists inside Xcode. So neither
mobile build runs on an ARM Linux workstation, and both run in CI instead:
`.github/workflows/expo.yml` builds Android on an x86_64 runner and iOS on a
macOS one, with nix supplying the identical toolchain in both. Nothing in the
flake changes; the machine does.

`just ffi-bindings` deliberately needs neither. It reads the UniFFI metadata out
of a *host* build of the crate, so the loop that actually matters day to day —
change the Rust, regenerate, see whether the app still compiles — is a couple of
seconds on any machine.

## The Android SDK comes from the machine, not from nix

`nix develop .#android` was composed with `androidenv` at first, so that the SDK
and NDK were pinned like everything else. It failed in CI, and the error was the
interesting part:

```
Failed to install the following SDK components:
    ndk;27.1.12297006 NDK (Side by side) 27.1.12297006
The SDK directory is not writable (/nix/store/…-androidsdk/libexec/android-sdk)
```

The Android Gradle Plugin does not merely *read* the SDK directory; it resolves
the versions a project asks for against it and installs whatever is missing. A
nix store path is read-only by construction, so any version the flake did not
happen to pin is a hard failure rather than a download — and the flake had
pinned build-tools 35 and NDK 28 against an Expo that wanted 36 and 27, in a
layout (`ndk-bundle` rather than `ndk/<version>`) the plugin does not recognise
as installed.

Matching those numbers exactly would have postponed the fight rather than won
it: Expo moves its `compileSdk` and `ndkVersion` on its own schedule and nixpkgs
moves on another, and the next bump breaks the build again in the same way.

So the SDK now comes from where it comes from for every other React Native
project — Android Studio locally, the runner image in CI — and nix pins the part
that is actually ours: the Rust toolchain, its Android targets, `cargo-ndk` and
bun. That is the half that has to match `Cargo.lock`; the Android SDK never did.
The shell also stopped being several gigabytes, and `flake.nix` stopped needing
an unfree opt-in to exist.

## Android is built by EAS

`.eas/build/rust.yml` installs Rust from `rust-toolchain.toml` — still the one
place a version or a target is named — then builds the module, the engine and
the turbo module before Expo's own prebuild and gradle steps. This should run
rarely by design: changing a mutation does not need a build, only changing the
engine does.

## The heart is a path, because the font has no heart in it

The obvious way to put a heart on a button is the character. It does not work:
Fira Sans, which iced embeds, has no U+2665, U+2661 or U+2764 in its cmap — all
three checked by reading the font's tables rather than by looking at a window.
The glyph silently draws nothing, so widgets lay out, input works, and the
button is blank. That is the same failure this log already records for a browser
build with no font at all, and it is just as hard to recognise the second time.

So `examples/heart.rs` draws it: two cubics down each side, filled when the song
is on the playlist and stroked when it is not. No icon font, no asset, and it
reads at a glance without needing colour to explain it.

The Expo screen writes `♥` and is fine, because React Native draws with the
system font. The two clients differ here for a reason, not by neglect.

## The server is a program, not a mode of an example

`just serve` used to run the multiplayer TUI with `--serve`, which meant the
server only existed inside a demo. It is `crates/server` now: an ordinary axum
program with `get(petros_axum::sync::<HarkenApp>)` mounted on it and a
`/healthz` beside it reading the same state.

That is what a server built on Petros should look like, and it is the honest
demonstration of the engine being sans-io — the whole thing is forty lines, and
none of them are about sync.

## The ORM went, because it described the schema a second time

Reads went through Diesel's DSL and writes through checked SQL, which meant the
tables were described twice: once as `diesel::table!`, once as the DDL in
`schema.sql`. Nothing held those two together — `check_for_backend` verifies a
model against `table!`, not against the database — and the half it could check
was the half that mattered least, because `apply` compiles to wasm and has no
Diesel in it to check.

So the read model is `petros_sql::query!` now, like everything else. One
description of the tables, one thing checking it, and it reaches both halves:
renaming `artist` in `schema.sql` produces four compile errors, two from the
mutations and two from the read model. Under the old arrangement it produced two
and a query that still compiled.

What was actually given up is small. `query!` returns a struct per row with a
field per column, typed from what SQLite declares, so the mapping layer the ORM
provided is generated rather than written. What is not given up is the type
checking, which is the thing people mean when they defend an ORM.

Petros still uses Diesel for its own three tables, and `petros::Connection` is
still `diesel::SqliteConnection`. That is the engine's business. An app declares
`App::SCHEMA` and `petros::batch` runs statements that take no parameters, so
nothing above the engine has to name a database library at all.

## The seed and the JSON encoder were never ours

Both were copied into this app from the engine's example and were identical to
it byte for byte. Neither was domain code.

`Seed` expands one uuid into as many ids as a verb needs, which is a rule of
`fill_auto`'s contract: it is the only place non-determinism is allowed, and
everything after it must be a pure function of what it wrote. `from_json` is the
other end of `mutations!` — the macro declares the verbs, petros-codegen emits
the TypeScript that calls them, and that TypeScript sends JSON. The id
convention was documented by the generator and implemented here, separately, in
every app that existed.

Both are `petros-schema` now, with tests the app never had: the expansion is
pinned against a fixture, because what it produces goes into the log and is
replayed forever.

## A feature on a dependency line reaches the wasm build

Asking for `petros-schema/author` beside `cbor` put serde_json and uuid in the
module, even though `crates/harken-wasm` sets `default-features = false` on the
domain crate. Features unify per target; turning a crate's own feature off does
not withdraw one it asked of a dependency unconditionally.

This is the same trap this log already records for `default-features` itself,
and it fails the same quiet way: the module builds, it runs, and it is bigger.
Size is the loop here, because Metro pushes the module on every save. The
feature belongs in `storage` — the one the wasm build actually turns off — and
`cargo tree -p harken-wasm --target wasm32-unknown-unknown` is how to check.
