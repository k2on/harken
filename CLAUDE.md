# harken

A self-hosted, local-first music system, and a **Petros app** — the sync engine
lives next door in `../petros` and arrives as a git dependency, patched back to
that working copy by a gitignored `.cargo/config.toml`.

Four directories, four things: the domain, the server, the desktop client, the
phone. All three programs run the same `apply`: the first two link it, the
phone loads it as a module. The phone's native half is a *feature* of the
domain crate rather than a package, because everything it exports was already
defined there. Each directory carries its own nix as `flake-module.nix`.

The domain is songs and a favourites playlist. Favourites is a real ordered
playlist rather than a flag, so "add to favourites" reads `MAX(pos) + 1` — which
is what makes the rebase visible: heart something while offline and it lands
after whatever arrived while you were away.

## Layout

```
domain/                  the domain — the ONLY apply
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
  mutators.sh            builds the module and its TypeScript types
  flake-module.nix       the same two steps as derivations
server/                  axum, with one Petros handler mounted on it
  flake-module.nix       the package, and the NixOS service
iced/                    the desktop and browser client
  src/heart.rs           the heart, drawn as a path (see below)
  web/                   the browser shell `harken web` serves
  flake-module.nix       the desktop package, and the wasm build with its shell
expo/                    the phone client; src/ is UI and a socket, nothing else
  modules/harken-native/ the turbo module — generated, gitignored, not authored
  gradle-deps.json       gradle's Maven graph, recorded, replayed by the APK build
  eas-rust.sh            the Rust half of an EAS build, for a container with no nix
  flake-module.nix       the APK, and everything on the way to it
flake.nix                the inputs, then everything the four share (see below)
```

The engine's own decisions — the rebase, the log, the wasm ABI, the schema
macro — are in `../petros/docs/decisions.md`. Read that first. This app's are
at the bottom of this file.

## Running it

Everything is a subcommand of `harken`, which the devshell carries and which
`nix run .#harken` runs without it. There is no justfile and no
`rust-toolchain.toml`: the toolchain is named once, in `flake.nix`, and a
contributor works inside `nix develop`.

```
harken                  # fmt, lint, test — what a laptop runs
nix flake check         # …the same three, as derivations. What CI runs
nix build .#harken-server   # …and .#harken-iced, .#harken-web
harken latency          # the measurements: fsync, the sandbox, the maintained view
harken mutators         # rebuild the domain module and hand it to Metro (~0.35s)
harken mutators-watch   # …on every save. Leave it running beside `bun start`.
harken serve            # the sync server…
harken iced alice       # …a desktop peer…
harken iced bob         # …and another, to watch them sync
harken web              # …a browser peer, at localhost:8080
harken bindings         # regenerate the Expo client's TS from the domain crate
harken expo-android     # …and a phone. Needs `nix develop .#android`.
harken engine <rev>     # move the engine pin in Cargo.toml
nix build .#apk         # …or the whole APK, toolchain and all
nix build .#ndk-check       # …does the NDK *start* here? Twenty seconds
nix build .#apk-release     # …the release build (debug-signed, see below)
harken gradle-deps          # re-record gradle's Maven graph (or the CI button)
```

`harken mutators` runs the generator out of `../petros`, so that repository has
to be checked out beside this one. `nix build .#mutators` does not: it builds
`petros-codegen` from the engine `flake.lock` pins, which is what the Android
build and the checks use.

## The flake is four modules and a tail

`flake.nix` names its inputs, imports one `flake-module.nix` from each of the
four directories, and then holds what none of them owns: one nixpkgs and one
toolchain for all four, the source trees, the workspace-wide checks, the
devshell and the `harken` command. Each directory's module is what is true
about that directory and nothing else:

```
domain/flake-module.nix   mutators and petros-codegen, via the engine's lib
server/flake-module.nix   harken-server; `services.harken`
iced/flake-module.nix     harken-iced, harken-web, the iced runtime libraries
expo/flake-module.nix     the phone: one call to `petrosJs.mkApp`, and the android shell
flake.nix (the tail)      pkgs, toolchain, sources, check-*, devShells.default, harken
```

Everything reusable about building for a phone lives elsewhere and arrives
through two inputs. `petros` brings the engine's nix — the crate list, the
`[patch]` that makes the lockfile resolvable, the code generator.
`petros-js` brings `ubrn`, the two-layer cross-compile and `mkApp`, and
carries `expo.nix` (node_modules, `expo prebuild`, `APP_VARIANT`, the gradle
state layer's source) and `android.nix` (the SDK, gradle, the Maven
recording, the layer mechanism, emulation) as inputs of its own. Each of
those flakes follows the one above it for nixpkgs, and the top of the chain
is this file — so `flake.lock` has one nixpkgs, and the SDK is composed from
the same one as the server. `expo/flake-module.nix` is the whole of what this
app has to say about Android: its files, its hashes, which crates are its
own. Read the three libraries' `README.md`s for how the pieces work; the
traps below are still true and still worth knowing, they are just fixed in
those repositories now.

