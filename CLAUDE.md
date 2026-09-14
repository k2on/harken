# harken

A self-hosted, local-first music system, and a **Petros app** — the sync engine
lives next door in `../petros` and arrives as a git dependency, patched back to
that working copy by a gitignored `.cargo/config.toml`.

Four directories, four things: the domain, the server, the desktop client, the
phone. All three programs run the same `apply`: the first two link it, the
phone loads it as a module. The phone's native half is a *feature* of the
domain crate rather than a package, because everything it exports was already
defined there. Each directory carries its own nix under `nix/`.

The domain is media — songs today, an episode or a sermon later — and
playlists. There is no favourites table: a heart means "on the playlist this
client is showing", and which playlist that is belongs to the client. A
playlist is ordered, so adding to one reads `MAX(pos) + 1` — which is what
makes the rebase visible: heart something while offline and it lands after
whatever arrived while you were away.

## Layout

```
domain/                  the domain — the ONLY apply
  schema.sql             the one description of the tables. `migrate` runs it,
                         `tables!` generates the row types from it, and its
                         foreign keys generate the relationships between them
  src/schema.rs          the model: the tables, and the view a client reads
  src/functions.rs       every mutation and every query, one definition each;
                         a mutation takes `ctx: &Ctx` for who authored it
  tests/conformance.rs   the native and wasm builds of `apply`, compared
  tests/converge.rs      the domain against a simulated fleet
  tests/read_model.rs    library() and favorites() against rows apply wrote
  src/lib.rs             …and, under `cfg(wasm32)`, the module's ABI
  src/foreign_client.rs  the client a foreign caller sees (feature `foreign`)
  src/wasm_app.rs        the App whose `apply` is a module (feature `foreign`)
  nix/default.nix        which crate is the module; the vendored-deps hash; `latency`
server/                  axum, with one Petros handler mounted on it, and the
                         sign-in routes beside it
  nix/default.nix        the package, `serve`, and the NixOS service — where
                         the OpenID Connect provider is configured
  nix/readme.nix         its section of README.md
iced/                    the desktop and browser client
  src/main.rs            …and how each target signs in: a loopback port, or the page
  src/icon.rs            every glyph the font has not got, drawn as SVG:
                         the heart and the transport (see below)
  src/vim.rs             the keyboard: vim's grammar, and the one trait a
                         component implements to get it
  src/player.rs          what is playing — an <audio> element in a browser,
                         and nothing at all on the desktop
  nix/readme.nix         its section of README.md
  web/                   the browser shell `nix run .#web` serves
  nix/default.nix        the desktop package and `iced`
  nix/web.nix            the wasm build, `web` and `web-build`
mobile/                  the phone client; src/ is UI and a socket, nothing else
  src/auth.ts            …and signing in, through a browser sheet and `harken://`
  nix/readme.nix         its section of README.md
  modules/harken-native/ the turbo module — generated, gitignored, not authored
  gradle-deps.json       gradle's Maven graph, recorded, replayed by the APK build
  eas.json               generated: the profiles, with what an EAS container installs
  eas-rust.sh            generated: the Rust half of an EAS build, for a container with no nix
  nix/default.nix        the name, the hashes, and the petros-js module they go to
  nix/eas.nix            the EAS profiles
Cargo.toml               the workspace, and the one place the engine is pinned
flake.nix                nixpkgs and petros, and `petros.lib.mkApp inputs ./.`
readme.nix               the top of README.md; the rest is each directory's
README.md                generated from those, like eas.json
```

The engine's own decisions — the rebase, the log, the wasm ABI, the schema
macro — are in `../petros/docs/decisions.md`. Read that first. This app's are
at the bottom of this file.

## Running it

Every developer task is `nix run .#<name>`, defined in the directory it
belongs to; `nix develop` carries the tools they need. There is no justfile
and no `rust-toolchain.toml`: the toolchain is the engine's, at the revision
`Cargo.toml` pins.

