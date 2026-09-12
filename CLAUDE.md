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
just              # fmt, lint, test — what a laptop runs
nix flake check   # …the same three, as derivations. What CI runs
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
nix build .#apk   # …or the whole APK, toolchain and all
nix build .#ndk-check       # …does the NDK *start* here? Twenty seconds
nix build .#apk-release     # …the release build (debug-signed, see below)
./scripts/gradle-deps.sh    # re-record gradle's Maven graph (or the CI button)
```

`just mutators` runs the generator out of `../petros`, so that repository has to
be checked out beside this one. `nix build .#mutators` does not: it builds
`petros-codegen` from the engine `flake.lock` pins, which is what the Android
build and the checks use.

## `just` is for a laptop; `nix flake check` is what CI runs

They are the same three things — fmt, clippy, the suite — and only one of them
is pinned. `just` runs cargo against whatever `target/` is lying around; the
checks are derivations over the narrow Rust tree and the vendored dependencies.
CI ran `just` behind `Swatinem/rust-cache` until that cache served a `target/`
whose fingerprints claimed a build script was fresh and whose binary it had
pruned:

    could not execute process …/build-script-build (never executed)

The identical `just` passed on a laptop. Two places, same command, different
answers — which is the whole argument for the derivations.

Three consequences worth knowing:

- **`nix flake check` skips when nothing it reads has changed.** What it reads
  is `engineSrc`: `crates`, `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`,
  `scripts`, `clients/iced`, and the module's config. A `.tsx` edit or a
  workflow change does not run it. An engine pin bump does, because that is
  `Cargo.toml`.
- **A check's output is an empty directory.** What is cached is that it passed,
  and that is the entire skip mechanism — so the outputs have to be gc-rooted
  before the CI cache saves, or the collection takes them and the next run
  learns what it already knew.
- **`just` never actually checked formatting.** `default: fmt lint test` runs
  `cargo fmt --all` first, which rewrites the tree, so the `--check` in `lint`
  measured what it had just written. `check-fmt` is the first thing here that
  can fail on formatting.

## The Android build does not reach the network

It did, in three places, and each one was pinned differently:

- **gradle's Maven graph** is `gradle-deps.json` — 1642 artifacts across
  `dl.google.com`, `maven.google.com`, `plugins.gradle.org` and Maven Central,
  replayed through nixpkgs' `mitm-cache` instead of fetched. Regenerate it with
  the `gradle-deps` workflow button, not on a laptop: recording runs both
  assembles, and a store plus two Android builds is about twenty gigabytes.
- **the mutator module** is a derivation. `scripts/mutators.sh` falls back to
  `cargo install --git` when there is no sibling checkout; `nix build .#mutators`
  builds `petros-codegen` from the pinned engine instead.
- **`ubrn`** is compiled from a crate inside `node_modules`, and the npm package
  ships no `Cargo.lock` at all. `clients/expo/ubrn-Cargo.lock` is committed here
  and the vendored result is hashed. Do not trust the lockfile that appears at
  `clients/expo/node_modules/uniffi-bindgen-react-native/Cargo.lock`: cargo
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
`modules/harken-native` — so changing a mutation cannot invalidate a gradle
build that never saw one. The APK restores `GRADLE_USER_HOME` and every `build`,
`.cxx` and `.gradle` directory under the project and under `node_modules`, adds
the engine, and assembles.

This is what removing `__noChroot` was for. Gradle's task history and ninja's
`.cxx` record *absolute* paths, so a layer built at one path tells a build at
another nothing; a sandboxed derivation runs at `/build` everywhere. `cp -a`
throughout, because gradle and ninja read timestamps as well as content — the
same reason the cargo layers use it.

Three things that are easy to get wrong here, two of which cost a run each:

- **`androidAttrs` is shared rather than duplicated.** `PATH` decides which
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
- **`buildCMakeDebug` reports `EXECUTED` even when it does nothing.** The task
  runs; ninja finds no work. Read the wall clock, not the task outcome.

A `.tsx` edit, a mutation, or a change to this file rebuilds the APK and not the
layer, which is the case worth being fast. Moving `bun.lock`, `app.json`,
`gradle-deps.json` or the SDK rebuilds both.

Measured on run 60, where the layer happened to be rebuilt in the same job and
so both halves are visible against each other:

```
harken-gradle-state>  642 actionable tasks: 540 executed, 102 from cache
harken-debug-apk>     642 actionable tasks: 44 executed, 5 from cache,
                                            593 up-to-date
harken-debug-apk>     BUILD SUCCESSFUL in 5m 24s
```

The same 642 tasks, and 593 of them already done. That run was *slower* overall
— it paid for the layer twice — which is worth remembering as a shape: a layer
whose inputs are too wide looks exactly like a layer that does not work.

`gradleStateSrc` names its six files individually for that reason. Written as
`${src}/clients/expo/…` it takes the whole cleaned repository as an input, so
every commit rebuilds it, and the commit that only touched this file — chosen
*because* it touches nothing the layer reads — rebuilt it too.

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

  `android-arm.yml` is the control, and it is worth keeping for that reason
  alone. An `ubuntu-24.04-arm` runner is aarch64 with 4 KiB pages — same
  architecture, same wrappers, same pinned qemu — and `.#androidDeps` builds
  there in 816s. The only variable left is the page size.

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
toolchain — and there is no longer a workflow that tries. `ubrn.config.yaml`
still describes the iOS targets and `HarkenNative.podspec` is still generated,
so the path exists; nobody has walked it. Adding a `macos-15` job back is the
whole of what it would take.

The JavaScript half of the bridge is unmeasured too. What crosses is measured —
`just latency` prints it, and `docs/decisions.md` explains why it is seventy
bytes rather than seventy kilobytes — but what React Native then does with those
values on a device is not.

Verified another way, because the local `.cargo/config.toml` hides it: with the
patch moved aside, the whole suite builds and passes against the *pinned* engine
from git, which is what CI and EAS actually resolve.
