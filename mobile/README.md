# harken · expo

A peer of the same Petros server the desktop client joins, on a phone. The
first screen asks which server to talk to and signs you in there — through a
browser sheet at whatever provider the server uses, coming back on
`harken://` — and the second is the music library. Who you are is what the
server says; `src/auth.ts` is the whole of it here.

## What the library screen is

The desktop client's arrangement, in a phone's idiom. Its sidebar — Library,
then the playlists, then the albums, then the artists — is two rows of chips
here: the headings on the first, what is under the chosen heading on the
second. Its table of Name, Artist, Album and Time is a stacked row. Its
now-playing bar is a bar above the bottom inset that expands, on a tap or a
drag, into a full-screen player with the seek bar and the heart.

Tapping a row plays it and makes the list on screen the queue, exactly as the
desktop does: a snapshot taken when play was pressed, so changing what is
shown — or somebody else's edit arriving — cannot silently redirect what plays
next.

There is nothing here that puts a track *into* the library, for the same
reason the desktop has nothing: the server's media directory is scanned, so a
client that types songs in is answering a question nobody asks. The heart
stays, because which playlist something is on is still a client's to say.

The grouping is not this directory's idea of how to fold a library. `albums`,
`artists`, `playlists`, `album` and `artist` are queries in `../domain`, and
both clients ask them rather than each inventing a way to group by album.

## What plays it

`expo-audio` — ExoPlayer on Android, AVPlayer on iOS. `src/player.tsx` hands
it a URL and gets streaming, buffering, range requests and seeking from the
platform, which is the same trade `iced/src/player.rs` makes with an `<audio>`
element in a browser. Neither client decodes anything.

`media.file` is a path and not a URL, so `src/media.ts` is the one place it
becomes something a player can open — the same function as `media_url` in
`iced/src/main.rs`: an absolute URL passes through, and a scanned track's
path is joined to the server's `/media/` and percent-encoded a segment at a
time. Handing the raw path to a player is what that exists to stop; a server
with a single-page fallback answers the wrong path with `index.html` and a
200, so the player is given HTML and reports only that it is unsuitable.

The transport is on the lock screen, which on Android is also what keeps
playback alive in the background past about three minutes. `app.config.ts`
configures the plugin for that and declines the microphone permission, because
this app never records.

## The look

Light and dark, following the system, with a gold accent. `src/theme.ts` is
the only place a color is written down — the same rule the desktop keeps by
asking iced for `extended_palette()` — and the two themes do not use the same
gold, because a bright leaf gold takes no text on a white sheet.

There is no component library. The animations are `react-native-reanimated`
and `react-native-gesture-handler`, both already here because `expo-router`
wants them, and the icons are `expo-symbols`, which was already here too: SF
Symbols on iOS and Google's Material Symbols on Android, from one name each in
`src/ui/icon.tsx`. Every glyph also states a character to fall back to, for the
reason `iced/src/icon.rs` exists — a missing glyph lays out fine and draws
nothing, so the button looks broken rather than unfontable.

## There is no domain logic in this directory

The hand-written TypeScript is a stack of screens, a WebSocket and a pump, and
a theme. Everything else — what a song is, what favouriting means, how `pos`
is computed, what the wire format is, when a mutation is refused, how the
rebase works — lives in `../domain` and reaches here as generated bindings. A
second `apply` in a second language is two definitions of the same thing, and
the first time they disagree the replicas diverge silently. So there is one.

`modules/harken-native/` is generated and gitignored. Do not edit it; change
the Rust and run:

```sh
nix run .#bindings     # regenerate, then typecheck the app against the result
```

That reads the UniFFI metadata out of a host build of the crate, so it needs
no NDK and no Xcode. `nix run .#mutators` rebuilds the wasm module a mutation
lives in and hands it to Metro; `nix run .#mutators-watch` does it on every save.

## Running it

```sh
nix run .#serve                              # the server, on 8787
export ANDROID_HOME=~/Android/Sdk         # your SDK; the devshell does not ship one
nix develop .#android -c nix run .#expo-android
```

Or let nix build the whole APK, SDK and all: `nix build .#apk` at the
repository root. `nix/` here is what only this app says about that build —
its name, its hashes, its EAS profiles — and petros-js's mobile module, pinned
by `@petros/client` in `package.json`, is the rest. `../CLAUDE.md` explains
every part of it.

The app calls into Rust, so **Expo Go cannot load it** — it needs a
development build. `ios/` and `android/` are generated by `expo prebuild`
rather than committed. A development build is `dev.harken.koon.us`, "Harken
Dev", with a banner on its icon; a release build is `harken.koon.us`. Both can
be installed at once.

`eas.json` and `eas-rust.sh` are generated by `nix run .#write-files`: the
Rust half of an EAS build, for a container that has no nix, with the
toolchain, the targets and the engine revision nix names.

## The server address

A phone cannot reach your workstation's `127.0.0.1`. The first screen defaults
to whatever host Metro is served from, which is usually right, and is editable
because sometimes it is not: an Android emulator reaches the host as
`10.0.2.2`, a simulator as `localhost`, a real device by its address on your
network.