```
nix run .#fmt               # cargo fmt
nix run .#lint              # clippy over everything, warnings are errors; fmt --check
nix run .#test              # the whole suite. Must stay under 30 seconds
nix flake check             # …the same three, as derivations. What CI runs
nix run .#latency           # the measurements: fsync, the sandbox, the maintained view
nix run .#mutators          # rebuild the domain module and hand it to Metro (~0.35s)
nix run .#mutators-watch    # …on every save. Leave it running beside `bun start`.
nix run .#serve             # the sync server, in dev auth: anyone is whoever they say…
nix run .#iced alice        # …a desktop peer…
nix run .#iced bob          # …and another, to watch them sync
nix run .#web               # …a browser peer, at localhost:8080
nix run .#bindings          # regenerate the Expo client's TS from the domain crate
nix run .#expo-android      # …and a phone. Needs `nix develop .#android`.
nix run .#write-files       # regenerate README.md, eas.json, eas-rust.sh, ubrn.config.yaml
nix build .#harken-server   # …and .#harken-iced, .#harken-web
nix build .#apk             # the whole APK, toolchain and all
nix build .#ndk-check       # …does the NDK *start* here? Twenty seconds
nix build .#apk-release     # …the release build (debug-signed, see below)
nix run .#gradle-deps       # re-record gradle's Maven graph (or the CI button)
```

`nix run .#mutators` and `nix build .#mutators` both use `petros-codegen`
built from the engine `Cargo.toml` pins; a local engine edit reaches the
crates through `.cargo/config.toml` but not the generator, which is rarely
what changes.

## The flake is four lines, and each directory's `nix/`

`flake.nix` names nixpkgs and the engine and hands the tree to
`petros.lib.mkApp`. That is flake-parts over every `*.nix` under this root —
the dendritic pattern, `import-tree` — plus the engine's own app modules,
which are what every Petros app shares: the toolchain, the workspace, the
three whole-workspace checks and their `nix run` twins, the devshell, and the
wasm module a domain compiles to. Which directories exist is what wires the
rest: each one's `nix/` says what is true about it and nothing else, and a
program with only a `domain/` would be this same `flake.nix` with less under
it. The same file serves a bare crate, a crate with a server, a crate with a
phone, or this.

```
domain/nix/default.nix   petros.mutators.crate, petros.cargoVendorHash; run: latency
server/nix/default.nix   harken-server, `services.harken`; run: serve
iced/nix/default.nix     harken-iced; run: iced
iced/nix/web.nix         harken-web; run: web, web-build
mobile/nix/default.nix   mobile.name, the hashes, the gradle version; the petros-js module
mobile/nix/eas.nix       mobile.eas.profiles
```

What a directory contributes to the workspace — libraries the checks need to
compile it, tools and a hook for the shell, a path a cross-compile reads —
goes through the `petros.*` options the engine's `workspace.nix` declares;
`flake.nix` names no directory and no directory names another.

**`Cargo.toml` is the one place the engine is pinned.** The engine's
`workspace.nix` reads the revision back and fetches petros at it — the crates
for the `[patch]`, `petros-codegen` for the module, `rust-toolchain.toml` for
the toolchain. The `petros` input in `flake.nix` is only the code that does
that reading, so bumping the engine is editing `Cargo.toml`; `nix flake
update petros` moves the nix and nothing else. The phone is pinned the same
way: `mobile/nix` imports petros-js's mobile module from the `@petros/client`
revision `mobile/package.json` names, the same repository as the client the
screens call.

Three files in `mobile/` are generated rather than written, because each
would otherwise repeat what nix knows: `eas.json`, with the Rust version, the
targets and the engine revision an EAS container installs; `eas-rust.sh`, the
hook that installs them and builds the module and the engine there; and the
turbo module's `ubrn.config.yaml`, with the ABIs the APK is built for.
`nix run .#write-files` writes them and `nix flake check` fails while a
committed copy differs from what nix would write — the `files` module,
`github:mightyiam/files`, which the engine's flake carries.

Everything reusable about building for a phone lives elsewhere. `petros-js`
brings `ubrn`, the two-layer cross-compile and the mobile module, and carries
`expo.nix` (node_modules, `expo prebuild`, `APP_VARIANT`, the gradle state
layer's source) and `android.nix` (the SDK, gradle, the Maven recording, the
layer mechanism, emulation), each pinned by its `flake.lock`. Read the three
libraries' `README.md`s for how the pieces work; the traps below are still
true and still worth knowing, they are just fixed in those repositories now.

The package names are unchanged — `apk`, `apk-debug`, `apk-release`,
`gradleState`, `androidEngine`, `androidDeps`, `expoModules`, `ubrn`,
`androidSdk`, `gradle9`, `mutators`, `ndk-check`, the three checks — because
the workflows gc-root them by name.