The package names are unchanged — `apk`, `apk-debug`, `apk-release`,
`gradleState`, `androidEngine`, `androidDeps`, `expoModules`, `ubrn`,
`androidSdk`, `gradle9`, `mutators`, `ndk-check`, the three checks — because
the workflows gc-root them by name.

The toolchain is `rust-bin.stable."1.90.0"` with the Android, iOS and wasm
targets, in `flake.nix`. The one other place the version is written is
`RUST_VERSION` in `expo/eas.json`, because an EAS container has no nix to read
it from; `expo/eas-rust.sh` installs that with rustup. Move them together.

## `harken` is for a laptop; `nix flake check` is what CI runs

There are two CI files and they run the same two commands.
`.github/workflows/android.yml` runs on GitHub's hosted runners, which start
empty, so most of that file is installing nix and carrying an eight-gigabyte
tarball of the store between runs — under a 10 GB per-repository ceiling that
holds exactly one such tarball, which is why `main` there is cold whenever a
branch has built since. `.gitea/workflows/android.yml` runs on a self-hosted
runner that builds into the machine's own store, so it has none of that: what
one run built, the next run finds, whatever ref it is on. Its only extra step
registers gc roots for the layers worth keeping, under `HARKEN_GCROOTS`, so
the host's collector walks around them.


They are the same three things — fmt, clippy, the suite — and only one of them
is pinned. `harken` runs cargo against whatever `target/` is lying around; the
checks are derivations over the narrow Rust tree and the vendored dependencies.
CI ran the justfile behind `Swatinem/rust-cache` until that cache served a `target/`
whose fingerprints claimed a build script was fresh and whose binary it had
pruned:

    could not execute process …/build-script-build (never executed)

The identical `harken` passed on a laptop. Two places, same command, different
answers — which is the whole argument for the derivations.

Three consequences worth knowing:

- **`nix flake check` skips when nothing it reads has changed.** What it reads
  is `engineSrc`: `domain`, `server`, `iced`, `Cargo.toml`, `Cargo.lock`, and
  the module's config under `expo/modules`. A `.tsx` edit or a workflow
  change does not run it. An engine pin bump does, because that is
  `Cargo.toml`.
- **A check's output is an empty directory.** What is cached is that it passed,
  and that is the entire skip mechanism — so the outputs have to be gc-rooted
  before the CI cache saves, or the collection takes them and the next run
  learns what it already knew.
- **`harken` alone never actually checks formatting.** It runs `cargo fmt
  --all` first, which rewrites the tree, so the `--check` in `lint` measures
  what it has just written. `check-fmt` is the thing that can fail on
  formatting, and `harken lint` on its own does too.

## The Android build does not reach the network

It did, in three places, and each one was pinned differently:

- **gradle's Maven graph** is `expo/gradle-deps.json` — 1642 artifacts across
  `dl.google.com`, `maven.google.com`, `plugins.gradle.org` and Maven Central,
  replayed through nixpkgs' `mitm-cache` instead of fetched. Regenerate it with
  the `gradle-deps` workflow button (`nix run .#gradle-deps`, or `harken
  gradle-deps`), not on a laptop: recording runs both assembles, and a store
  plus two Android builds is about twenty gigabytes.
- **the mutator module** is a derivation. `domain/mutators.sh` falls back to
  `cargo install --git` when there is no sibling checkout; `nix build .#mutators`
  builds `petros-codegen` from the pinned engine instead.
- **`ubrn`** is compiled from a crate inside `node_modules`, and the npm package
  ships no `Cargo.lock` at all. `expo/ubrn-Cargo.lock` is committed here
  and the vendored result is hashed. Do not trust the lockfile that appears at
  `expo/node_modules/uniffi-bindgen-react-native/Cargo.lock`: cargo
  writes it whenever it runs under this tree, and it comes out carrying the
  engine's seven crates because `[patch]` applies to whatever cargo resolves
  there. It is a local artifact wearing upstream's name.

So there is no `__noChroot` anywhere, which is the point of having done it.
What a sandbox forbids is an *ordinary* derivation reaching outside itself — a
fixed-output one still gets the network, which is how the vendoring fetches —
and what it therefore catches is a build quietly using the machine it happens
to be running on. It has caught four: a hardcoded compiler triple that meant
nothing was set on the runner, `/usr/bin/env` in npm's shims, the aapt2 below,
and the NDK's own emulator. Each of them worked on every machine anyone had
tried.

It also buys a stable path. A sandboxed build runs at `/build` everywhere and
an impure one somewhere ending in a pid and a random number, and gradle's task
history and ninja's `.cxx` both record absolute paths — so native build state
can be carried between derivations, which is worth about three minutes a
module.

## Gradle is 9.3.1, and the version is not negotiable

`pkgs.gradle_9` is 9.4.1 and cannot build this project: it carries
`kotlin-stdlib-2.3.0`, and Expo SDK 57's gradle plugins are compiled with Kotlin
2.1.0, which reads metadata up to 2.2.0. It arrives as `Internal compiler error`
against Expo's own settings plugin, naming no versions. That is what the
template's 9.3.1 pin is for.

`gradle-packages.mkGradle` builds that version and `wrapGradle` puts nixpkgs'
setup hook and `passthru.fetchDeps` on it. Both are exposed deliberately.

