# The icons, written out in the two languages that have to agree about them.
#
# `branding/icons/` is the one vendored copy — Lucide, ISC, with its licence
# beside it, the rule `trumpet.svg` and `branding/font/` already follow. The
# files there are Lucide's own, normalised once on the way in:
#
#   * the `@license` comment goes, because `LICENSE.lucide` holds it;
#   * `class="lucide …"` goes, because a DOM class means nothing to either
#     client;
#   * `width` and `height` go, because the widget says how big it is drawn and
#     a size in the file is a second answer;
#   * and it is folded to one line, because it is about to be a string literal.
#
# And then one thing that is *not* normalising, which is worth keeping
# separate from the list above because it is a change of look rather than of
# container: every glyph's `stroke-linecap` and `stroke-linejoin` were Lucide's
# `round` and are `butt` and `miter`, and the `rx` on the four glyphs built out
# of rects is gone. Lucide draws with round caps, joins and corners throughout;
# this program does not want that, so the ends are square and the corners are
# sharp. Both are written out explicitly rather than deleted, even though they
# are the SVG defaults — an attribute that is *absent* reads as one the
# vendoring dropped, and dropping attributes is how four glyphs lost their
# geometry the last time. Present and contradicting upstream says it was
# chosen.
#
# Which also means these are modified files, not upstream's. ISC asks for
# nothing on that count; the rule this repository follows about a vendored
# thing — take it on the terms offered and say so where it can be seen — asks
# for the sentence anyway, and this is it.
#
# `currentColor` is deliberately *kept*, which is Lucide's own convention and
# happens to be exactly the rule `iced/src/icon.rs` has always had: a colour
# baked into a glyph is the same colour on a dark row, a light one and the
# accent-filled one under the cursor. iced replaces every colour in the
# drawing through the `svg` style's filter, so what `currentColor` resolves to
# there never matters; `react-native-svg` resolves it from the `color` prop,
# which is the ordinary way to tint an SVG on that side. One placeholder, two
# mechanisms, and neither client writes a colour into a file.
#
# Normalised on the way *in* rather than here on purpose: nix has no general
# text substitution, and a generator that cannot do the transformation cannot
# be checked against the thing it generates. What is in `branding/icons/` is
# exactly what goes in the literals, so this file only wraps it.
#
# Why generate rather than have each client read the SVGs: neither narrow
# source tree contains `branding/`, the same wall `branding/nix/font.nix`
# meets — and Metro has no SVG loader without another transformer. So these
# are files like `palette.rs` and `palette.ts`: `nix run .#write-files` writes
# them, `nix flake check` fails while a committed copy differs.
{
  perSystem = { lib, ... }:
    let
      # What each glyph is called here, and which Lucide icon it is. The names
      # on the left are this program's vocabulary — `note`, `album`, `artist`,
      # `composer` — so a set swapped underneath would move this table and
      # nothing else.
      glyphs = {
        album = "disc-3";
        artist = "user";
        back = "chevron-left";
        chevron = "chevron-right";
        close = "x";
        composer = "users";
        debug = "bug";
        devices = "cast";
        down = "chevron-down";
        home = "house";
        laptop = "laptop";
        library = "library-big";
        addTo = "list-plus";
        more = "ellipsis-vertical";
        next = "skip-forward";
        note = "music";
        offline = "wifi-off";
        online = "wifi";
        pause = "pause";
        phone = "smartphone";
        play = "play";
        playing = "audio-lines";
        playlist = "list-music";
        plus = "plus";
        previous = "skip-back";
        search = "search";
        shuffle = "shuffle";
        signOut = "log-out";
        speaker = "speaker";
        stop = "circle-stop";
        tick = "check";
        ticked = "circle-check";
        untick = "circle";
      };

      names = builtins.attrNames glyphs;
      svg = key: lib.removeSuffix "\n" (builtins.readFile (../icons + "/${glyphs.${key}}.svg"));

      # `SCREAMING_SNAKE` from a camelCase key, for Rust's constants.
      upper = lib.stringToCharacters "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
      shout = key:
        lib.toUpper (builtins.replaceStrings upper (map (c: "_${c}") upper) key);

      # Written as one expression rather than an indented block, because a
      # `''` block's trailing blank line is a byte, and a byte is the whole of
      # what `files` compares.
      rust = lib.concatMapStringsSep "\n\n"
        (key: "/// `${glyphs.${key}}`\npub const ${shout key}: &[u8] = br##\"${svg key}\"##;")
        names;

      ts = lib.concatMapStringsSep "\n" (key: "  ${key}:\n    '${svg key}',") names;
    in
    {
      files.file = {
        "iced/src/glyphs.rs".text = ''
          //! The icons, generated from `branding/icons/`.
          //!
          //! Do not edit: `nix run .#write-files` writes this and `nix flake
          //! check` fails while it differs. The phone draws the same drawings
          //! from `mobile/src/ui/glyphs.ts`, generated from the same files —
          //! which is the whole point of them being here rather than one set
          //! per client.
          //!
          //! Geometry only. What colour a glyph is drawn, how big, and when,
          //! is [`crate::icon`]'s — the `currentColor` in every one of these
          //! is a placeholder the `svg` style's colour filter replaces.
          //!
          //! `dead_code` is allowed because the table is the *program's*
          //! vocabulary rather than this client's: the phone draws `home`,
          //! `search` and a tab bar's worth of glyphs the desktop has no
          //! place for, and generating two subsets would be two tables to
          //! keep in step, which is the thing this replaced.
          #![allow(dead_code)]

          ${rust}
        '';

        "mobile/src/ui/glyphs.ts".text = ''
          /**
           * The icons, generated from `branding/icons/`.
           *
           * Do not edit: `nix run .#write-files` writes this and `nix flake
           * check` fails while it differs. The desktop draws the same drawings
           * from `iced/src/glyphs.rs`, generated from the same files.
           *
           * Geometry only. `icon.tsx` beside it says what colour and how big;
           * the `currentColor` in every one of these is what its `color` prop
           * resolves.
           */
          export const GLYPHS = {
          ${ts}
          } as const;

          export type IconName = keyof typeof GLYPHS;
        '';
      };
    };
}
