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
playlists. There is no favorites table and no heart on either client: a
playlist is a list somebody made, and which one a track is on is a client's
question to ask. A playlist is ordered, so adding to one reads `MAX(pos) + 1`
— which is what makes the rebase visible: put something on a playlist while
offline and it lands after whatever arrived while you were away.

## Layout

```
domain/                  the domain — the ONLY apply
  schema.sql             the one description of the tables. `migrate` runs it,
                         `tables!` generates the row types from it, and its
                         foreign keys generate the relationships between them.
                         media / song, and person / work / movement /
                         recording / credit — see "A work is not a recording"
  src/schema.rs          the model: the tables, and the view a client reads
  src/listening.rs       the *other* wire: one account's audio session, and the
                         four sentences its devices say. Not the log, ever
  src/functions.rs       every mutation and every query, one definition each;
                         a mutation takes `ctx: &Ctx` for who authored it
  tests/conformance.rs   the native and wasm builds of `apply`, compared
  tests/converge.rs      the domain against a simulated fleet
  tests/read_model.rs    library(), the covers, the playlists and the work
                         chain, against rows apply wrote
  src/lib.rs             …and, under `cfg(wasm32)`, the module's ABI
  src/foreign_client.rs  the client a foreign caller sees (feature `foreign`)
  src/wasm_app.rs        the App whose `apply` is a module (feature `foreign`)
  nix/default.nix        which crate is the module; the vendored-deps hash; `latency`
server/                  axum, with one Petros handler mounted on it, and the
                         sign-in routes beside it
  src/library.rs         the media directory, as a peer: walk it, watch it,
                         author what it finds
  src/listening.rs       one audio session per account: who is making the
                         sound, and the socket at /listen that relays it
  src/assistant.rs       …and the house: every Home Assistant media_player as
                         a device in it, over one thread and six service calls
  tests/library.rs       …a directory of real files becoming songs, once
  nix/default.nix        the package, `serve`, and the NixOS service — where
                         the OpenID Connect provider is configured
  nix/readme.nix         its section of README.md
iced/                    the desktop and browser client
  src/main.rs            …and how each target signs in: a loopback port, or the page
  src/icon.rs            every glyph the font has not got, drawn as SVG:
                         the transport, in the bar and on the playing row
  src/vim.rs             the keyboard: vim's grammar, and the one trait a
                         component implements to get it
  src/route.rs           what the address bar says, and the back button
  src/listening.rs       this device's end of the account's audio session:
                         the socket, and the one rule about making a sound
  src/player.rs          what is playing — an <audio> element in a browser,
                         and nothing at all on the desktop
  src/covers.rs          a cover, fetched once and shrunk before the renderer
                         ever sees it; two caches, one `want`/`handle` pair
  src/art.rs             …and the square derived from the name, for the eight
                         albums in twelve that have no picture
  nix/readme.nix         its section of README.md
  web/                   the browser shell `nix run .#web` serves
  nix/default.nix        the desktop package and `iced`
  nix/web.nix            the wasm build, `web` and `web-build`
mobile/                  the phone client; src/ is UI and a socket, nothing else
  src/auth.ts            …and signing in, through a browser sheet and `harken://`
  src/app/auth.tsx       …and the route that scheme names, because it is one
  src/app/index.tsx      which server, and the login it remembers for it
  src/app/(app)/_layout.tsx  the signed-in shell: the peer, the sheets, the
                         player and the tab bar, over one `Stack`
  src/app/(app)/home.tsx     …blank, and saying why
  src/app/(app)/search.tsx   …the library, filtered, locally
  src/app/(app)/library.tsx  …playlists, albums, artists
  src/app/(app)/list.tsx     …and the one page all four of those open
  src/peer.ts            the database, the maintained library, and what a
                         screen may ask of either
  src/player.tsx         what is playing — `expo-audio`, the queue, the
                         transport on the lock screen, and this account's end
                         of the listening session, because they are one thing
  src/listening.ts       …the socket that carries it, and the wire spelt the
                         way the wire spells it
  src/media.ts           …and the one place a `file` becomes a URL
  src/theme.ts           the palette: light, dark, and the gold. The only place
                         in this directory a color is written down
  src/ui/icon.tsx        every glyph, as an SF Symbol, a Material Symbol, and a
                         character to fall back to (see below)
  src/ui/player.tsx      the bar and the sheet it grows into, which are one
                         thing with two faces
  src/ui/tabbar.tsx      Home, Search, Your Library — drawn, not routed
  src/ui/swipe.tsx       the two gestures every row has
  src/ui/tracklist.tsx   …and the list that gives them to it
  src/ui/playlists.tsx   which playlists a track is on, and how to make one
  src/ui/devices.tsx     which device is making the sound, and moving it
  src/ui/debug.tsx       every number the peer holds, on the phone being wrong —
                         and the one screen that can re-point it
  nix/readme.nix         its section of README.md
  modules/harken-native/ the turbo module — generated, gitignored, not authored
  gradle-deps.json       gradle's Maven graph, recorded, replayed by the APK build
  eas.json               generated: the profiles, with what an EAS container installs
  eas-rust.sh            generated: the Rust half of an EAS build, for a container with no nix
  nix/default.nix        the name, the hashes, and the petros-js module they go to
  nix/eas.nix            the EAS profiles