Two things follow from taking gradle from nixpkgs rather than unpacking the
zip. The native libraries are patched properly — which is why `ncurses` is
suddenly in the closure, since gradle's `native-platform` jars link it. And
building gradle means *compiling* ncurses: `ncurses-abi5-compat` is
multi-output, Hydra never pushed its `dev` output, and nix substitutes
per-output but builds per-derivation. One 404 costs the whole compile, so
gradle is one of the gc-rooted layers.

## Gradle's state is a layer, the way cargo's `target/` is

nix caches a derivation's output whole and gradle starts every derivation from
nothing, so without help every build recompiles React Native's and Expo's gradle
plugins from Kotlin, re-transforms every AAR, and re-runs CMake for
expo-modules-core, reanimated, worklets, screens and gesture-handler once per
ABI. None of that has anything to do with this app.

`gradleState` does it once. What it is *not* built from is the point: its source
is the Expo project's manifests and nothing else — no Rust, no TypeScript, no
`expo/modules/harken-native` — so changing a mutation cannot invalidate a gradle
build that never saw one. The APK restores `GRADLE_USER_HOME` and every `build`,
`.cxx` and `.gradle` directory under the project and under `node_modules`, adds
the engine, and assembles.

This is what removing `__noChroot` was for. Gradle's task history and ninja's
`.cxx` record *absolute* paths, so a layer built at one path tells a build at
another nothing; a sandboxed derivation runs at `/build` everywhere. `cp -a`
throughout, because gradle and ninja read timestamps as well as content — the
same reason the cargo layers use it.

Three things that are easy to get wrong here, two of which cost a run each:

- **The build attributes are shared rather than duplicated.** `PATH` decides which
  `ninja` and which compiler CMake finds, and CMake writes those paths into
  `build.ninja`. A layer configured with a different `PATH` produces state the
  next build silently cannot use.
- **A nix indented string strips the smallest indentation any line has.**
  Interpolated blocks written flush left take that minimum to zero, so nothing
  is stripped — and `<<'PROPS'` needs its terminator at column 0. The heredoc
  then runs to the end of the script and swallows the SDK copy,
  `local.properties`, the aapt2 override and the `cd android`. What you see is
  bash naming a line in stdenv's `setup` and gradle saying
  `does not contain a Gradle build`. Neither points at indentation. Moving the
  lines "further right" is not enough either: it is a column, not a direction,
  and every line including the `runHook`s has to agree on it.
- **The store erases timestamps, and AGP's C++ configure compares them for
  equality.** nix sets every file in an output to mtime 1 when it registers
  the path, so `cp -a` from the layer carries a 1 into the next build. Gradle
  hashes content and does not care; AGP's configure fingerprint is
  `(lastModified, length)` per input, compared with `!=`, and a mismatch is a
  reconfigure — after which a fresh prefab directory is newer than every
  object and ninja rebuilds the lot. Run 63 had 593 tasks up to date and
  still spent five minutes in CMake, with clang's warnings in the log to
  prove it was compiling. The layer records every carried file's mtime at
  nanosecond precision before the store gets it, and the build replays them
  after the copy.
- **`buildCMakeDebug` reports `EXECUTED` whether or not ninja did anything.**
  The task outcome says nothing; a `C/C++:` compiler warning in the log, or
  thirty seconds after `configureCMakeDebug`, says CMake ran.
- **A configuration-time cache in the carried tree is a list of what the
  layer saw, not what the build has.** React Native's settings plugin keeps
  the output of `react-native config` in
  `android/build/generated/autolinking/` and reuses it while the lockfiles'
  hashes match. The layer wrote it without `harken-native`; the APK build has
  the same lockfiles; so seven runs of APKs linked the layer's list and
  carried no engine at all — 642 tasks where a build without the layer has
  680, `mergeDebugNativeLibs` UP-TO-DATE, and not one `:harken-native:` task.
  The restore deletes that directory. Read the task count against 680 and
  look for `:harken-native:` before believing any layered build.
- **The layer must not be fixed up.** stdenv's fixup ran `patchelf` over 980
  Android objects in the carried `.cxx` and `build/` directories, eighty
  seconds a build, and would have changed any that carried an rpath under
  gradle's content hashes. `dontFixup = true`.

A `.tsx` edit, a mutation, or a change to this file rebuilds the APK and not the
layer, which is the case worth being fast. Moving `bun.lock`, `app.config.ts`,
`gradle-deps.json` or the SDK rebuilds both.

Measured, as the APK step of the workflow:

| run | APK step | what it built                       |
|-----|----------|-------------------------------------|
| 54  | ~540s    | the APK, before any of this         |
| 59  | 1349s    | the layer and the APK, both cold    |
| 60  | 1279s    | the layer *again*, and the APK      |
| 62  | **416s** | the APK, against a cached layer     |

Run 62 is the one to read for the Kotlin half, and its gradle says why:

```
harken-debug-apk>  BUILD SUCCESSFUL in 5m 28s
harken-debug-apk>  642 actionable tasks: 44 executed, 5 from cache,
                                         593 up-to-date
```