## `nix run` is for a laptop; `nix flake check` is what CI runs

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
is pinned. `nix run .#test` runs cargo against whatever `target/` is lying
around; the checks are derivations over the narrow Rust tree and the vendored
dependencies.
CI ran the justfile behind `Swatinem/rust-cache` until that cache served a `target/`
whose fingerprints claimed a build script was fresh and whose binary it had
pruned:

    could not execute process …/build-script-build (never executed)

The identical command passed on a laptop. Two places, same command, different
answers — which is the whole argument for the derivations.

Three consequences worth knowing:

- **`nix flake check` skips when nothing it reads has changed.** What it reads
  is `engineSrc`: `domain`, `server`, `iced`, `Cargo.toml`, `Cargo.lock`, and
  the module's config under `mobile/modules`. A `.tsx` edit or a workflow
  change does not run it. An engine pin bump does, because that is
  `Cargo.toml`.
- **A check's output is an empty directory.** What is cached is that it passed,
  and that is the entire skip mechanism — so the outputs have to be gc-rooted
  before the CI cache saves, or the collection takes them and the next run
  learns what it already knew.
- **Formatting is checked, not applied.** The justfile ran `cargo fmt --all`
  before `--check`, so the check measured what it had just written.
  `check-fmt` and `nix run .#lint` both only check; `nix run .#fmt` writes.

## The Android build does not reach the network

It did, in three places, and each one was pinned differently:

- **gradle's Maven graph** is `mobile/gradle-deps.json` — 1642 artifacts across
  `dl.google.com`, `maven.google.com`, `plugins.gradle.org` and Maven Central,
  replayed through nixpkgs' `mitm-cache` instead of fetched. Regenerate it with
  the `gradle-deps` workflow button (`nix run .#gradle-deps`), not on a laptop: recording runs both assembles, and a store
  plus two Android builds is about twenty gigabytes.
- **the mutator module** is a derivation, and `petros-codegen` with it, both
  from the engine `Cargo.toml` pins. The EAS hook, which has no nix, installs
  the generator with `cargo install --git … --rev` at the same revision.
- **`ubrn`** is compiled from a crate inside `node_modules`, and the npm package
  ships no `Cargo.lock` at all. `mobile/ubrn-Cargo.lock` is committed here
  and the vendored result is hashed. Do not trust the lockfile that appears at
  `mobile/node_modules/uniffi-bindgen-react-native/Cargo.lock`: cargo
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
`mobile/modules/harken-native` — so changing a mutation cannot invalidate a gradle
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

The layer's source is `mobile` minus an exclusion list for that reason.
Written as `${src}/mobile/…` it takes the whole cleaned repository as an
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
both can be installed at once. `mobile/app.config.ts` is the whole app
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
`mobile/plugins/with-dev-badge.js` calls the package's `addBadge` from a dangerous
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

## Who a peer is, the server says

The engine holds every entry to the login that pushed it, so every peer
signs in, and all three do it the same way: open the server's `/auth/login`
with somewhere to come back to, receive a single-use code, trade it at
`/auth/exchange` for a session token, and put the token in every `Hello`.
The server is the only OpenID Connect client — `services.harken.oidc` on
NixOS names the provider, the client id, and a *file* holding the secret,
which systemd hands to the service as a credential — and the clients hold no
secret and know no provider. `petros-auth` is all of it; `../petros/docs/
decisions.md` has the reasoning.

Where the code comes back to is the one thing that differs:

- the desktop listens on a loopback port and opens the system browser; the
  login is kept in `~/.config/harken/logins.json`;
- the page sends itself to `/auth/login` and comes back to its own address
  with `?code=`, which it takes out of the URL; the login is in
  `localStorage`. Served by the server it signs in against its own origin,
  which is why `services.harken.web` and `HARKEN_WEB` exist;
- the phone opens the login in `expo-web-browser`'s sheet and comes back on
  `harken://auth`, the scheme `app.config.ts` declares; `mobile/src/auth.ts`.

`nix run .#serve` runs in dev auth — no provider, anyone is whoever they say,
said loudly at startup — so `nix run .#iced alice` is a login for a name and
opens no browser, and the phone's sheet shows a text box. A server refuses
to start with neither a provider nor `HARKEN_DEV_AUTH=1`.

What a mutation sees of this is `ctx: &Ctx`: `ctx.user.id` is the user the
server verified and every song's `actor`; `ctx.session.id` is the login it
was authored under. A peer turned away — an expired token, a revoked
session — keeps its database and its pending edits, stops reconnecting, and
shows a sign-in button; the next login as the same person offers them.

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