branding/                what the program looks like, once
  trumpet.svg            the mark: Pictogrammers' MDI glyph, vendored, Apache 2.0
  LICENSE.trumpet        …and its licence, kept beside it
  nix/palette.nix        the colors — AppKit's greys and the gold — and nothing else
  nix/icon.nix           the angle, the centring, the ground, and every raster
  nix/default.nix        …written out as `palette.rs` and `palette.ts`
  nix/readme.nix         its section of README.md
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
nix run .#icons             # rebuild the app icons into mobile/assets/images
nix run .#write-files       # regenerate README.md, eas.json, the palettes, the favicon
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
branding/nix/*.nix       branding.palette; the icons; run: icons
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
update petros` moves the nix and nothing else.

Editing it is not the whole of bumping it, though: `mobile/eas.json` is
*generated* and carries that revision as `PETROS_REV`, so a pin bump without
`nix run .#write-files` beside it fails `nix flake check` on the files module
rather than on anything that compiles. The diff it prints is the revision, on
one line, which is the whole message — and it is easy to read past as a
formatting complaint about a file nobody typed. The phone is pinned the same
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
  `harken://auth`, the scheme `app.config.ts` declares; `mobile/src/auth.ts`,
  and `mobile/src/app/auth.tsx`, because that URL is also a route.

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

**And a verb missing from `peer!` fails in the one place nothing looks.**
`#[mutation]` writes the authoring function *and* the schema section, and
`peer!` is a separate list — so a verb left out of it compiles, type-checks at
every call site, reaches `mutations.txt`, and reaches the generated
TypeScript. Everything that could plausibly be checked says the verb exists.
What it cannot do is *apply*, and what a caller gets is:

```
rejected: unknown mutation "SetArtwork"; this build knows AddSong,
CreatePlaylist, AddToPlaylist, AddAllToPlaylist, RemoveFromPlaylist,
RemoveMedia
```

at run time, from `mutate`. `set_artwork` shipped that way for the whole two
commits it existed — which is also what later made it the one verb that could
be deleted rather than kept forever, since no log ever carried an entry naming
it. Three things made it invisible and each is worth knowing on its own:

- **`mutations.txt` is not evidence a mutation can be applied.** It is
  generated from the module's schema section, which `#[mutation]` produces —
  so `SetArtwork` was recorded, `check-log` passed, and the log surface was
  correct about a verb no build could run. It says what the log may *carry*,
  never what `apply` will *do*.
- **`let _ = client.mutate(…)` threw the message away.** The demo's seed
  discarded every result, so twelve rejections in a row were silent. A seed
  mutation can only be refused by a mistake in this repository, so it is a
  `debug_assert!` now — which names the verb and the reason on the first run
  of any test or dev build.
- **What it looked like was right.** Covers fell back to the derived square,
  which is what eight of the twelve demo albums correctly do. A grid of
  gold squares is the same picture whether the feature works or has never
  once run, and that is the shape of bug to write a test for rather than
  read for: `domain/tests/read_model.rs` unwraps every cover mutation rather
  than discarding it, and `iced/src/main.rs`'s `demo_covers` walks the demo's
  own boot.

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
*tree* — a song with its favorites hanging off it — rather than a flat product.
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

React Native has the same problem wearing different clothes. The system font
does have `♥`, and the screen used to just write it — but a transport drawn in
text characters looks like a transport drawn in text characters, so the phone
uses `expo-symbols` now: SF Symbols on iOS and Google's Material Symbols on
Android, two names for one idea, both stated in `mobile/src/ui/icon.tsx`. A
name either platform does not have draws nothing at all, which is the identical
failure — so every glyph there also gives `SymbolView` a `fallback` of the
character, and the worst case is a plain arrow rather than a blank square where
the play button should be. `tsc` checks both names against the platform's own
union, which catches the misspelling and not the omission.

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

Its color is **not** in the file. The `fill` and `stroke` there are
placeholders that the `svg` style's color filter replaces, because a heart
with a red baked into it is the same red on a white row, a dark row and the
accent-colored row under the cursor — three backgrounds, and a color chosen
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
- **A new dependency moves two pinned things, and one of them is not local.**
  `nodeModulesHash` in `mobile/nix/default.nix` is the hash of the installed
  tree, per platform, and each can only be computed on the machine it belongs
  to — a stale one fails the build and nix prints the right one, which is the
  whole mechanism. A new *native* module moves `mobile/gradle-deps.json` as
  well: gradle's Maven graph is recorded and replayed offline, so an artifact
  nobody recorded is not a slow download but a build that cannot reach it.
  Re-record with the `gradle-deps` workflow button (`nix run .#gradle-deps`),
  not on a laptop — recording runs both assembles, and a store plus two Android
  builds is about twenty gigabytes. `expo-audio` is the case that made this
  worth writing down: it brings androidx.media3, which nothing here had.
- **…and adding the first one found that the recording could not record.**
  Worth keeping because of how it presented. The recording is made from the
  APK derivation, which restores the gradle state layer — and the layer is an
  ordinary sandboxed build that replays `gradle-deps.json`. So recording a new
  dependency meant first building a layer offline from the graph that, by
  definition, does not have it. The message was not a cycle and named nothing
  to do with one:

  ```
  error: Cannot build '…-harken-gradle-state.drv'
  > Could not find androidx.media3:media3-exoplayer:1.9.0
    Searched in the following locations:
      - https://dl.google.com/dl/android/maven2/androidx/media3/…
  ```

  which is a published artifact at a reachable URL, so it sends you to check
  the repository list and the version, and both are fine. android.nix records
  from a build with no layer to restore now, which also makes the graph the
  complete one: a build with nothing restored fetches everything itself.
  The lesson that generalises is the one about the state layer already in this
  file — *a cache that is also a build input is a cache that can make the
  build wrong* — and this is that, one level up.
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
- **The module goes in before the database is opened, and getting that wrong
  bricks the app permanently.** Opening a peer *replays*: `Client::open`
  finishes whatever confirmed entries the log is ahead on, and replaying a
  confirmed entry is calling `apply` — which, on the phone, is the wasm
  module. Install it afterwards and the first sync that lands before the
  install stores entries nothing can apply; every launch after that meets them
  inside `open`, fails there, and never reaches the line that would have
  installed anything. What the phone shows is

  ```
  could not open the database: PeerError.Refused: no mutator module is
  loaded; the app must install one before mutating
  ```

  which reads like a missing file, sends you to look at `mutators.gen.ts` and
  the bundle, and finds both perfectly correct. Nothing the app does clears it,
  because the one thing that would is on the other side of the failure.

  `mobile/src/mutators.ts` has `before()` for this, and `peer.ts` calls it
  *inside* the `open` it hands the hook rather than in an effect beside it.
  It reaches `petros::foreign_peer!`'s free `install_mutators`, which is free
  for exactly this reason: the interpreter is the process's and never was the
  peer's. `Peer::load_mutators` is still there for the hot swap it was always
  for. The engine's `tests/mutators.rs` walks the whole sequence, because
  neither half of it looks wrong on its own.
- **What a maintained view's reader keeps must outlive the component.**
  `libraryUpdate()` reports what *moved* since it was last asked, and it is
  asked once per session — so the list being moved has to live as long as the
  session too. Held in a `useRef` it starts empty on the second mount while the
  view goes on reporting deltas against a list nobody has, and the screen shows
  an empty library over a full database. It reads exactly like "it saves
  nothing", which is a horrible bug to be told about and an easy one to write.
  `@petros/client` puts it in the session's `scratch` for this reason.
- **The session outlives every screen, so where it is dialling can differ from
  where the screen thinks it is.** `session()` in `@petros/client` is a
  module-level singleton keyed by the actor — deliberately, because that is
  what keeps the database, the socket and the pending queue alive across a
  remount. The consequence is that a peer opened against one address goes on
  dialling it, and a screen rendered with another shows the second. It reads
  as "offline" with no explanation, and it is the one thing the pill cannot
  tell you: `denied` means the server refused, and a socket that never opened
  is not refused by anybody. `src/ui/debug.tsx` prints both, side by side, and
  marks them when they differ. It is also the only caller of `setServer` —
  which existed on the peer for a long time with no screen behind it, so a
  phone that remembered an address it can no longer reach had no way back.
  The connect screen is not that way out: it is skipped once a login is
  remembered, which is the whole point of remembering one.
- **The URL a sign-in comes back on is also a route, and it has to exist.**
  `harken://auth?code=…` is two things at once: the answer
  `openAuthSessionAsync` is watching for, and a deep link the OS hands to the
  app. `expo-router` navigates on the second whatever the first does — so with
  no `src/app/auth.tsx` the sign-in lands on the unmatched-route screen, which
  says *this screen does not exist* while the exchange is quietly succeeding
  behind it. That is a lie about which half is broken, and it sends you to the
  server to look for a page that was never missing. The route takes the code
  and hands it to the sign-in that is waiting for it (`offer`), which is
  whichever of the two saw it first; with nothing waiting — the app was killed
  while the browser was open, and the link relaunched it — it trades the code
  itself, against the server `signIn` recorded *before* opening the sheet.
- **A phone that has a login does not ask for one.** Both answers the connect
  screen wants are remembered, so when `recallServer()` has a login the screen
  redirects into the library rather than rendering — asking somebody to
  confirm the server they typed and the account they chose, on every launch,
  is asking them to agree with themselves. Signing out is what comes back, and
  it works because it forgets the login first. `go` then `replace`s rather
  than pushing, so a back gesture out of the library cannot land on an `auth`
  route that is mid-exchange.
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

## The table, and where its colors come from

The track list is a table — Name, Artist, Album, Time, under headings — and
every row is one line. Four things about it are load-bearing:

- **The row's background belongs to a `container` spanning the full width**,
  not to a `button` wrapped around the title. A stripe that stops where the
  text does is not a row. The click comes from a `mouse_area` around that
  container, so the whole line is the target.
- **Nothing is a literal color.** Every background and every text color is
  asked of `palette::of(theme)`, which is the entire reason dark mode works:
  iced already picks Light or Dark from the system and hands every style
  closure the one it picked, so `of` reads which that was and answers with
  the branding's. The only way to get it wrong is to write a color down.
  The zebra is `background.base`
  alternating with `background.weak`; the cursor is `primary.base` with
  `primary.base.text` on it.
- **A row under the cursor is painted in the accent color, so text on it has
  exactly one legible color** — the one that accent was paired with. A
  dimmed column there gets the same hue at lower alpha, never a grey that was
  chosen against the window instead.
- **There is one shape in the table, and it is on the row making a sound.**
  The heart used to sit in that column on every row, which meant the column
  said something about all hundred of them and therefore nothing about any.
  Now it is empty except on what is playing, where it is the transport —
  play or pause, and a button, because the place you look to see what is
  playing is the place you reach to stop it. The empty one is as tall as the
  button it stands in for: an empty container has no height, so the rows that
  were not playing came out shorter than the one that was.

  What replaced the heart is `a`, which is the same question the phone's sheet
  asks: every playlist, each ticked or not, and a last row that makes one. It
  is one `vim::Grid::column` a cell longer than the playlists, so `j` walks onto the
  new-playlist row like anything else and `<Enter>` there starts naming — one
  shape, one cursor, and no second key to learn. `<Space>` is deliberately not
  bound inside it: the transport should not stop working because a panel is
  up.

- **Three ways to ask, and they are the same menu.** Three dots at the end of
  every row, a right click anywhere on it, and `m` — all three send one
  message and open one panel: Play, Add to playlist, and the album and artist
  the track belongs to. Every entry is something this window could already do;
  what the menu adds is asking for it *about a row*, which neither a key nor a
  click on the row itself could say.

  `RowMenu::entries` is the one definition of what is in it. The view draws
  those and `<Enter>` runs them, so the two cannot come to disagree about what
  the third entry is — which is what happens the second time a menu is edited
  in two places.

  It answers to the keyboard like everything else: `j` and `k` walk it,
  `<Enter>` runs the entry, `<Esc>` closes it, and a click does the same two
  things a key does — lands the cursor and runs it — so whichever you used
  last, the other carries on from there. Asked for with `m` it has no pointer
  to sit under, so it opens at the top of the list.

  **Add to playlist is that menu's submenu, and it is the same component `a`
  opens.** One `view_picker`, in two places: reached from the menu it is
  pinned beside it with the parent still up, and `<Esc>` there goes back to
  the menu rather than to the list; reached with `a` it has no parent and no
  pointer, so it is centred and the page behind it is dimmed. Two ways to one
  question should not be two panels to keep in step — and `fit` is the one
  rule that places both, so a submenu against the right edge opens left for
  the same reason the menu does.

  Both overlays are layers of one `stack!` over the page, not panels in place
  of the list: the picker is about a row, and something that replaces the rows
  hides the one it is about. The menu is placed with `pin`, at coordinates the
  root's `mouse_area(..).on_move` reported — which is why the tracking is on
  the root and not on the list, since `pin` and the point have to share an
  origin. It costs a message per mouse move, and the view is rebuilt by the
  tick twenty times a second anyway; there is no way to put a menu under the
  pointer in iced without a point, and no other widget reports one.

  Each overlay gets a backdrop that closes it, and picking an entry closes it
  too. A menu that only answers to the key that opened it is a menu people
  click around and then click again; one that stays up after it has been
  answered is one you dismiss twice.

  **The menu opens away from whichever edge it is against.** `pin` clips
  rather than scrolls, so a menu asked for near the bottom of the window would
  simply not have its last entries. Which way it has room is arithmetic and
  not a measurement — iced lays out after `view` and this decides before it —
  so `menu_origin` computes the panel's height from its padding, its title and
  however many entries the track earns, against a window size kept in step by
  `window::resize_events`.

- **A scrollbar is not a thing to accent.** iced's default draws a hovered
  scroller in `primary.strong`, which was blue while it read iced's palette
  and would have been *gold* once it read ours — and gold here means "this is
  playing, this is on a playlist, press this". A bar you reached for is none
  of those. It is `background.base.text`, which is white on a dark theme and
  near-black on a light one from the same line.
- **The address bar is a control, and the page had broken it.** A library,
  then an album, then an artist, and the back button leaves the site — which
  is the one thing every browser does that a page does not get to opt out of.
  What the sidebar picked goes in the fragment now: `#album/Water%20Music`,
  read back on the tick, and a link to one is a link somebody can send.

  Four things decided it:

  - **The fragment, not the query.** `?server=` and `?code=` are already
    there — how a page is told where its server is, and what a sign-in comes
    back with — and recording a sidebar click in the query would mean
    rewriting those on every click. A fragment is also the one part of a URL a
    static host never has to be told about, which is what Pages serves the
    demo as.
  - **A route carries names, not ids.** `Source::Playlist` holds an id and an
    id is not something a person types. So the URL says the name and the app
    resolves it against the playlists it has — which also decides what a stale
    link does: a playlist since renamed lands on the library, which is a page,
    rather than on a heading with nothing under it.
  - **Read by polling, not by listening.** Hearing `popstate` means a closure
    kept alive for the life of the page publishing into a channel iced can
    subscribe to. The tick already runs twenty times a second for the
    transport, and reading `location.hash` on it is a string compare that
    gives the same answer a frame later.
  - **The history API, not `location.hash = …`.** Both change the URL; the
    second also fires a `hashchange` the poll would read back as somebody
    pressing the back button. And the write is skipped when the address bar
    already says it, which is exactly the case where the route *came* from the
    back button — writing there would make one place two history entries and
    the back button need pressing twice.
  - **The bar is reconciled, not pushed.** This is the part the first version
    got wrong, and it is worth the paragraph. There are four ways what is
    shown can change — a click on the sidebar, `j` in it, the row menu's
    "Go to", and the back button — and pushing the route from `Message::Select`
    covered three of them. The one it missed was the sidebar's own cursor,
    which is how this window is actually driven: the cursor *is* the selection
    there, so every `j` changes what is shown without going through `Select`
    at all, and the URL silently stopped matching the screen.

    So `update` is a wrapper now: it runs `step`, then `sync_route`, which is
    the only thing in this program that writes the address bar. Every path
    reconciles, including the several that `return` early out of `step` and
    the ones that recurse back into `update`. A rule each call site has to
    remember is a rule that is already broken; this one cannot be missed
    because nothing has to remember it.
  - **Walking the sidebar is one navigation, not forty.** Since the cursor is
    the selection, holding `j` through forty albums changes what is shown
    forty times — and forty history entries is a back button that needs forty
    presses to undo one scroll. `Nav::Replace` says rewrite rather than add,
    it is set by `show_under_cursor` alone, and `sync_route` resets it every
    time so only the step that meant it gets it. The URL is still right at
    every one of the forty.

  `App::routed` is what stops the tick re-resolving the same fragment twenty
  times a second, and a route is not consumed until the sidebar has something
  to resolve it against — so a deep link that arrives before the database has
  opened is still waiting when it does.
- **The page's own background is the one thing a style closure cannot reach.**
  With no `Theme` of our own, iced paints the window from *its* Dark, and
  every row that draws no background — which is half of them, since the zebra
  is a wash over whatever is behind — shows that through. So the root
  container paints itself from `palette::of(theme)`, and the near-black in
  `branding/` is what is actually on screen. This is what "iced's own widget
  defaults keep iced's colors" costs in practice, and it is worth knowing that
  the *page* is one of them.
- **The two highlights mean different things and are drawn differently.** The
  sidebar's is a *selection* — what the table is showing — so it persists when
  the keyboard is elsewhere. The table's is a *cursor*, only ever "where the
  next `j` goes", so it is not drawn at all unless its pane has the keyboard:
  a dimmed one would sit one shade from the zebra and mean something else
  entirely. The playing track is the one row drawn in the accent *color*
  rather than filled with it, so it stays findable under either.

The Album column is the interesting one, because `album_name` is on the `song`
side table and the library row is deliberately kind-neutral. Rather than widen
`Item` — which would put a join behind every list, the thing `media` exists to
avoid — `track_details()` returns the song-only facts and the client joins them
in memory while drawing. A screen that wants a column asks for it; one that
does not, does not pay.

**An album is a page, not a filter, and the three differences are all the
same difference.** Everywhere else the list is a library, a playlist or an
artist — orderings that have nothing to do with where a movement sits in its
work — so an album page is the one place a track number means the sequence
it is drawn beside, and the only place the column appears at all. It is also
the one list in the domain not in library order: `album()` sorts by `part`
then `track`, because that is what makes the section headings a fold of an
ordered list rather than a grouping the client has to invent. And the Album
column goes, because every row of it would repeat the heading; the performer
takes its place, which is the fact that actually differs down the page — two
recordings of one work are two performers, not two albums.

## The sidebar browses; the now-playing bar plays, in a browser

**The sidebar is a fixed four lines now, and the browsing is pages.** It used
to be one line per album and one per artist under headings, which is readable
at the twenty a demo has and is a second scrolling list at two hundred — you
scroll the sidebar to find the thing you scroll. So `Music` holds Songs,
Albums and Artists, `Playlists` holds the lists somebody made, and the two
indexes are *pages*: a grid of cards, each opening the record it stands for.

Four things follow, and each is a decision:

- **The index pages are the first `vim::Grid` caller** this program has had.
  CLAUDE.md said for a while that `Grid` was kept and tested without one,
  because a hook with one implementation is not a hook. A page laid out in
  rows of cards is what `h` and `l` mean something on, and one column there
  would refuse them and hand the cursor to the sidebar halfway along a shelf.
- **How many cards fit is arithmetic, not a measurement.** `App::columns`
  divides the same width by the same card that `view_cards` does, from the
  same window size, for the reason `menu_origin` does: iced lays out after
  `view` and the keyboard has to know where `l` lands before that. A grid
  drawn four across and walked as though it were five puts the cursor on a
  card nobody can see, and it reads as the keymap skipping rows at random —
  which is why the geometry is four constants in one place.
- **`<Enter>` opens a card and plays a row, with no mode to be in.** What the
  cursor is on decides: `card_under_cursor` answers `None` on every page that
  is a list, so one key means the obvious thing in both places.
- **A route resolves against the lists rather than the sidebar.** `source_of`
  used to find the `Choice` whose route matched, and no sidebar line carries
  an album any more — so `#album/Water%20Music` is checked against `albums`
  directly. The rule that a name this peer has not got lands on the library is
  unchanged.

An album page and an artist page open with a header — the square, what it is,
the name, and the numbers true of the whole of it — above the table they
already had. A record is not only a list of tracks, and a page that opens on
the first row of a table says nothing about the record it is of. The artist
header counts *albums*, because that is the fact that differs there; an album
header would be saying "1". Lengths there are `2 hr 30 min` rather than
`m:ss`, because Messiah in `m:ss` is `150:37` and nobody reads that as a
length — `clock` is still what a row uses.

The client is a library with a sidebar: playlists, then albums, then artists.
Which of those a row belongs to is not a column on the library list — the album
lives on the `song` side table precisely so that `media` stays kind-neutral —
so the grouping is four queries in the domain (`albums`, `artists`, `album`,
`artist`) rather than a wider `Item` — with `composers`, `works`, `recordings`
and `recording` beside them now, which nothing draws yet. Both clients would then fold the library
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

**The page says what is playing, and the platform draws it — in a browser,
and nowhere else.** The tab's title becomes `Title — Artist`, and
`navigator.mediaSession` gets the same thing as metadata, which is what puts
a track on Windows' controller beside the clock, on a macOS Now Playing tile
and on an Android lock screen instead of the bare site name. All of it is
`cfg(target_arch = "wasm32")` down to the `Remote` type itself, for the same
reason `AUDIBLE` is false on the desktop: there is no media session there and
no tab to title, so a no-op that pretends otherwise would be one more thing
to read past. Three things about how it is wired:

- **It is a snippet, not web-sys.** `MediaSession` is behind
  `web_sys_unstable_apis`, which is a `RUSTFLAGS` that every build of the
  crate would have to agree on — the nix one, the devshell's, and whatever
  else compiles it. `#[wasm_bindgen(inline_js = …)]` in `iced/src/player.rs`
  travels in the module instead, and nothing outside that file knows it is
  there.
- **A lock-screen button leaves a note.** Its handler runs on the browser's
  stack and the state it wants to move is behind iced's update loop, so the
  snippet keeps one pending instruction and the 50ms tick — the one already
  watching for the end of a track — collects it. Last one wins, which is
  what two presses in a row meant.
- **`play` and `pause` are not one toggle.** The operating system says which
  it means: a controller showing "paused" sends `play`, and answering that
  with a toggle pauses a track something else had already resumed. Same for
  the announcement itself, which is made from the tick rather than from each
  button: the element pauses itself when a stream stalls or runs out, and a
  controller still saying "playing" for it is worse than one a frame behind.

The demo's library is classical recordings from Wikimedia Commons, by way of
the mp3 transcode Commons generates for every audio file: a browser plays mp3
everywhere, and Vorbis in an `.ogg` does not play in Safari at all. The URL
goes in `file`, which is what that column has always been for, so nothing
about the log changed to carry a recording. Every one was checked for a
licence and a transcode that answers `audio/mpeg` to a range request — a dead
link there is a silent demo.

**One recording per work.** The harvest found two complete Goldbergs, and
keeping both meant sixty-four rows for thirty-two pieces — an album claiming
64 tracks and the same variations twice under two sets of titles. Having a
piece twice is not having two pieces. The Brandenburgs show the rule is
about *pieces* rather than performers: no movement there appears twice, and
Nos. 1 and 4 are two recordings between them because that is the only way
either is complete.

**Almost all of it reserves no rights, and the exception is paid for rather
than ignored.** The harvest kept public domain, PDM and CC0 only, because a
credit nobody draws is a condition nobody met. That rule would have left the
Brandenburg Concertos out of a Bach library: every no-rights-reserved
Brandenburg on Commons is a fragment — a coda, the closing bars, a five-second
MIDI cadence — and every complete movement is CC BY or CC BY-SA. So `Seed`
carries a `licence`, `seed()` writes it into `performer`, and the album page
draws that column, which is what makes taking them honest: the credit is on
screen beside the track rather than in a comment nobody reads. `licence` is
not a column on `song` — it is a fact about this demo's sources, not about
music, and the one place it has to appear is next to the performer it belongs
to.

The library is filled by the scanner, so the client does not add to it:
there is no entry box to type a song into, no bulk favorite and no per-row
remove in either build. The mutations stay in the domain — the log is
permanent and `add_song` is what the scanner authors — they simply have no
button. What is still `#[cfg(not(feature = "demo"))]` is signing in and going
offline, which the demo has nothing to do either of with.

**A track's `file` is not a URL, and handing it to the player as one is a
silent failure.** The scanner writes a path relative to the media root and
`/media/` serves that same path back, so the client has to join the two —
`media_url` in `iced/src/main.rs`, which passes a whole URL through unchanged
because the demo's library is Wikimedia links. Skipping the join does not
404: the relative path resolves against the page, loses the `/media/` prefix,
and a server with a single-page fallback answers *any* unknown path with
`index.html` and a **200**. The `<audio>` element is handed HTML and reports
only `the media resource ... was not suitable`, which names neither the URL
nor the type. Each path segment is percent-encoded for the same reason a
`#` in a filename is worse than a space: everything after it is a fragment,
so the request stops mid-filename.

## One audio session per account, and it is not in the log

A person with a phone, a laptop and a browser tab has **one** thing playing,
not three. Every device signed in as them sees it, any of them can pause it,
and moving the sound from one to another is a sentence rather than a track
started again somewhere else.

**None of it is in the log, and none of it ever will be.** That is the first
decision and everything else follows from it. The log is permanent and totally
ordered: every peer replays every entry, forever. An afternoon of listening is
thousands of pauses, seeks and skips, and not one of them is worth replaying
tomorrow — "what is playing right now" is precisely the state that *should* be
lost when the server restarts. So it lives in the server's memory, over a
socket of its own at `/listen`, and `functions.rs` is still the only `apply`.

The protocol is `domain/src/listening.rs`, and it is in the domain crate
because that crate is the only vocabulary the server and the clients already
share — not because it is domain. Nothing in it writes a row. It is behind
`storage`, so the module the phone loads never carries it, and `Id`'s serde
impls come in through that feature rather than the dependency line, for the
reason the `petros-schema/author` comment gives.

**JSON, not CBOR.** The engine's frames are CBOR because they are the log;
these are not, and a third client reads them — the phone, in TypeScript, over
a `WebSocket` the platform already has. `encode` and `decode` are in the
protocol rather than at each end, so a server writing `serde_json::to_string`
and a client writing something else cannot become two answers to one question.

Three rules are the whole of `server/src/listening.rs`:

- **Exactly one device is the output**, and only it makes a sound. Every other
  device of that account draws what it is told.
- **A command goes to the output, not to whoever asked.** That is the feature:
  pressing pause on a phone pauses the laptop.
- **A device that can be heard and asks for something, when nothing else is
  the output, becomes the output.** Without this the first tap of the day
  would do nothing and there would be a device to pick before any music could
  start, which is a setup step for the common case.

Some details that are each a decision:

- **A device is a login.** `petros-auth` says a session is "one login on one
  device", which is exactly the identity this needs and already exists — so
  the device id *is* `ctx.session.id`. A reloaded tab is the same device and
  does not appear twice; signing out and back in is honestly a new one.
- **`audible` is a field, not a guess.** The desktop build has no audio device
  at all, so it is a remote control and can never be the output. The server is
  told rather than inferring it from a user agent.
- **`Desk` owns no socket.** Every method takes what happened and delivers what
  falls out, so who becomes the output, what a command does when nothing can be
  heard, and what a device leaving means are all tested against channels rather
  than against a network. The engine's own shape, for the engine's own reason.
- **A hand-off is one command.** `Command::Start` carries the queue, the place
  in it, the point in the track and whether it was playing — because somebody
  pressing a track and the session moving to another device are the same
  sentence with different numbers in it. Two commands would be two code paths
  on every client and one of them would be the one nobody tested.
- **The old output is told nothing.** It learns from the broadcast that it is
  no longer the output, and a client that is not the output is silent. One
  rule, in one place, rather than a stop command that a dropped socket could
  lose.
- **There is no timestamp on the position.** The server has a clock and the
  clients have clocks and they do not agree, so a device that wants a moving
  scrubber counts from when *it* received the state — and the output resends
  about once a second, which is what keeps the counting honest.
- **The last device out takes the room with it.** Keeping the queue would mean
  a phone opened tomorrow resumes an afternoon nobody remembers, and the log is
  where things are kept.

### What the client does with it, and the one rule

`iced/src/listening.rs` is this device's end. Everything the rest of the
program needs from it is two questions — *am I the output* and *is the sound
somewhere else* — and they are deliberately not each other's negation: a
session with **no** output is neither, and there the right answer is to play
here and let the report claim it. Without that third case the first tap of the
day would need a device picked first, which is a setup step for the common
case.

`App::ask` is the one place a transport button is routed, and the whole of it
is:

```rust
fn ask(&mut self, command: listening::Command) -> Task<Message> {
    if self.listening.elsewhere() {
        self.listening.ask(command);   // a message
        return Task::none();
    }
    self.obey(command)                 // an instruction
}
```

`obey` is also what the *server* calls into when it tells this device to do
something, so a button pressed on a phone and a button pressed here reach the
same five lines. Everything routes through it: play/pause, the two skips, the
scrubber, a click on a row — and the media keys, because a lock-screen button
is a transport button and the controller this page put there is a remote for
the *session* rather than for the tab.

Seven things that each had to be decided:

- **The queue is a `listening::Track`, not an `Item`.** It is also what a
  hand-off carries, and the device receiving it may not have that album in its
  replica yet — so the rows are copied rather than referred to. One type means
  reporting costs a clone instead of a conversion per frame. `file` travels as
  the log's path and each device joins it to *its own* server, which is what
  `media_url` was always for.
- **`Player::toggle` is gone.** Play and pause are two verbs, because no caller
  is in a position to toggle: an operating system's controller says which it
  means, another device cannot know from here whether this one is playing, and
  even the button in this window now asks the *session* which is true before
  deciding. A toggle would answer a lock screen showing "paused" by pausing a
  track something else had already resumed.
- **Silence is enforced on the tick, not at the buttons.** Losing the sound is
  not something a device *does* — it is told, by a broadcast — so the only
  place that can notice is the loop that reads them. One line, and no rule for
  a call site to remember.
- **A device that has never played anything says nothing.** Reporting is what
  claims the output, so a report from an idle tab would take the sound from
  the one actually using it. The guard is `player.track().is_some()`.
- **One number decides when to report.** The position drifting past 1.1s is
  the heartbeat while playing (it crosses about once a second), silence while
  paused (it never moves), and a seek (it crosses at once, however long ago
  the last report was). Three behaviours nobody had to write separately.
- **The bar is optimistic, the way the engine's view is.** A press updates the
  copy of the session this device holds and the broadcast replaces it — because
  the scrubber is *dragged*, and waiting for a round trip per pixel reads as a
  control that did not take. Only play, pause and seek are guessed: `Next` and
  `Start` change which track it is, which would mean guessing against a queue
  another device holds, and a bar showing the wrong title is worse than a bar a
  moment behind.
- **The transport works in a build that cannot make a sound.** `AUDIBLE` only
  decides whether *this* device can be the one playing; being a remote control
  is a use, so the buttons are enabled whenever there is a session to send them
  to.

And the picker, `d` or the speaker at the end of the bar: one row per device,
plus a last row that stops it everywhere — the same shape the playlist picker
has, so `j` reaches it without a second key to learn, and the same `fit` places
it, so being asked for from the bottom of the window is what makes it open
upwards. A device that cannot be heard is **drawn and not selectable**: hiding
it would be worse, because a laptop that is in the session and controlling it
should see itself listed, and "no audio device" is a different answer from "not
here".

**A browser, and only a browser** — the same `cfg` as the media session, for
the same reason it was put there the first time. The desktop has no audio
device so it can never be the output; being a *remote control* is the half it
could still do, and that wants a native WebSocket client, which is a dependency
this workspace does not have and a `cargoVendorHash` to move for it. `nix run
.#web` is the desktop client for anyone who wants one.

### The phone is a shell with six screens, and the player is one of them

`(app)/_layout.tsx` is one `Stack` with the player and the tab bar drawn over
it, rather than `expo-router`'s `Tabs`. That is not a preference: the player
bar has to sit **above** the tab bar and **below** every screen, including the
ones pushed over the tabs, and a router-owned tab bar is a sibling of its
screens with nowhere between them to put a third thing. So a tab is a
`replace` and a record is a push — which is also what makes the back gesture
mean "out of this album" rather than "back to the tab I was on".

**The peer is in that shell and nowhere else.** It used to be opened by the
library screen, which was right while that screen was the whole app; six
screens later, each opening its own would run the read model six times against
one session. `@petros/client` keys the session by actor and would hand them
all the same database, so it would not be *wrong* — just paid for six times
per change, which is the thing maintaining the view was for. The two sheets
live there too, because a row on any screen can ask to be put on a playlist.

Both answers the connect screen collects are **remembered** rather than
carried as route parameters. `server` and `online` used to ride on
`/library?server=…&online=…`, which meant every screen that wanted them had to
be reached through that link — fine for one screen, impossible for six that
reach each other — and a relaunch silently went back online.

**The player is one thing with two faces.** A bar and a sheet used to be two
components with two animations, and the seam showed: what you dragged up was
not what appeared, and closing played a different animation from opening. Now
there is one full-screen container translated by one shared value in `[0, 1]`
— at 0 its top edge is exactly where the bar belongs and the sheet is off the
bottom of the screen, at 1 it covers everything. The bar is pinned to the
container's top and crossfades with the sheet, so the thing under your finger
is the thing that arrives, and the same value run backwards is the close.

Four things about it that are each a decision:

- **Two gestures, split by axis.** Vertical opens and closes, from anywhere,
  including anywhere on the expanded sheet; it fails on a horizontal drag,
  which is what leaves the seek bar's own gesture alone. Horizontal *on the
  bar* skips, because a mini bar is the one control people reach for without
  looking. Both track the finger rather than firing on release: a sheet that
  only moves after you let go is a sheet you are not sure you are dragging.
- **The lift gesture is built twice from one description.** A `Gesture`
  belongs to the detector it is given to, and this one has to be on the sheet
  *and* on the bar.
- **`opacity: 0` is not `pointerEvents: none`.** Collapsed, the sheet is off
  the bottom of the screen except for the strip hanging over the tab bar — and
  an invisible view still takes touches, so without the flag every tap on Your
  Library went into a player nobody could see. Which is why the animation's
  shared value is not enough on its own and there is a React `expanded` beside
  it.
- **The tab bar is drawn first, so the expanded player covers it.** A
  full-screen player with a tab bar across the bottom is two apps.

**A row has two swipes**, in `ui/swipe.tsx`: right to put the track on a
playlist, left for what is not built yet. It snaps back either way rather than
staying open, because both actions are immediate and a row that stays open is
a row with a second state to close. The left one saying "Play next is not
built yet" out loud is deliberate — a swipe that appears to do nothing is a
gesture people try once.

### And the phone, which is the device the feature is about

`mobile/src/listening.ts` is the same end in TypeScript — the protocol mirrored
from `domain/src/listening.rs`, and a module-level singleton socket beside
`@petros/client`'s `session()`, for the reason that one exists: it has to
outlive every screen. `src/app/library.tsx` *points* it (it is the screen that
knows which server and which login) and never owns it.

**The wire is spelt the way the wire spells it**: `position_ms`, not
`positionMs`. Renaming on the way in would be a second description of the
protocol living in the client that reads it, and the first time a field moved
the two would disagree somewhere nobody was looking. The awkwardness is the
point — a field that looks foreign is a field somebody else defined. `player.tsx`
holds the only two conversions, `wireOf` and `trackOf`, and `Track` gained a
`file` beside its `url`: the URL is this phone's answer and the path is what
another device is handed, because it resolves one of its own.

**It is inside `PlayerProvider` rather than in a provider of its own**, because
these are one thing: the session is what is playing, and so is this. Which buys
the part worth having — `usePlayer()` answers with the *session*, so
`miniplayer.tsx` and `nowplaying.tsx` draw a laptop's track without either of
them learning that a laptop exists. Only the picker is new UI.

Three phone-specific things:

- **The lock screen keeps working when the sound is on the laptop, and the
  mechanism is a transition rather than a hook.** `expo-audio` wires the
  notification's buttons straight to its own player and does not offer to hand
  them over — so when the element starts while the sound is elsewhere, nothing
  in this app asked it to, because every button here goes through `ask`. That
  is enough to forward it: go quiet, and pass the press on to whichever device
  is playing. Safe even when it was not a press — the tail of a hand-off made
  while playing looks the same — because `play` is a verb and not a toggle.

  The metadata follows the *session* rather than this phone's element, so a
  phone watching a laptop still carries a transport. What the platform
  **draws**, though, is its own audio session's, which on a phone that is not
  the output has no source loaded: a notification may not appear until this
  phone has played something, and while the sound is elsewhere it will read
  paused whatever the laptop is doing. Fixing that properly means a foreground
  service of our own rather than `expo-audio`'s, which is a native module this
  app does not have. Not verified on a device.
- **A remote scrubber needs a ticker.** The output reports about once a second
  and nothing else re-renders in between, so a bar drawn from another device
  would step rather than move. 250ms while `elsewhere && playing`, and not at
  all otherwise.
- **A device that cannot be heard is drawn and not selectable**, the same as on
  the desktop and for the same reason: hiding the laptop would answer "where
  is my laptop" with silence, and "no audio device" is a different answer from
  "not here". The sheet says which.

## A speaker is a device in the session, not a client of it

`services.harken.homeAssistant` offers every `media_player` entity you name to
every account that is listening. A Sonos becomes something you can pick in the
play bar, hand the sound to, and take back.

Nothing about the protocol changed, and that is the point: `harken::listening`
already describes a device — it joins a room, becomes the output, is told
things and reports what it is doing — and nothing in it says the far end has
to be somebody's screen. What `server/src/assistant.rs` adds is the thing that
*is* a device on a speaker's behalf.

**Home Assistant rather than Sonos directly.** Not convenience: you do not get
Sonos, you get every `media_player` in the house — a Chromecast, a television,
an AirPlay receiver, a speaker group — for the same six service calls. Talking
UPnP would buy one make of speaker and a discovery problem.

**The queue is harken's and the position is the speaker's.** The list is
enqueued rather than fed a track at a time, so the speaker's own buttons and
the Sonos app keep working — and where it *is* in that list is read back from
what it says it is playing. That is not two sources of truth; it is the rule
the `Desk` already has, that the output reports and whoever is making the
sound is right about it. Somebody pressing skip on the speaker moves every
phone in the house.

Six things that are each a decision:

- **A standing device does not open a room and does not keep one alive.** A
  speaker is in the session when somebody is listening, not the other way
  round — and without the second half, a bridge standing in every room would
  mean no room was ever empty, so the queue a person left behind would still
  be there tomorrow. `Desk::stand` beside `Desk::join`, and the reap counts
  the *client* wires.
- **Two people can both take the kitchen, and the second one gets it.** A
  speaker is one piece of hardware and a room is one account's, so the first
  is *told* — `Act::Release`, which is a `transfer(user, None)` — rather than
  left drawing a transport for somebody else's music. That is what a real
  speaker does.
- **A speaker playing something that is not ours is let go.** If it reports a
  URL that is not in the queue it was given — a radio stream, a doorbell
  chime — the session stops claiming it, because a bar with a scrubber
  counting along somebody else's audio is a lie.
- **Only a held player is polled.** A speaker nobody handed anything to costs
  nothing, and a server whose house is asleep makes no requests at all.
- **A speaker fetches from a different address than a phone.** `mediaUrl` is
  its own option: the phone may be on `https://harken.example.com` while the
  speaker only knows one on the LAN. The module *asserts* against the default,
  because the default `publicUrl` is loopback — a Sonos handed
  `http://127.0.0.1:8787/media/…` fetches from itself, and that presents as
  "the speaker plays nothing".
- **`/media` is served with no authentication at all.** That is what lets a
  speaker fetch bytes, and it was true before any of this — worth writing down
  now rather than discovering it later. On a LAN-bound server it is fine; the
  moment the media route is really public, so is the library, and the fix is a
  signed one-shot URL minted by the bridge so the speaker still needs no
  token.

**A missing token file takes the whole server down, before the binary runs.**
`tokenFile` is loaded by systemd as a credential, exactly as the OpenID
Connect secret is — and `LoadCredential` whose source is not there fails the
unit at `step CREDENTIALS`, naming neither the credential nor the path:

```
harken.service: Failed to set up credentials: No such file or directory
harken.service: Failed at step CREDENTIALS spawning …/harken-server
```

Two things make that hard to read. It looks like the *binary* is missing,
because the path in the message is the binary's. And there are two credentials
on this unit, so the message is the same whichever is at fault —
`systemctl show harken -p LoadCredential` lists both sources and `ls` on each
is what tells them apart. A secret under `/run` is written by sops-nix or
agenix rather than by the module, so the unit has to be ordered after
whatever writes it; a secret that is simply not there yet is a file to create.

It is also worth stating the trade plainly: configuring `homeAssistant` is
what makes a speaker's access token able to stop the music. There is no
optional credential in systemd, and the alternative — falling back to an empty
token through `SetCredential` — would start the server and then get 401s from
the house, which is a worse answer than not starting.

`Bridge` owns no socket, for the reason `Desk` does not: every rule above is
tested against values rather than a network, and the thread that polls Home
Assistant is the thin part. Blocking `ureq` on a thread of its own, the same
shape the scanner's watch has — there is no async in this server's own code
and a bridge to a house is not a reason to start.

**A device id is no longer always a login.** It was "one login on one device",
which is exactly right for a client and meaningless for a speaker. It is
"whatever names one output stably" now — `media_player.kitchen` for a speaker,
a login's session id for a client.

One limitation, honestly: **previous walks back only as far as the track you
started from.** The window pushed at the speaker begins at `at`, so earlier
tracks are in harken's queue and not the speaker's. Asking for one should be a
fresh hand-off rather than a `media_previous_track`, and nothing does that
yet. And none of this has been run against a real Home Assistant from here —
the rules are tested, the six service calls are not.

## An album and an artist are rows, and each carries its own cover

They were not, and a cover had nowhere to live. An album was a string on `song`
and an artist was `media.creator`, so the first answer was an `artwork` table
keyed by `(subject, name)` — a table of pictures about things that were not
rows — and a `set_artwork` verb to write it. Both are gone. `album` and
`artist` are tables now, `song.album_name` is a foreign key into the first, and
the cover is an ordinary column on each.

**Keyed by the name, not by an id**, which is the decision everything else
follows from. Two reasons, and the mechanical one comes first: `fill_auto`
hands a mutation exactly *one* uuid, so no single verb can mint an album row
and a song row in the same entry — and splitting it into two verbs would mean
a client authoring a reference to a row it has not seen confirmed. The second
is the rebase. Two peers each adding "Water Music" offline would author two
rows with two ids, one of which loses, and every song pointing at the loser is
left pointing at nothing. A name is the thing both peers already agree on
without being told. So there is no `create_album` and no `create_artist`: a row
appears because a song named it.

**Which means `add_song` writes them**, and carries the two covers:
`album_art` and `artist_art` at the end of its arguments. That is what "the
cover arrives with the song" means — the entry that knows a track's album is
the entry that adds the track, and a picture reaches the log on it. The scanner
sends both empty until it learns to read an embedded tag or a `folder.jpg`.

**`art_to_write` is the one last-write-wins rule in the domain**, and it has
three cases that reading cannot tell apart, so `read_model.rs` asserts each:

- **No row yet — make one**, with whatever the entry brought. Usually nothing,
  and a row with an empty `art` is right: the row is what a song points at, and
  having no cover is a fact about the album rather than a reason not to have
  one.
- **A picture replaces a picture.** `add_song` lets the *first* entry win on a
  file, because adding a song twice is a mistake — but somebody who picks a
  better cover means the newer one, and the log being totally ordered is what
  makes "newer" a fact every replica reaches the same way.
- **Nothing replaces nothing.** An entry with no picture leaves the one that is
  there. This is the case the scanner is in on *every* rescan, and getting it
  wrong wipes a library's covers one track at a time.

  …and the same picture again writes nothing at all, which is what keeps a
  rescan of a thousand tracks a thousand reads and no writes: a `put` would
  move `added_ms` and report a change to every maintained view watching.

**`song.album_name` is nullable, and a foreign key is why.** Petros enforces
these — the first version had it `NOT NULL`, and every test writing a song with
no album failed with `writing to song: FOREIGN KEY constraint failed`, which is
the schema being right rather than the tests being wrong. A song on no record
has nothing to point at; `''` would need an album called `''` to point at, and
inventing one would put a nameless card on the albums page for every single
somebody ever typed in. It also fixed a latent bug: `albums()` used to group
every album-less song under one entry named `""`.

**Nothing has a foreign key into `person`** (which `artist` became — see the
section below), deliberately. `media.creator` is the one kind-neutral name for
whoever made a thing — a song's artist, a classical work's composer, a sermon's
speaker, a podcast's show — and a foreign key from it into a table of people
would be naming some of those wrongly. So `person` is a table of what is *known
about* a creator, joined by name where there is a row, and `artists()` still
reads its names from `media`: a new kind appears in that list without the query
learning about it, and a creator with no row is a creator with no cover rather
than one who does not exist. `albums()` is the same shape for the same reason —
which albums *exist* is the songs' answer, and the table is asked only for the
picture.

**`art` is spelt exactly as `media.file` is** — a path under the media root or
a whole URL — because a client already knows how to turn one of those into
something it can fetch, and a second rule for pictures would be a second thing
to get wrong.

**No `SCHEMA_VERSION` bump, by decision rather than by accident.** `song` lost
a column and gained another, which is exactly the shape change the bump exists
for — so an existing database keeps its old `song.album`, `migrate` will not
alter it (`CREATE TABLE IF NOT EXISTS` does nothing to a table that is there),
and every query breaks on the missing `album_name`. The call was "still alpha":
nobody carrying a database has to be able to open it, so anyone holding one
reinstalls. Bumping it is a one-line change to make later, and it is the thing
to do the day somebody's library is worth keeping.

**`set_artwork` could only be deleted because it had never once run.** The
engine forbids removing a mutation variant — `log-compat` refuses it by name,
"the log still carries entries naming it, and every peer replays them" — and
that rule is right. The exemption here is factual rather than an override: the
verb was missing from `peer!` for its entire life, so every call was refused at
apply time and no log anywhere has ever held a `SetArtwork` entry. `--write`
records the new surface deliberately, and this was the one moment it could be
done honestly.

**Both clients draw it now, and the caches are different on purpose.**
`iced::widget::image` takes bytes or a path and there is no URL widget, so the
desktop needed a fetch of its own: `iced/src/covers.rs`, one `want`/`handle`
pair over two implementations.

- **On a desktop it is a directory**, under `XDG_CACHE_HOME/harken/covers`,
  keyed by FNV-1a of the URL. Beside the remembered login rather than in the
  state directory, because this is a *cache* — losing it costs a re-fetch and
  nothing else — and that is where a machine sweeps caches. The fetch is
  blocking `ureq` on a thread of its own with the answer coming back over
  `iced::futures::channel::oneshot`, because the desktop executor is `smol`
  and handing it a blocking HTTP call would stall every other task behind it.
- **In a browser the disk belongs to the browser**, so it is the Cache API —
  the same store a service worker uses, keyed by URL, which is exactly the
  shape of this question. A snippet rather than `web-sys`, the same trade
  `player.rs` makes for the media session and for the same reason:
  `CacheStorage` is behind `web_sys_unstable_apis`, which is a `RUSTFLAGS`
  every build of the crate would have to agree on. `caches` needs a secure
  context, so it is absent on plain `http://` that is not localhost — a
  fall-through to a plain fetch rather than a failure.

Three things that are each a decision:

- **A failure is remembered and its reason is dropped.** A 404 does not become
  a 200 because the window was redrawn, so `State::Missing` stops it being
  asked again — but a status line reading "could not fetch the cover" forty
  times would bury the notes that matter, like having been signed out. A cover
  is decoration; the derived square is already correct.
- **Every cover on a page is asked for, not the visible ones.** Scrolling does
  not go through `update`, so a cover that starts loading when it comes into
  view is a cover that is never there when you look at it. `want` is
  idempotent, which is what makes that affordable.
- **The ask follows the library, not the wire, and getting that wrong showed
  nothing at all.** The first version asked when a sync *arrived*, which is a
  different question and was wrong in both directions: the demo has no server,
  so nothing ever arrived and not one cover was ever fetched; and a real
  client's first library comes out of opening its own database, before any
  sync lands. `Peer::art_gen` is bumped whenever `reload_sidebar` rebuilds the
  two lists and `App` compares it against what it last asked for — so the
  trigger is the data changing, which is the thing that actually decides
  whether there is a new cover to want. It costs one integer compare per
  frame, where calling `want_covers` outright would walk every album and
  artist twenty times a second.
- **The demo's pending mutations are in a file of their own.** A peer with no
  server never confirms anything, so the whole seeded library lives in
  `petros-demo-demo.db-intents` beside the database — and deleting the `.db`
  alone reopens a peer that replays every song the last run authored, with
  `seed` then returning early on a library it did not make. The test that
  wants a fresh demo deletes all four files, and it went green against a
  stale one first.
- **…and the database name is per test, because that file is shared.** The path
  is `petros-demo-{id}.db` and cargo runs a binary's tests on several threads,
  so two tests both deleting and reseeding one demo database fail *only when
  run together*: each passes alone and the suite fails, which is the worst way
  to find out. `fresh` takes a name for exactly this.
- **What is cached beyond the session is bytes, never handles.** A `Handle`
  holds decoded pixels; the map is the session's and the disk is the machine's.
- **The decode happens where the fetch is, and putting it anywhere else was a
  visible stutter.** `Handle::from_bytes` does *not* decode — it hands iced the
  encoded bytes and the decode runs inside whichever frame first draws them. So
  each seeded portrait was a 1.2 megapixel JPEG decoded in a frame and then
  4.8 MB of RGBA in the atlas for something drawn 132 pixels wide; seven
  artists is 34 MB. iced evicts what a frame did not use, so opening an artist
  page dropped six of them and going back decoded all six again — which is why
  it presented as a hitch on *navigation* rather than on load.

  `covers::BOUND` is 384, a shade under 3× the largest square this program
  draws, and `Handle::from_rgba` is what reaches the renderer. Natively that is
  the `image` crate on the thread the blocking fetch is already on — no new
  *crate*, since iced's own `image` feature already put it in `Cargo.lock`. In
  a browser it is `createImageBitmap`, the platform's decoder, off the main
  thread.

  **It moves `cargoVendorHash` anyway, and the commit that added it said
  otherwise.** "No new crate, so the vendored set is identical" is the obvious
  reasoning and it is wrong: `nix/app/workspace.nix` ends its vendor derivation
  with `cp Cargo.lock $out/Cargo.lock`, so the *lockfile is part of the output*
  and a dependency edge added to a crate already in the tree moves the hash
  like anything else. `domain/nix/default.nix` says exactly this, in a comment
  written the last time somebody learned it. Read the comment next to the
  number before reasoning about the number.

  **Decoded from the fetched blob, never from the URL.** A canvas that has
  drawn a cross-origin image is *tainted* and `getImageData` on it throws a
  SecurityError, so going through bytes the page already holds is what makes
  the pixels readable at all.

  It is a bound and not a size: aspect is kept, so `ContentFit::Cover` still
  does the cropping in `view` where that decision belongs. And what the disk
  caches is still the *encoded original* — that cache is of the fetch, and
  keeping the shrunk pixels would be ten times the disk to save CPU that is no
  longer on the render thread, in a form that could not be re-shrunk if the
  bound ever moved.

## A work is not a recording, and neither is an album

Three nouns where there was one, and a classical library is unbrowsable without
them. **Composer → Work → Recording → Album/Track** is Apple Music Classical's
spine and MusicBrainz's; their catalogue is quoted as "20,000+ composers,
115,000+ unique works, 350,000+ movements" over 5M tracks, which is three
counts because they are three tables. MusicBrainz says the join outright: a
track is always associated with exactly one recording, and a recording can be
linked to any number of tracks.

- a **work** is the composition — `BWV 988`, written once, never performed;
- a **recording** is one performance of it, by particular people on a
  particular day;
- an **album** is a *release*, and may carry several recordings.

That last distinction is the whole thing. This file used to grope for it in
prose — "Nos. 1 and 4 are two recordings between them because that is the only
way either is complete" — with nowhere to put the word. The demo now says it in
rows: BWV 1046 and BWV 1049 each come out with **two recordings**, and it took
no change to `seed.rs` at all.

**Pop is not a second case, and that is the point.** The industry already has
both layers — an ISWC identifies a work and an ISRC a recording — and pop hides
them because its works have one movement and nobody quotes their catalogue
numbers. So a pop track is a song with no work and no movement, which is a true
statement rather than a hole, and `media`, the library list, search, playlists
and the player are byte-identical to what they were.

**Every track has a recording, though, pop included**, and that one is load
bearing. The first draft made `recording` require a `work`, which means credits
hang off recordings for classical and off nothing for pop — so "who played
this" would be two different questions depending on genre and one browse page
would mean two things. `recording.work_id` is nullable instead. The cost is
about two rows per pop track and the gain is that `credit` is the one answer
for every genre, which also turns "somebody feat. somebody else" from a string
a list draws into two people a library can be browsed by.

**Credits are on the recording, not the track**, because a conductor is a fact
about a performance. Per track, two movements of one recording could disagree
about their own conductor — the same argument that keeps a cover off `song`,
one level up.

### The keys are natural, and classical is why that is free

`fill_auto` hands a mutation exactly one uuid, so no verb can mint a work and a
song in the same entry, and two peers each adding "BWV 988" offline would author
two rows with one losing the rebase and taking every track pointing at it into a
dangling reference. Classical is the one genre where the answer costs nothing:
**a catalogue number is exactly a name every peer agrees on without being
told**, which is what catalogue numbers are for. `work_key`, `movement_key` and
`recording_key` in `functions.rs` are the whole of it, they are permanent the
way `apply` is, and nothing ever draws one.

That is also what lets there be separate verbs at all — `describe_work`,
`describe_recording`, `describe_person`, `credit_recording` — because a client
referencing a row it has not seen confirmed is safe when the reference is a
function of the data.

- **`describe_work` and `describe_recording` refuse a row no song has named**,
  rather than making one: a work with an id and no composer breaks the foreign
  key that makes it a work. `describe_person` does make it, because a person row
  needs nothing but a name.
- **Every field is fill-if-given.** An empty string and a 0 leave what is there,
  so a scanner that learns the period on a second pass and says nothing about
  the key does not erase the key. This is `art_to_write`'s rule, generalised.

### Two numbers that look like one

`song.track` is where a track sits **on the release**; `movement.no` is where it
sits **in the work**. A compilation puts the Moonlight's first movement at track
9. `album()` sorts by the first and `recording()` by the second, from the same
rows, and conflating them is what makes an album page and a work page disagree.

### What an entry written before any of this replays as

`AddSong` still carries `part`, `catalogue` and `performer` forever — a log
argument can never be withdrawn — so the only question was where `apply` puts
them. Reading an old entry's empty `work_title` as "no work" would throw away a
whole library's structure, so two things in an old entry are read as saying
there is one:

- **a catalogue number**, because nobody catalogues a track;
- **a part**, because `song.part` was defined as "the division of the work this
  belongs to", so a track with one is a track of a work by the definition of the
  column.

The second is the one that got missed, and it silently dropped the part of every
uncatalogued suite — a whole album's grouping — until
`an_album_is_in_the_works_order_and_not_the_librarys` failed. The work's *name*
is then the album's, because this library was authored one record per work, and
`movement_no` reads as the track number for the same reason. Those readings are
why the demo grows works, movements and recordings without one line of `seed.rs`
moving.

### The lumped performer, and the row that is a fallback

`add_song` gets one string — "London Symphony Orchestra, Hermann Scherchen" —
and splitting it into two people with two roles is a guess. So it writes one
credit at **`pos: 0`, which is what marks it a fallback**, and
`credit_recording` writes real credits from 1. `performers_of` prefers the real
ones and ignores the fallback when there are any.

Without that they read together and every properly credited recording lists its
orchestra twice, once on its own and once inside the lumped string it came
from. The test did not catch it: `a_lumped_performer_becomes_people_with_roles`
asserted `contains(…)`, which the fallback satisfied on its own, so the test
passed under falsification. It asserts the whole string now.

**A known wart, and it is the demo's data rather than the schema's.** `seed.rs`
puts the licence *inside* the performer string, so Water Music's first suite is
credited to a person called `(CC BY-SA 3.0)`. `recording.licence` is where that
belongs and moving it is one line — but the album page deliberately *draws* the
licence beside the track ("a credit nobody draws is a condition nobody met"), so
the seed and the client have to move together or the credit is simply lost.

### The demo says all of it, and the seed is where that is said

`seed.rs` carries three small tables beside the tracks, because a track cannot
carry any of this:

- **`WORKS`**, keyed by catalogue number, with the work's own name, its form,
  its period and the year. This is what the album-name fallback could not do:
  before it, Book I of the Well-Tempered Clavier was twenty-four works all
  called "The Well-Tempered Clavier" and the six Brandenburgs were six called
  "Brandenburg Concertos", because a work with no name of its own takes the
  record's and this library puts a whole collection on one record.
- **`CREDITS`**, keyed by the exact lumped performer string, saying which half
  of "London Symphony Orchestra, Hermann Scherchen" is the orchestra. An
  instrument is left empty where the source does not say one — Kimiko Ishizaka's
  Open Goldberg is piano and Vince DiMartino plays the trumpet part Brandenburg
  No. 2 is famous for, and the rest are named without one.
- **`COMPOSERS`**, with sort names and dates.

**The seed computes the same keys `apply` does, from the same two functions**,
because `describe_recording` needs an id and the only way to have one is to
derive it. That is a guard rather than a duplication: if the two ever disagreed,
the verb refuses an id it does not have and the seed's `debug_assert!` says so
on the first run of any test. Falsifying one of these tests tripped exactly that
assertion rather than the test's own — which is the mechanism working.

**The licence moved to `recording.licence`** and `TrackDetail` carries it, so
`credit(performer, licence)` in `iced/src/main.rs` puts the two back together
for the one column a table has. The wart stage one left standing is gone:
nobody is called `(CC BY-SA 3.0)` any more. Both halves are asserted, because
moving the licence out without drawing it again would have been a regression
dressed as a cleanup — the demo's Brandenburgs are CC BY and CC BY-SA.

Water Music's first suite is still credited to nobody, with its licence shown
and no performer: Commons does not say who played it, and an empty credit is
the honest answer rather than a guess.

### What is not done yet

The domain and the demo's data are done; the *clients* are not. `composers()`,
`works()`, `recordings()`, `recording()` and `credits()` exist and nothing draws
them — the sidebar is still Songs / Albums / Artists, and `albums()`,
`artists()`, `album()`, `artist()` and `track_details()` keep their signatures
so both clients still work. `track_details` joins `part` and `catalogue` out of
the work and rebuilds `performer` from the credits, which is what makes that
true.

A library with no works has no Composers page and no Works page to draw, so the
sidebar will have to show a line only when there are rows behind it — the rule
`Playlists` already follows.

## The square when there is no cover

`iced/src/art.rs` derives one from the name, which is most of what a cover is
doing in a list and the part that survives having no picture. It is the
*fallback* now rather than the answer.

**It is an SVG tinted by the style closure, and the first version was a
`container` with a gradient background that drew nothing at all.** The
container laid out at the right size and the glyph inside it appeared; the
background simply never painted. This program already knew the answer to that
shape of problem — `icon.rs` draws the transport and the heart as SVG because
an image widget positions itself from its own bounds and nothing else — and
the `svg` feature was already on for exactly that. Rounded corners and the
artist page's circle come free with it.

The consequence is that the *colour* cannot be in the file. `view` never sees
the theme: iced resolves Light or Dark internally and hands it only to style
closures, so a two-colour gradient chosen per theme is not something a handle
can carry. `svg`'s filter replaces every colour in the drawing with one. So
the depth is in **alpha** instead — the gradient runs from the tint at full
strength to the tint at a third, which survives the filter because the filter
replaces colour and leaves opacity alone.

Two smaller traps, both paid for:

- **`"#` ends a `r#"…"#` literal**, and every colour in an SVG is written
  `fill="#RRGGBB"`. It fails as `expected token: ','` pointing inside the
  string, which reads like a `format!` problem. `r##"…"##`.
- **U+00B7 is a four-pixel dot at any size.** The first note drawn on the
  square was a `·` at a third of the square's height, and what appeared was a
  speck in the middle of an invisible rectangle — which made the missing
  background much harder to see, because something *was* on screen. The note
  is drawn now, not typed.

**The two clients must agree, and the hash is where that is won or lost.** The
six gradients are generated into `iced/src/palette.rs` and
`mobile/src/palette.ts` from one description, so that half cannot drift. The
other half is which of the six a name picks, and the phone's is FNV-1a over
`charCodeAt` — UTF-16 code units. `art.rs` walks `encode_utf16()` for exactly
that reason, and `wrapping_mul` because `Math.imul` wraps where Rust would
panic in debug.

The test pins it, and the case that matters is the one nothing in the library
has: every name here is in the Basic Multilingual Plane, where one `char` is
one code unit and the two walks are the same function — so "Für Elise" would
not catch `chars()`. U+1F3B5 would: `0x442e75ca` walked as UTF-16 and
`0xb154da50` walked as chars. Pinning it now is cheaper than finding out when
somebody names a playlist with an emoji and one device draws a different
square.

## The media directory is a peer, and a rescan is free

`services.harken.mediaPath` points at a directory — `/srv/media` by default,
created with its `music/` on activation. The server walks it at startup,
watches it after, and authors each track as an ordinary mutation.

**One root for every kind, and music lives in `music/` under it.** The
alternative was one directory per kind and one option each, and it is wrong
for the same reason `media` is kind-neutral in the schema: `file` is a column
on the kind-neutral side, so its base has to be kind-neutral too. An episode
becomes `podcasts/` beside `music/`, served by the same `/media/` and carried
in the log the same way, with nothing new to configure. The cost is that the
scanner has to *refuse* audio outside `music/` — the watch covers the whole
root, and a podcast has the extension and the tags of a song — which is one
`starts_with` in `offer` and the thing to break if you want to see the test
fail.

**A default has to exist before it can be bound.** `BindReadOnlyPaths` of a
path that is not there fails the unit at startup, so a default nobody had
created yet would mean a server that will not boot until someone makes a
directory. `systemd.tmpfiles.rules` makes it, which is also what makes the
default worth having: install the module, drop files in `/srv/media/music`.
`/var/lib/harken` would *not* have worked — `DynamicUser` puts a
`StateDirectory` under `/var/lib/private`, which is root-only 0700, so the
path the administrator is told about is not the one the files would land in.

**It is a peer, not a writer.** `server/src/library.rs` holds a real
`petros::Client` with its own database and its own pending queue, and reaches
the hub through `Hub::exchange` instead of a socket. Writing rows directly
would have been fewer moving parts and a second definition of what "add a
song" means — and the first time it disagreed with `apply`, the replicas would
diverge with nothing to say so. Being a peer also means the entry carries an
actor and a session like every other: the server mints one for the `library`
account at startup, because "the server wrote it" is not an exemption.

**Idempotency is in `apply`, not in the scanner.** `add_song` refuses a
non-empty `file` the library already has. It has to be there rather than in
the scanner, because the id is chosen fresh per authoring call: a second scan
produces a *different* id for the same path and the id check cannot see it.
Deciding it inside `apply` means every peer replaying reaches the same answer
— the first entry for a path wins, wherever the rescan happened. An empty
`file` does not collide, because a song typed in by hand has no path and two
of those are two songs.

**A default playlist is made by every client, and only one of them survives.**
Each peer makes "Favorites" on its first run, and it has to do that *before*
anything has synced — the list is empty because the log has not arrived, not
because nobody has one. So a person signing in on a phone, a laptop and a
browser tab authored three, each with a fresh id, and all three landed. The
fix is `add_song`'s, in the same place and for the same reason:
`create_playlist` refuses a name that person already has, inside `apply`, so
every peer replaying reaches the same answer and the first entry for a
(person, name) wins wherever it was authored. No client could have caught it —
each of them was right about what it could see.

Three things follow:

- **By person, not by library.** One log is one library and several people may
  be in it, so Bob's "Favorites" is not Alice's. A check on the name alone
  would leave whoever signed in second without the playlist their own client
  had just made for them — a worse bug, and one that only appears on a server
  with two accounts on it.
- **Case is not folded**, deliberately. A name is what somebody typed, and
  deciding that "favorites" is the same word as "Favorites" is deciding what
  they meant. `add_song`'s file check does not do that either.
- **The id a client is holding can now vanish**, because the loser is dropped
  on the rebase. Both clients resolve the playlist their view is read against
  against the *list* rather than remembering the one they chose —
  `reload_sidebar` on the desktop, and the scratch's `kept.playlist` on the
  phone — and re-point the view only when it actually moved, because that
  costs a re-hydrate.

This is the one `SCHEMA_VERSION` bump that is not a shape change. `playlist`
has exactly the columns it had; what moved is what `apply` *means*, and an
app's tables are a function of the log. Without the bump a peer that already
had the duplicates would keep them for ever while a fresh install replayed the
same log into one row — two databases disagreeing, neither of them wrong.
The phone does not pay for the rebuild and does not get it either:
`ForeignApp` leaves `SCHEMA_VERSION` at 0 on purpose, so a phone with
duplicates keeps them until it is reinstalled.

**The log carries the path relative to the media root**, and `/media/` serves
that same path back — so a track reads `music/Bach/air.flac` and plays from
`/media/music/Bach/air.flac`. One string, so a client plays what the scanner
wrote without either knowing where the directory is — and the log does not
freeze this machine's layout into it forever.

Two things the test found that reading the code would not have:

- **A new *directory* is the case that matters, and it is not a new file.**
  Dropping an album folder in creates the directory and its tracks in the same
  breath, and a recursive watch has to add a watch for the new directory
  before it can report anything inside it — so the tracks land in the gap and
  are never announced. What *is* announced is the directory, so a directory
  event walks it rather than trying to index it as a file.
- **`ProtectHome = true` masks `/home` outright**, so a library under there is
  not merely unreadable but invisible, and `ReadOnlyPaths` cannot reach past
  it. The unit uses `BindReadOnlyPaths` for the one directory instead, which
  overrides the mask without opening the rest.

A removal is deliberately not handled. The log is permanent, and a file
disappearing is not evidence anybody meant to delete the song — an unplugged
disk looks exactly the same.

## The keyboard is vim's, and a component opts in by saying what shape it is

`iced/src/vim.rs` is a small library with its own tests and no idea what a
song is. Three things are kept apart, because each is useful without the
others and mixing them is what makes keyboard code untestable:

- **`Keys` turns presses into an `Action`.** It knows `5j` is five downs and
  `gg` is the top, and nothing about what is on screen. Tested by pressing
  letters at it.
- **`Grid` turns a `Motion` into a new cursor.** A component says how its view
  is laid out — how many cells, how many columns — and gets counts, `gg`, `G`
  and `{count}G` for free without ever seeing a key.
- **`main.rs` does the rest** — which pane holds the cursor, what `Activate`
  means, what `/` matches against. None of that generalises, so none of it is
  in `vim.rs`.

There are two panes, the sidebar and the table, and **the now-playing bar is
not one of them**: everything it does has a key of its own (`<Space>`, `{`,
`}`), so making it a third place the cursor can be would only add a stop to
`<Tab>` that nobody needs to pass through. In the sidebar the cursor *is* the
selection — moving onto a row shows it, with no `<Enter>` in between, because
needing a key to confirm what you have already moved onto is a keystroke that
only ever means "yes, that one". `<Enter>` there steps into the table instead.

**`step` returning `None` means "not mine", and that is the whole pane
mechanism.** There is one rule and every shape is written in terms of it, in
`vim::along`: **a step is refused only when the cursor is already at that
edge.** So `5j` three cells from the end goes to the end — there was somewhere
to go — and `j` at the end refuses, because there was not. `Pane::beyond` then
reads a refused `h` as "the sidebar" and a refused `j` as nothing at all.

It did not start that way, and the first version had a real bug in it. A
`List` refused `h` and `l` outright while a `Grid` *clamped* them, on the
reasoning that refusing and clamping were different answers to different
questions — a grid has both axes, so the motion was its own. That holds until
a grid has a caller: the album page was then somewhere the keyboard could walk
into and never walk out of, because `h` at column zero clamped to column zero
for ever. One rule fixes it with no special case — a grid still answers `h` in
the middle of a row and refuses it at column zero, which is exactly the
boundary the sidebar is on the other side of.

**There is one shape, because a list and a row were special cases of it.**
There used to be three — a `List` that refused `h` and `l`, a `Row` that
refused `j` and `k`, and a `Grid` — behind a `Navigate` trait so a caller could
hold whichever it had. All three were the same arithmetic. `Grid::column` is a
grid one wide, where every cell is on the left edge and the right edge at
once, so both horizontal motions refuse without a line of code saying "this
one is a list"; `Grid::row` is a grid one tall and the vertical pair go the
same way. Deleting the other two deleted the trait with them, and a
`Box<dyn Navigate>` per keypress with it — thirty-six lines net, and one fewer
place for two shapes to disagree.

They did disagree, in a small way that came free of being separate: an empty
`List` answered `Some(0)` to `gg` and `G` while an empty `Grid` refused. There
is one answer now, and the empty-shape test asserts it for all three ways of
writing a shape.

**`Grid::progress` is the other half of "the shape knows".** Keeping the
cursor on screen was `at / (cells - 1)` in `App::reveal`, which is right for a
column and wrong for a grid — six cells share one row and only rows can be
scrolled past, so the first card of the last row of forty-in-six scrolled to
92% of the way down instead of to the end. The shape answers now, with the
single-column version as the default so a new shape that *is* one gets it
free.

**Hovering moves the cursor, in the content panes only.** One highlight,
whether you got there with the mouse or with `j` — and it takes the keyboard
as well as the highlight, because the cursor is only *drawn* in the pane that
has it, so a hover that moved an undrawn cursor would look like nothing
happening and then the next `j` would jump from wherever the mouse had been.

Deliberately not the sidebar. There the cursor *is* the selection — moving
onto a line shows it, with no `<Enter>` in between — so hovering would
navigate on the way past, and a sidebar highlight that did *not* navigate
would be a second meaning for the one highlight that pane has.

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

## Both clients maintain their list; only one of them re-reads anything

`iced` holds a `petros::ivm::View`, hydrates it once at boot, and
splices its `Vec<Item>` from the patches the view reports. A tap costs the rows
that moved rather than the whole library — flat, where re-reading grew:
`nix run .#latency` prints the numbers and the decisions below explain them.

The phone does the same thing across the bridge. `Peer` holds the views, the
macro settles them after every mutation and every frame, and `libraryUpdate()`
hands back the rows that moved — seventy bytes, flat, where the whole list was
seventy kilobytes and growing. `mobile/src/peer.ts` splices a list held in the
session's `scratch`, which is where it has to live: the view reports what moved
*since it was last asked*, so a list held in a component starts empty on the
second mount and then receives patches against a list nobody has.

What is *not* maintained, in either client, is the list a playlist, an album or
an artist is showing. That is one query per change and deliberately so: the
maintained view is the list you are looking at most of the time, and re-reading
a single album when something moves is a hundred rows rather than the library.
Both clients do it at the same moment for the same reason — `reload_shown` on
the desktop, and the selection folded into the phone's `query`, re-run by a
`run(() => {})` when a chip is tapped.

A reader is what makes the second one subtle: the phone's read happens inside
`usePeer`'s `query`, which `run` calls synchronously, *before* React has
re-rendered. So the selection is held in a ref beside its state — state alone
answers with the selection that was just replaced. Reading it through the hook's
`run` from a `useMemo` instead does something worse: `run` snapshots, the
snapshot is a new object, the memo's dependency moves, and it reads again,
forever.

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
- **The phone plays, and the platform is what plays it.** `expo-audio` gets a
  URL and gives back streaming, buffering, range requests, seeking and a lock
  screen — the same trade `iced/src/player.rs` makes with an `<audio>` element,
  and the same reason: those are five problems that would each have to be
  solved again in a language that is not the platform's. `mobile/src/player.tsx`
  is the provider, and it is at the *root* rather than on the library screen
  because `useAudioPlayer` releases its player when the component holding it
  unmounts, and music that stops when you leave a screen is not music.
  What crosses from the library is a `Track` and a queue, and the queue is a
  snapshot of what was on screen when play was pressed — the desktop's rule,
  for the desktop's reason: a list that is a live reference gets silently
  redirected by somebody else's edit arriving.
- **`media.file` is a path, so the client joins it.** The column has always
  been "the file in the media store… the bytes travel over HTTP and only the
  name of them is synced", and the demo's Wikimedia recordings are absolute
  URLs, which is why the desktop can hand the string straight to an element and
  get away with it. `mobile/src/media.ts` is the one place that decision is
  written down: an absolute URL is already an answer, anything else is resolved
  against the server this peer is pointed at, and a peer working alone has no
  URL for a relative name and says so rather than handing the player something
  it will fail on in silence.
- **A sidebar becomes two rows of chips, and nothing else moves.** The phone
  shows the same four choices the desktop's sidebar does, in the same order,
  from the same four queries — headings on the first row, what is under the
  chosen heading on the second. That the grouping lives in the domain is what
  makes this a rearrangement rather than a second design: a client that folded
  the library itself would fold it differently, and the two would disagree
  about what an album is the first time one of them met a track with two
  artists.
- **No component library, and the animations are the ones already pinned.**
  A kit was the obvious answer and it is the wrong one twice over: what makes
  this look like a music app is a gold palette, a bar that expands and a bar
  that can be dragged, none of which a Material or Tailwind kit gives you — and
  every one of them arrives as a babel plugin, a metro wrapper, a peer-version
  range against Reanimated 4, or all three, in a tree where a dependency change
  moves two pinned hashes and, for a native one, a recorded Maven graph. So the
  animations are `react-native-reanimated` and `react-native-gesture-handler`
  directly, which every such kit is a wrapper over and which `expo-router`
  already requires; the icons are `expo-symbols`, which was already here; and
  the palette is `mobile/src/theme.ts`, which is forty lines. The two
  dependencies that were added are the two that buy something nothing here can
  do: `expo-audio` and `expo-linear-gradient`.
- **A heart was an answer to a question the domain does not ask.** It meant
  "on the playlist this client happens to be showing membership for", which
  is one playlist out of however many exist, chosen by the client and invisible
  in the UI. So both clients lost it. The phone has a sheet instead: every
  playlist, each ticked or not, and "New playlist" at the end — which is the
  question people were actually asking, and the only shape that lets a track
  be on two lists. The desktop has neither; its one column is the transport.

  Three things it needed:

  - **A `playlists_of` query, so it can be a toggle.** A list of every
    playlist with nothing marked is a list you can add the same track to twice
    and never take it off. It is one query for one track, asked when the sheet
    opens — carrying every track's memberships in the shelf would make every
    change in the app pay for something only that sheet asks.
  - **`run` is the only way into the client, so a read borrows it.**
    `@petros/client`'s hook exposes `run(f)` and nothing else, and it calls
    `f` synchronously — so `playlistsOf` writes into a local and returns it.
    The cost is one render and a `lastMutationMs` that timed a query.
  - **Creating a playlist does not add to it.** `create_playlist` and
    `add_to_playlist` are two entries, and the first one's id is not known
    until it has been applied — so the new row appears in the sheet and is one
    tap away, rather than the sheet pretending to know an id it cannot have.
- **The gold is not one color.** Every background and every text color comes
  from `theme.ts` and none from a component, which is the phone's version of
  the rule that makes the desktop work in dark mode — but the two themes do not
  share the accent. A dark sheet can take a bright leaf gold; a white one
  cannot, because `onAccent` has to be legible *on* it and nothing is legible
  on bright gold. That asymmetry is the only one, and it is in the branding so
  that it stays the only one.
- **The greys are AppKit's, because the program this looks like is Music.app.**
  Every neutral in `branding/nix/palette.nix` is now a named macOS system
  color rather than one chosen here — `controlBackgroundColor` for the track
  list, `underPageBackgroundColor` for the sidebar, `windowBackgroundColor`
  for the toolbar, `unemphasizedSelectedContentBackgroundColor` for a
  selection that has lost the keyboard, and the three ranks of `labelColor`
  for the three ranks of text. Each is written beside its value so the next
  person can check it against Apple instead of against taste.

  What is *not* Apple's is the accent, and that is the point of taking the
  rest: `controlAccentColor` is the system tint, which on a stock Mac is blue
  and which Music.app fills its selected row and playing indicator with — and
  that slot is the gold. Making the neutrals somebody else's decision is what
  leaves the gold as the only thing in the window that is ours.

  Three things worth knowing:

  - **Some of these are alphas, and a generated palette cannot hand a client
    a rule.** `labelColor` is white at 85%, `separatorColor` white at 10%;
    what is in the file is each of them flattened over the plane it is
    actually drawn on. So `text` on dark is `#DDDDDD` rather than `#FFFFFF`,
    which is what a track title in Music.app really is, and `border` is one
    value where AppKit gets a different one per plane.
  - **The zebra lands on macOS's alternating row by arithmetic.** `row_style`
    draws the odd row as `text` at 4.5% over `background`, which on this pair
    is `#272727` dark and `#F5F5F5` light — against AppKit's own
    `alternatingContentBackgroundColors`, about `#252525` and `#F4F5F5`.
    Nobody tuned that. It falls out of using Apple's plane and Apple's label
    together, and it is the tell that the two are quoted right.
  - **The phone gets macOS's greys, not iOS's, and they are not the same
    family.** iOS's page is `#000000` and its greys are cool (`#1C1C1E`,
    `#2C2C2E`); AppKit's page is `#1E1E1E` and its greys are neutral. One
    description for two clients means one of them is quoting the other
    platform's system colors, and this is the direction that was asked for.

  The app icon did **not** follow. Its ground is a gradient from `#1C1811`,
  and `#1C1811` to `#1E1E1E` is two shades of one dark with no visible fade,
  where `#1C1811` to `#050505` is the warm-to-black it was drawn as — so
  `branding/nix/icon.nix` names its own `night` now. A launcher composites an
  icon against a wallpaper rather than against the track list, so it never had
  to agree with the window; it agreed by accident, and that is what ended.

- **One description of what the program looks like, and it is not in either
  client.** The desktop asked iced for its Light and Dark and the phone kept
  its own tables, so "the gold" was two golds that were equal only while
  somebody remembered both — and the app icons were still Expo's template
  blue, which is a third answer nobody chose. `branding/` is the one
  description now: `nix/palette.nix` is the colors and `nix/icon.nix` is the
  mark, and the two clients get *generated* files. `iced/src/palette.rs`,
  `mobile/src/palette.ts` and `iced/web/favicon.svg` are checked the way
  `README.md` is, so a change to one client's colors that is not a change to
  the branding fails `nix flake check` rather than shipping.

  Three things worth knowing about how it is built:

  - **iced still picks the theme; we only pick the colors.** A fully custom
    `iced::Theme` is what you would reach for, and it cannot follow the
    system: iced resolves the preference internally and surfaces it only by
    handing the chosen theme to each style closure. So the closures ask
    `palette::of(theme)`, which reads `is_dark` off what iced picked and
    answers with ours. The cost is that iced's *own* widget defaults — a
    `button::text`, a bare `slider` — keep iced's colors; closing that means
    a colour-scheme dependency (`dark-light`, or `mundy`, which iced already
    carries) and a moved `cargoVendorHash`.
  - **The mark is vendored, not drawn.** Several hand-drawn angels were tried
    and an icon set's trumpet is better than any of them. Pictogrammers' MDI
    glyph, Apache 2.0, unmodified, with its licence beside it — the same
    rule the demo's recordings follow: take it on the terms offered, and say
    so where it can be seen.
  - **A generated Rust file has to come out of nix already formatted.**
    `check-fmt` runs over the whole workspace and does not care that nobody
    typed `palette.rs` — rustfmt orders `theme::palette::Extended` before
    `theme::Palette`, and emitting the other order fails the check on a file
    the fix for is in `branding/nix/`. It hides, too: running `cargo fmt`
    locally rewrites the file, so the next `--check` passes against something
    nix would not have written. Generate it, *then* `cargo fmt --all --check`
    without writing first.
  - **The rasters are not checked files.** `files` compares strings and a PNG
    is bytes, so `nix run .#icons` writes them into `mobile/assets/images/`
    the way `nix run .#mutators` writes the module. The favicon is SVG, so it
    *is* checked — and it carries both themes in a media query, because a tab
    strip is light or dark and the page is never told which.