593 of 642 tasks already done. Run 60 ran the same 642 tasks twice — once to
build the layer, once to use it — and so came out *slower* than having no layer
at all. That is worth remembering as a shape: a layer whose inputs are too wide
is indistinguishable from a layer that does not work, because both present as
"the step got longer". The task counts tell them apart and the clock does not.

The layer's source is `expo` minus an exclusion list for that reason.
Written as `${src}/expo/…` it takes the whole cleaned repository as an
input, so every commit rebuilds it. Written as six named files it was correct
until the seventh: a new `babel.config.js` or `react-native.config.js` would
have been read by the build and unknown to the layer until someone remembered
to list it. Excluding what changes per commit — `src`, `modules`, the prose —
makes a new config file an input the day it appears, and a forgotten exclusion
costs one extra layer build rather than a layer that quietly stops matching.

The carried directories are found the same way: every `build`, `.cxx` and
`.gradle` whose parent has a gradle build file, wherever it lives. React
Native's gradle plugin, Expo's module plugin and the dev-launcher's are gradle
projects outside any `android/`, and a rule that only looked there left their
compiled state behind — the build cache was covering for it.

## Two apps: `dev.harken.koon.us` beside `harken.koon.us`

A development build and a release build are different apps to Android, so
both can be installed at once. `expo/app.config.ts` is the whole app
config — there is no `app.json` — and decides which from `APP_VARIANT`:
`production` is `harken.koon.us`, "Harken", the icon as drawn; anything else
is `dev.harken.koon.us`, "Harken Dev", with a DEV banner across the bottom of
the icon. The nix build exports it from the APK derivation's `variant` before
`expo prebuild`, `eas.json` sets it per profile, and a bare `expo start` gets
development, which is the only thing it can be pointed at. The config is an
`ExpoConfig`, so `tsc` checks it; the badge plugin is applied as a function
rather than listed under `plugins`, which `ExpoConfig` types as names only,
so its props are checked against the plugin's own type too.

The banner is drawn by `app-icon-badge`, but not by its config plugin.
`expo/plugins/with-dev-badge.js` calls the package's `addBadge` from a dangerous
mod and waits for each file to be readable before pointing the config at it.
The package's own plugin starts the drawing and returns at once — the promise
is dropped, `addBadge` does not await its write, and the iOS branch writes to
the same path as the flat icon at the same time — which showed up here as a
Jimp `parseBitmap` error and, once, a prebuild that lost `android.package`.
A sandboxed build gets no second try. It is JavaScript under `@ts-check`
rather than TypeScript because Expo compiles `app.config.ts` itself but
resolves that file's imports with Node, which does not find a `.ts` beside it.

Two things the plugin needs and does not say: the adaptive foreground must be
1024px, because the adaptive overlay is drawn at that size and composited at
the origin, so on the 512px original the banner lands outside the canvas and
nothing appears — hence `android-icon-foreground-1024.png`, used by the
development variant only. And `ios.icon` is `assets/expo.icon`, a directory in
Apple's layered format that nothing can draw on, so the development variant
drops it and iOS takes the badged flat icon. The legacy launcher icon that
Android 7 draws is composed from the foreground's centre and loses the banner;
everything from Android 8 shows it.

## Never write domain logic in TypeScript

Every mutation and every query is in `domain/src/functions.rs`, written
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
`harken latency` measures it. A thousand songs is 35ms natively and 73ms through
the sandbox; a hundred is 2ms and 16ms.

Petros still uses Diesel internally; that is the engine's business. An app
declares `App::SCHEMA` and never names a database library.

Changing a mutation does **not** need a native build: `harken mutators` rebuilds
the module and rewrites the base64 `.ts` Metro pushes. Changing the *engine*
does, and that is what EAS is for.

`domain/mutators.sh` is the one thing that builds it outside nix, and `harken
mutators` and the EAS hook both call it — only one of them has the devshell.
The single difference between them is where `petros-codegen` comes from: the
checkout beside this one when there is one, so an engine edit needs no commit,
and the published branch otherwise, because a build container has no sibling
directory. CI is `nix build .#mutators`, the same two steps as a derivation.

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
text heart lays out fine and draws nothing at all. So `iced/src/heart.rs` draws
it with two cubics down each side, filled when the song is on the playlist and
stroked when it is not. React Native uses the system font, which has the glyph,
so the Expo screen just writes `♥`.

## Traps in the client toolchain

- **The NDK is x86_64-only, and the build brings its own qemu.** Google
  publishes no aarch64-linux host toolchain, so the NDK's `clang` is an x86_64
  binary. `mkSdk` answers that by wrapping every x86_64 executable in the SDK
  in a pinned `qemu-x86_64` from nixpkgs, so `nix build .#apk` needs nothing
  registered on the machine. iOS still needs Xcode, so a macOS runner.

  It used to rely on the host's `binfmt_misc` instead, which cannot work in a
  sandbox: the kernel invokes qemu, so nothing in the build gets to say how.
  Supplying the emulator is what let `__noChroot` go. It did *not* fix ARM —
  the three traps below did some of that and the last one cannot be fixed from
  here at all — so do not read this bullet as saying ARM works.