**An id says what it identifies.** `Id<tables::Media>` and
`Id<tables::Playlist>` are different types, so `add_to_playlist(playlist_id,
media_id)` cannot be called with its arguments swapped — which it could, and
silently wrote a playlist entry pointing at nothing. The tag comes from the
DDL: `REFERENCES playlist(id)` is what makes a column an `Id<Playlist>`, and
the table reaches `mutations.txt` as `Id(playlist)` and the phone's
TypeScript as a branded `PlaylistId`. Write the row type as `tables!`
generated it — an alias makes the macro guess a table that does not exist,
which is a compile error naming both. The engine's reasoning is under "An id
knows what it identifies" in `../petros/docs/decisions.md`.

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
`nix run .#latency` measures it. A thousand songs is 35ms natively and 73ms through
the sandbox; a hundred is 2ms and 16ms.

Petros still uses Diesel internally; that is the engine's business. An app
declares `App::SCHEMA` and never names a database library.

Changing a mutation does **not** need a native build: `nix run .#mutators` rebuilds
the module and rewrites the base64 `.ts` Metro pushes. Changing the *engine*
does, and that is what EAS is for. The generated `mobile/eas-rust.sh` runs the
same two steps in a container with no nix, and CI is `nix build .#mutators`,
the same two steps as a derivation.

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

## Anything outside Latin-1 is a drawing, not a character

iced embeds Fira Sans, which is a text face. It has no U+2665 heart, and no
U+25B6 play, U+275A pause or U+2582 block either. A missing glyph lays out
fine and draws a `?` or nothing at all, so the button looks *broken* rather
than unfontable — and that is a hard thing to recognise as a font problem.

This cost two rounds, which is the reason it is a rule and not a note. The
heart was fixed first; the transport then shipped with a `?` on its play
button and another in the search prompt's caret, because the lesson had been
written down as "the heart needs an SVG" instead of as what it was. `«`, `»`
and `·` are in the font; `▶`, `❚`, `♥` and `▂` are not. `iced/src/icon.rs`
draws all of them.

React Native uses the system font, which has the glyphs, so the Expo screen
just writes `♥`.

It was a `canvas` for a while, and that is the trap worth keeping. **A canvas
inside a `scrollable` is not translated to the row it belongs to under the
WebGL renderer.** One per list row means every heart draws at very nearly the
same place, so twenty of them stack into what looks like a single stray heart
near the bottom of the list, and scrolling moves the pile rather than the
hearts. It reads as "the heart does not render" — which is how it survived a
demo, because one heart *was* rendering and it was all twenty of them. The tell
is that scrolling changes *which* rows appear to have one.

`iced/src/icon.rs` draws them as SVG, which an image widget positions from the
widget's own bounds rather than from geometry in a shared layer. Same path —
two cubics down each side, filled when the song is on the playlist and stroked
when it is not — and it costs the `svg` feature instead of `canvas`.

Its colour is **not** in the file. The `fill` and `stroke` there are
placeholders that the `svg` style's colour filter replaces, because a heart
with a red baked into it is the same red on a white row, a dark row and the
accent-coloured row under the cursor — three backgrounds, and a colour chosen
against one of them.

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
- **The phone remembers where it was pointed, and who it was there.**
  `src/auth.ts` keeps the last server and, per server, the login — over
  `src/storage.ts`, one JSON file read synchronously, because a connect
  screen needs its initial value while it renders and an async read is a
  frame of the wrong answer. "use offline" is one of the answers, and needs a
  login from before: a peer with no server opens no socket and retries
  nothing; the engine is told with `disconnected()` so it stops filling an outbox nobody
  will drain.
- **Expo Go cannot load this app.** It calls into Rust, so it needs a
  development build. `ios/` and `android/` are generated by `expo prebuild`.
- **`uniffi` is pinned to `=0.31`** because `uniffi-bindgen-react-native` pins
  it. The generator and the runtime must agree on the metadata format.
- **`nix develop` sets `TMPDIR`.** The demo databases go to
  `std::env::temp_dir()`, so a server started inside `nix develop --command`
  does not share a database with one started under direnv.

## The table, and where its colours come from

The track list is a table — Name, Artist, Album, Time, under headings — and
every row is one line. Four things about it are load-bearing:

