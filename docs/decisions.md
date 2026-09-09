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