- **An emulated compiler has to be told where it lives.** clang reads its own
  path to find its resource headers, its sysroot and the `ld.lld` it execs, and
  under emulation `/proc/self/exe` does not tell it — so `argv[0]` is all it
  has. The wrappers pass their own full path for exactly this. Given a bare
  name instead, clang searches `$PATH`, fails, and settles on the working
  directory:

  ```
  InstalledDir: /nix/var/nix/builds/nix-build-ndk-check…
  ```

  and finds none of those things. It still compiles a file that includes
  nothing, which is why `.#ndk-check` includes a header and links a shared
  object rather than asking for `--version`.

  Linking is the other half. A process already inside qemu cannot exec a
  foreign binary on its own, so the linker only works because clang finds *the
  wrapper* rather than the bare x86_64 `ld.lld` — which is the same fact from
  the other side.
- **An emulated toolchain has to bind eagerly.** Lazy binding resolves a PLT
  entry at first call, through machinery that does not survive emulation, and
  it fails as a lookup error naming a symbol that is demonstrably present:

  ```
  clang: symbol lookup error: undefined symbol: ceilf, version GLIBC_2.2.5
  ```

  That reads like a missing library and is nothing of the kind.
  `LD_DEBUG=libs,versions` shows `libm.so.6` found where it should be and
  ``checking for version `GLIBC_2.2.5'`` *passing*, at load time — and the same
  symbol reported undefined later, at first call. Nothing is absent. The
  wrapper exports `LD_BIND_NOW` for this, and `LD_BIND_NOW=1 clang --version`
  is what named it.

  Corroborating, from the same trace: libm requires `GLIBC_ABI_DT_X86_64_PLT`
  from libc, the marker for a glibc that rewrites PLT entries in place at run
  time. Self-modifying code is what a TCG emulator handles worst, and eager
  binding never goes near it.
- **The host's pages must be 4 KiB, and Asahi's are not.** qemu presents a
  4 KiB-page address space to an x86_64 guest. Where the host cannot map at
  that granularity it has to fake it, and mappings that should be independent
  end up sharing a host page — so a workload with enough mmap churn corrupts
  itself. An `-O3` compile of the SQLite amalgamation is enough:

  ```
  libc++abi: Pure virtual function called!
  qemu: uncaught target signal 6 (Aborted) - core dumped
  ```

  `SIGABRT` rather than `SIGILL`, which is the tell: memory changing under a
  process, not an instruction the emulator cannot execute.

  The control was an `ubuntu-24.04-arm` runner — aarch64 with 4 KiB pages,
  same architecture, same wrappers, same pinned qemu — where `.#androidDeps`
  built in 816s. The only variable left was the page size. That workflow is
  gone now; the finding stands, and anyone who doubts it can run
  `.#androidDeps` on any 4 KiB-page ARM machine.

  **There is no 4 KiB kernel to boot.** Fedora Asahi ships a unified
  `kernel-16k` and dropped the 4 KiB one; nixos-apple-silicon is 16 KiB only,
  and its 4 KiB patch no longer applies with no plan to restore it. Apple
  Silicon runs 16 KiB pages natively and the distributions have settled on
  that. So this is not a setting, and the emulated path on such a machine is
  simply unavailable — which is why the build now refuses to start rather than
  corrupting itself fourteen minutes in (`emulationGuard`, in the engine).

  FEX hits the same wall and answers it by running inside a 4 KiB microVM
  (`muvm`) rather than on the host at all. That is the only local way through,
  and it would mean running the nix build inside the VM — with a store of its
  own, since the host's daemon is on the wrong side of it.

  Offloading is the other way out, and not the obvious one. `.#apk` on an ARM
  machine is an *aarch64* derivation that runs x86_64 code inside itself, so a
  remote x86_64 builder will never be offered it: nix dispatches on a
  derivation's `system`, and this one is native. What offloads is the x86_64
  build of the same expression —

  ```
  nix build .#packages.x86_64-linux.apk --builders 'ssh://box x86_64-linux'
  ```

  which is what CI runs, and where nothing is emulated at all.
- **`.#ndk-check` answers a narrower question than it looks like.** It runs the
  NDK's clang, compiles a file that includes a header, and links a shared
  object — which is enough to catch a toolchain that cannot start, find its
  resource headers, or exec its linker. It is *not* enough to catch either trap
  above: both need a real compile to surface, and one of them needs a large
  one. The check asserts the wrapper still carries `LD_BIND_NOW` rather than
  re-running clang, because re-running clang cannot see the difference.
  `.#androidDeps` is the real test, and `.#androidDeps-debug` is it with the
  guest's loader narrating.
- **`nix develop .#android` cannot run the NDK's clang, and a plain shell can.**
  Under emulation the same command fails inside the devshell with
  `undefined symbol: ceil, version GLIBC_2.2.5` and succeeds outside it — and
  *replaying the devshell's entire environment* in a plain shell also succeeds,
  so it is not a variable. Something about the process `nix develop` creates
  upsets qemu's loader. This is the devshell only: it brings no SDK, so the one
  on your `PATH` is unwrapped and it is the kernel running qemu again. Use
  `nix build .#apk`, which supplies its own, or a plain shell.