- **The row's background belongs to a `container` spanning the full width**,
  not to a `button` wrapped around the title. A stripe that stops where the
  text does is not a row. The click comes from a `mouse_area` around that
  container, so the whole line is the target.
- **Nothing is a literal colour.** Every background and every text colour is
  asked of `theme.extended_palette()`, which is the entire reason dark mode
  works: iced already picks Light or Dark from the system, so the only way to
  get it wrong is to write a colour down. The zebra is `background.base`
  alternating with `background.weak`; the cursor is `primary.base` with
  `primary.base.text` on it.
- **A row under the cursor is painted in the accent colour, so text on it has
  exactly one legible colour** — the one that accent was paired with. A
  dimmed column there gets the same hue at lower alpha, never a grey that was
  chosen against the window instead.
- **The two highlights mean different things and are drawn differently.** The
  sidebar's is a *selection* — what the table is showing — so it persists when
  the keyboard is elsewhere. The table's is a *cursor*, only ever "where the
  next `j` goes", so it is not drawn at all unless its pane has the keyboard:
  a dimmed one would sit one shade from the zebra and mean something else
  entirely. The playing track is the one row drawn in the accent *colour*
  rather than filled with it, so it stays findable under either.

The Album column is the interesting one, because `album` is on the `song`
side table and the library row is deliberately kind-neutral. Rather than widen
`Item` — which would put a join behind every list, the thing `media` exists to
avoid — `track_albums()` returns the pairs and the client joins them in memory
while drawing. A screen that wants the column asks for it; one that does not,
does not pay.

## The sidebar browses; the now-playing bar plays, in a browser

The client is a library with a sidebar: playlists, then albums, then artists.
Which of those a row belongs to is not a column on the library list — `album`
lives on the `song` side table precisely so that `media` stays kind-neutral —
so the grouping is four queries in the domain (`albums`, `artists`, `album`,
`artist`) rather than a wider `Item`. Both clients would then fold the library
the same way, and a screen that browses by album is asking a song-shaped
question and gets a song-shaped answer.

Only the library list is the maintained view. Picking a playlist, an album or
an artist costs one query per change, which is the right trade: the list you
are looking at most of the time is the maintained one, and re-reading a single
album when something moves is a hundred rows rather than the library.

**Audio is the browser's, and only the browser's.** `iced/src/player.rs` hands
a URL to an `<audio>` element, which buys streaming, buffering, range requests
and seeking from the platform — four problems that would each have to be solved
again in Rust. The desktop build has no audio device wired up at all:
`Player::AUDIBLE` is false there, the bar says so, and the transport is
disabled rather than silently doing nothing. Closing that gap means a decoder
and an output device (`rodio`, so `cpal`, so ALSA) plus an HTTP reader to feed
them, and it is worth doing when the desktop client has a media store to stream
from — which it does not yet.

The demo's library is public-domain classical recordings from Wikimedia
Commons, by way of the mp3 transcode Commons generates for every audio file:
a browser plays mp3 everywhere, and Vorbis in an `.ogg` does not play in Safari
at all. The URL goes in `file`, which is what that column has always been for,
so nothing about the log changed to carry a recording. Every one was checked
for a public-domain licence and a transcode that answers `audio/mpeg` to a
range request — a dead link there is a silent demo.

The demo is also a listening UI and nothing else: no sign-in (it has no
accounts and no server), no typing a song in, no bulk favouriting, no per-row
remove. Those are `#[cfg(not(feature = "demo"))]` rather than deleted, because
against a real server they are the only way to sign in, add anything, or take
it back out.

## The keyboard is vim's, and a component opts in by saying what shape it is

`iced/src/vim.rs` is a small library with its own tests and no idea what a
song is. Three things are kept apart, because each is useful without the
others and mixing them is what makes keyboard code untestable:

- **`Keys` turns presses into an `Action`.** It knows `5j` is five downs and
  `gg` is the top, and nothing about what is on screen. Tested by pressing
  letters at it.
- **`Navigate` turns a `Motion` into a new cursor.** This is the hook: a
  component says how many cells it has and where a motion lands, and gets
  counts, `gg`, `G` and `{count}G` for free without ever seeing a key.
  `List`, `Row` and `Grid` are the three shapes.
- **`main.rs` does the rest** — which pane holds the cursor, what `Activate`
  means, what `/` matches against. None of that generalises, so none of it is
  in `vim.rs`.