- **A host build can pick up the NDK's compiler by accident.** `cargo-ndk` sets
  `CC` for its child, which cc-rs also consults for *host* artifacts — and
  `petros-sql` is a proc macro that links SQLite, so an Android build compiles
  libsqlite3-sys for the host too. It then fails on a missing `stdio.h`, because
  the NDK has no glibc sysroot. Setting the target-qualified variable fixes it,
  since cc-rs prefers that over the bare one — and the triple has to be the
  *build* machine's, not a fixed one:

  ```nix
  hostTriple = builtins.replaceStrings [ "-" ] [ "_" ]
    pkgs.stdenv.buildPlatform.config;
  "CC_${hostTriple}" = "gcc";
  ```

  This read `aarch64` for a while, which is one development box and nothing at
  all on an x86_64 runner — where the build worked anyway, because `__noChroot`
  let the NDK's clang reach the host's `/usr/include`. It surfaced the first
  time a layer was built in a real sandbox, which is the general lesson: an
  impure build hides the environment it depends on.
- **Everything in the SDK must come from nixpkgs, and the SDK must be
  writable.** Two separate problems that look like one. AGP resolves the
  versions a project asks for against the SDK directory and *installs* what is
  missing, which a store path can never allow — so the build copies the SDK
  somewhere writable. But letting AGP install is not a fix either: what it
  downloads is a raw Google binary wanting `/lib64/ld-linux-x86-64.so.2`, which
  NixOS does not have. nixpkgs' copies are patched to a store interpreter and
  run; Google's do not. So every component a build touches has to be in
  `composeAndroidPackages` — here build-tools 35 *and* 36, platform 36, NDK
  27.1, CMake 3.22.1 — and the writable copy exists only so AGP can write its
  own metadata beside them.
- **The SDK's CMake cannot run under emulation, and does not need to.**
  qemu refuses it with "Unable to find a guest_base to satisfy all guest address
  mapping requirements" — the same thing it says about bun. CMake is not
  arch-specific work, though: it only *drives* the compiler. So a native
  aarch64 cmake and ninja go in a directory of their own and
  `local.properties` names it:

  ```
  cmake.dir=/path/to/native/cmake
  ```

  Then only the NDK's clang is emulated, which is the part that has to be.
- **nixpkgs' CMake cannot build React Native, and the version is a red
  herring.** `find_package(ReactAndroid REQUIRED CONFIG)` fails under it even
  though the config file is exactly where `CMAKE_FIND_ROOT_PATH` points. Two
  CMake versions were built chasing that before asking the right question —
  `cmake --debug-find` shows it searching the NDK sysroot and *never trying the
  prefab path at all*. The cause is not the version:

  ```
  000-nixpkgs-cmake-prefix-path.diff
  001-search-path.diff
  ```

  nixpkgs patches CMake's prefix and search-path handling so it does not wander
  outside the store — which is right for nixpkgs and fatal for the Android
  Gradle Plugin, whose prefab support depends on exactly that re-rooting. An
  unpatched CMake is required. On x86_64 that is the SDK's own, which is why
  none of this arises there.
- **AGP brings its own aapt2, and it is not the SDK's.** It resolves
  `com.android.tools.build:aapt2` from Maven and unpacks a raw Google binary,
  which wants `/lib64/ld-linux-x86-64.so.2` and reports its absence as

  ```
  AAPT2 aapt2-8.12.0-13700139-linux Daemon #0: Daemon startup failed
  ```

  naming neither the file nor the loader. The SDK's copy *is* patched and does
  run, and AGP takes an override for exactly this:

  ```
  android.aapt2FromMavenOverride=<sdk>/build-tools/36.0.0/aapt2
  ```

  This file described that trap for a long time while the property was not
  actually set, and the build was green — because an Ubuntu runner *does* have
  that loader, so under `__noChroot` Google's unpatched binary ran and nothing
  said which aapt2 was compiling the resources. Sandboxing the build is what
  asked the question. A documented trap is not a fixed one.

  The pattern is worth stating once: **anything Google's build downloads for
  itself is unpatched and will not run; anything nixpkgs packaged is patched and
  will.** Every such component has to be pinned and pointed at.
- **nix does not supply the Android SDK** for the *devshell*, on purpose:
  gradle installs missing
  components into the SDK directory and the store is read-only. Bring your own
  and export `ANDROID_HOME`; `nix develop .#android` adds `cargo-ndk` and a JDK.
- **Precompiled headers are worth 1.51x on the native half.** Expo SDK 56 added
  `android.usePrecompiledHeaders`, reached through the `expo-build-properties`
  plugin rather than `gradle.properties`. Measured here, run 29 against run 31:
  the gradle stage went 12m28s to 9m52s and the CMake window 6m42s to 4m26s.
  Expo's own benchmark reports 2.81x, but theirs starts at seventeen minutes of
  CMake; 1.3x is what they quote for a default project.
- **`expo prebuild` writes `gradle.properties` with no trailing newline**, and
  its last line is `expo.inlineModules.watchedDirectories=[]`. An appended line
  lands on the end of it, which gives that property a value Expo's autolinking
  plugin hands to `JSON.parse`. Gradle reports only
  `Process 'command 'node'' finished with non-zero exit value 1`, with node's
  message nowhere in the log. Append a newline first.
- **A `command 'node'` failure from a gradle plugin carries no diagnosis.**
  `nix build --keep-failed`, then re-run gradle out of the kept directory with
  `--stacktrace` to find the call site, then run that node command by hand. It
  turns a twenty-minute guess into a one-line bug.
- **The release APK is signed with a key everyone has.** `.#apk-release` is
  `assembleRelease` — the JavaScript compiled to Hermes bytecode and bundled in
  rather than fetched from a dev server — and Expo's template signs that with
  the debug keystore it ships, under a comment telling you to generate your
  own. So it installs anywhere and belongs nowhere public: anyone can re-sign
  it as this app. A real key cannot simply be added to the derivation either,
  because the nix store is world-readable; it would mean building unsigned and
  signing outside. CI builds this from the `workflow_dispatch` button or a `v*`
  tag, and a branch push still builds the development APK.
- **What a maintained view's reader keeps must outlive the component.**
  `libraryUpdate()` reports what *moved* since it was last asked, and it is
  asked once per session — so the list being moved has to live as long as the
  session too. Held in a `useRef` it starts empty on the second mount while the
  view goes on reporting deltas against a list nobody has, and the screen shows
  an empty library over a full database. It reads exactly like "it saves
  nothing", which is a horrible bug to be told about and an easy one to write.
  `@petros/client` puts it in the session's `scratch` for this reason.
- **The phone remembers where it was pointed, and "nowhere" is one of the
  answers.** `recallServer`/`rememberServer` keep the choice across launches
  over `src/storage.ts`, one JSON file read synchronously — a connect screen
  needs its initial value while it renders, and an async read is a frame of the
  wrong answer. A peer with no server opens no socket and retries nothing; the
  engine is told with `disconnected()` so it stops filling an outbox nobody
  will drain.
- **Expo Go cannot load this app.** It calls into Rust, so it needs a
  development build. `ios/` and `android/` are generated by `expo prebuild`.
- **`uniffi` is pinned to `=0.31`** because `uniffi-bindgen-react-native` pins
  it. The generator and the runtime must agree on the metadata format.
- **`nix develop` sets `TMPDIR`.** The demo databases go to
  `std::env::temp_dir()`, so a server started inside `nix develop --command`
  does not share a database with one started under direnv.

## The desktop client maintains its list; the phone does not yet

`iced` holds a `petros::ivm::View`, hydrates it once at boot, and
splices its `Vec<Song>` from the patches the view reports. A tap costs the rows
that moved rather than the whole library — flat, where re-reading grew:
`harken latency` prints the numbers and the decisions below explain them.

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
toolchain — and there is no longer a workflow that tries. `ubrn.config.yaml`
still describes the iOS targets and `HarkenNative.podspec` is still generated,
so the path exists; nobody has walked it. Adding a `macos-15` job back is the
whole of what it would take.

The JavaScript half of the bridge is unmeasured too. What crosses is measured —
`harken latency` prints it, and the decisions below explain why it is seventy
bytes rather than seventy kilobytes — but what React Native then does with those
values on a device is not.

Verified another way, because the local `.cargo/config.toml` hides it: with the
patch moved aside, the whole suite builds and passes against the *pinned* engine
from git, which is what CI and EAS actually resolve.

## Decisions

What is true because this app ships to a phone, one paragraph each, oldest
first. The engine's own decisions are in `../petros/docs/decisions.md`.

- **The Expo client calls Rust; it does not reimplement it.** The obvious way
  to put a list on a phone is to write one in TypeScript and teach it the wire
  format. It was tried first, and it was wrong: two `apply`s in two languages
  is two definitions of what a mutation *means*, and the first time they
  disagree — about `MAX(pos) + 1`, about whether a trimmed empty string is
  refused, about what happens to an edit whose row a confirmed entry removed —
  the replicas diverge silently. So the domain crate's `foreign` feature
  exports the client over UniFFI and `uniffi-bindgen-react-native` generates
  the TypeScript. The generated files are gitignored rather than committed, so
  nobody can hand-edit them, and `harken bindings` regenerates and then runs
  `tsc`, which turns "the app still calls the API the Rust used to have" into
  a compile error. There is no UDL file: `#[uniffi::export]` on the Rust *is*
  the interface definition.
- **The socket stayed in JavaScript.** The foreign client exposes the sans-io
  peer and nothing else: `take_outgoing()` hands back encoded frames, `recv()`
  takes them, and the caller owns the transport. React Native does what a
  browser does in the engine's `transport/web.rs`, because the platform
  already has a WebSocket. The alternative — `tungstenite` and a TLS stack in
  the mobile binary, a uniffi callback interface, a thread to manage across
  backgrounding, `wss://` reimplemented beside the platform trust store — was
  thinner at the call site and worse everywhere else. Frames are tens of bytes
  a few times a second; if that ever changes, the transport is a page of code.