There are two panes, the sidebar and the table, and **the now-playing bar is
not one of them**: everything it does has a key of its own (`p`, `{`, `}`), so
making it a third place the cursor can be would only add a stop to `<Tab>`
that nobody needs to pass through. In the sidebar the cursor *is* the
selection — moving onto a row shows it, with no `<Enter>` in between, because
needing a key to confirm what you have already moved onto is a keystroke that
only ever means "yes, that one". `<Enter>` there steps into the table instead.

**`step` returning `None` means "not mine", and that is the whole pane
mechanism.** A vertical `List` refuses `h` and `l` because it has no
horizontal axis, so `Pane::beyond` is free to read a refused `l` in the
sidebar as "the track list". A `Grid` accepts all four and *clamps* at its
edges, because it does have both axes and the motion was its own. Refusing
and clamping are different answers to different questions, and keeping them
apart is what lets one grammar drive a list, a row and a grid. The bar is the
`Row`: `h` and `l` walk the transport, and `k` is what leaves it.

Every pane is a vertical list, so `List` is the only shape with a caller.
`Row` and `Grid` are kept and tested anyway, because a hook with one
implementation is not a hook — `List` alone could not tell you whether
`Navigate` was a general shape or a description of the track list, and `Grid`
is what makes the difference between *refusing* a motion and *clamping* it
visible.

Two things that are easy to get wrong and cost a round each:

- **`keyboard::listen()` reports only the presses no widget took.** That is
  what makes a modeless vim layer safe beside a `text_input`: a focused entry
  box consumes its own keys and none reach the subscription, so typing a song
  title cannot also walk the cursor down the list. There is no insert mode
  because there is nothing to get stuck in.
- **A freshly loaded page focuses `<body>`, not the canvas**, so every key
  went nowhere until something had been clicked — which reads exactly like
  "the keymap does not work". iced gives its canvas `tabindex=0`, so it can
  take focus; `iced/web/index.html` tells it to, on load and whenever the
  window comes back. That file also sets `color-scheme: light dark`, so the
  page behind the canvas agrees with the theme iced picked rather than
  flashing white under a dark window.

`?` draws the keymap. A keymap nobody can guess is a keymap nobody uses, and
the status line carries the pane and anything half-typed — a swallowed `5`,
or a `g` still waiting for its pair — because invisible pending input is the
one thing that makes a modal keymap feel broken.

## The desktop client maintains its list; the phone does not yet

`iced` holds a `petros::ivm::View`, hydrates it once at boot, and
splices its `Vec<Song>` from the patches the view reports. A tap costs the rows
that moved rather than the whole library — flat, where re-reading grew:
`nix run .#latency` prints the numbers and the decisions below explain them.

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
`nix run .#latency` prints it, and the decisions below explain why it is seventy
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
  nobody can hand-edit them, and `nix run .#bindings` regenerates and then runs
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
- **The server signs people in; the clients only open a URL.** Three clients
  is three places to put an OpenID Connect library, three copies of a
  client secret, and three ways for a phone's suspended socket and a
  browser's cookie jar to disagree about who is signed in. Instead the
  server is the one relying party and hands out sessions of its own, and
  what a client does is the same everywhere: open `/auth/login`, get a code
  back, exchange it, put the token on the socket. The secret is a file the
  NixOS module loads as a systemd credential, never a store path. `nix run
  .#serve` keeps the loop on a laptop honest by signing anyone in as a name
  — and says so at startup — because two peers named alice and bob are still
  the demonstration of the rebase.
- **`README.md` is generated, one section per directory.** A README for
  four directories is four people's prose in one file, and the file is where
  it goes stale. Each directory's `nix/readme.nix` writes its own section
  beside its nix; `readme.nix` at the root is the intro; the engine's
  `readme` module orders them into `README.md` through the files module, so
  it is checked like `eas.json`.
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
  say *what* changed. `nix run .#latency` runs it:

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
  `iced/` and `mobile/` each carry a `nix/` for what is in them and
  the `nix run` programs that belong to them; the engine's `lib.mkApp` holds
  what they share, so `flake.nix` is four lines that would serve any Petros
  app. There is no `scripts/`, no `docs/`, no justfile, and no
  `rust-toolchain.toml`. What would repeat a fact nix holds — the Rust
  version, the targets, the ABIs, the engine revision an EAS container needs
  — is generated from it instead, and the one script left, the EAS hook, is
  one of the generated files.