- **Native projects are generated, not committed.** There is no `ios/` or
  `android/` in the tree; `expo prebuild` makes them, and the turbo module is a
  workspace package React Native autolinks. This is why the client cannot run
  in Expo Go, and a development build is not a limitation to work around but
  the consequence of calling into Rust at all.
- **The server is a program, not a mode of a demo.** `server/` is an ordinary
  axum program with `get(petros_axum::sync::<HarkenApp>)` mounted on it and a
  `/healthz` beside it. Forty lines, none of them about sync, which is the
  honest demonstration of the engine being sans-io.
- **No ORM, because it described the schema a second time.** Reads went
  through Diesel's DSL and writes through checked SQL, so the tables were
  described twice and nothing held the two together — and the half that could
  be checked was the half that mattered least, because `apply` compiles to
  wasm and has no Diesel in it. One description in `schema.sql` now, reaching
  both halves: renaming a column produces compile errors in the mutations and
  the read model alike.
- **A feature on a dependency line reaches the wasm build.** Asking for
  `petros-schema/author` beside `cbor` put serde_json and uuid in the module
  even though the wasm build passes `--no-default-features`: features unify
  per target, and turning a crate's own feature off does not withdraw one it
  asked of a dependency unconditionally. It fails quietly — the module builds,
  runs, and is bigger — and size is the loop, because Metro pushes the module
  on every save. To check:

  ```
  cargo tree -p harken --no-default-features -e normal \
      --target wasm32-unknown-unknown
  ```

  `-e normal` matters: dev-dependencies pull in the engine, Diesel and
  serde_json, and none of them ship in the module.
- **One crate, and the two adapters are gone.** A `harken-wasm` crate was one
  `export!`; it existed because the module must be a `cdylib` built with
  `--no-default-features`, which felt like a package and is a flag on the
  build. `crate-type = ["rlib", "cdylib"]` on the domain crate does the same
  and the native rebuild got faster for it. An `ffi` crate went the same way:
  nearly everything it exported was already defined in the domain, so it is a
  feature now, off by default. `petros_schema::row!` emits both `Song` and the
  `foreign::Song` the boundary needs, because `Song.id` is a `petros::Id` and
  the orphan rule refuses `impl FfiConverter for Id`.
- **The read model is a tree, and the join is in the schema.** `library()`
  was a LEFT JOIN and `favorites()` an INNER JOIN, written as SQL. Both are
  gone: `REFERENCES song(id)` is the only place the tables meet, `tables!`
  generates both directions, and a read returns a tree already grouped. Which
  end you read from decides the join. What it cost: `favorite_all` was one
  `INSERT ... SELECT` and is a write per song now, because typed writes must
  say *what* changed. `harken latency` runs it:

  ```
    favorite_all, as a loop over N songs:
       songs      native        wasm
          10     0.22 ms     2.53 ms
         100     2.13 ms    16.45 ms
        1000    34.55 ms    72.81 ms
  ```

  Affordable for a deliberate act on a library that size; the fix if a bulk
  verb ever feels slow is a batched request, not a return to SQL.
- **The client maintains the library, and what that actually bought.**
  `refresh()` ran `library()` after every tap. It holds a `petros::ivm::View`
  now. Maintaining the query alone was six times faster and still grew with
  the library, because `songs_of` decoded every row into a `Song` on every
  call. So the view reports what it did to its own list, as positions, and the
  client splices:

  ```
        songs       re-read    maintained    ratio
           10      0.165 ms      0.026 ms     6.5x
          100      0.349 ms      0.019 ms    18.3x
         1000      1.360 ms      0.009 ms   160.0x
  ```

  Flat. That is the property, not the ratio: the cost is the rows that moved.
- **The phone gets the patches, not the list.** Across the UniFFI bridge the
  whole list crossed on every change. `Peer` holds the views and
  `petros::foreign_peer!` settles them after every mutation and every frame,
  because a view updated by the call sites that happen to think of it is a
  view that is wrong on the ones that do not. `libraryUpdate()` returns either
  patches or, after a rebase, the whole list with `reset` set:

  ```
    one tap, then what the peer hands the phone:
        songs             library()      library_update()
           10      20 rows     1428 B       1 rows       69 B
          100     110 rows     7904 B       1 rows       70 B
         1000    1010 rows    74360 B       1 rows       71 B
  ```

  Seventy bytes, flat, where it was seventy-four kilobytes and growing.
- **The phone, measured on the phone.** Every number above was taken on a
  laptop; the one that decides whether any of it worked is a mutation on an
  Android device, and it is under ten milliseconds however many are made in a
  row. See "Verified on a device" above for the three causes.
- **The layout is four directories, each with its nix.** `domain/`, `server/`,
  `iced/` and `expo/` each carry a `flake-module.nix` for what is in them;
  `flake.nix` holds what they share. There is no `scripts/` — the developer
  tasks are `harken`, in the devshell — no `docs/`, no justfile, and no
  `rust-toolchain.toml`, because a version written in two files is a version
  that drifts. The two scripts that survive do so because something without
  nix runs them: `domain/mutators.sh` for the EAS hook, and `expo/eas-rust.sh`
  which is that hook.
