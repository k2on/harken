# The palette, written out in the two languages that have to agree about it.
#
# Neither client reads a color from here at run time — one is Rust compiled to
# a binary and a wasm module, the other is TypeScript pushed by Metro — so the
# only way one description can serve both is to generate both. These are files
# like `README.md` and `eas.json`: `nix run .#write-files` writes them, and
# `nix flake check` fails while a committed copy differs from what nix would
# write, which is what stops the two drifting apart the usual way.
{
  perSystem = { config, lib, ... }:
    let
      p = config.branding.palette;

      # `#E9BB45` as the three bytes Rust wants. iced takes a `Color`, not a
      # string, so the hex has to be split somewhere and nix is the only place
      # that already knows it.
      rgb = h: "0x${lib.substring 1 2 h}, 0x${lib.substring 3 2 h}, 0x${lib.substring 5 2 h}";

      # The six colors iced generates a whole theme from. `warning` is the
      # accent: gold already means "look at this", and a second warning color
      # would be a color nothing here chose.
      icedPalette = t: lib.concatStringsSep "\n" [
        "    background: rgb(${rgb t.bg}),"
        "    text: rgb(${rgb t.text}),"
        "    primary: rgb(${rgb t.accent}),"
        "    success: rgb(${rgb t.good}),"
        "    warning: rgb(${rgb t.accent}),"
        "    danger: rgb(${rgb t.danger}),"
      ];

      # The six stand-in gradients, as pairs of stops. `iced::art` picks one
      # by hashing a name; the phone picks the same one out of the same six in
      # `mobile/src/palette.ts`, which is what makes an album the same square
      # on a laptop and on a phone.
      icedArt = t: lib.concatMapStringsSep "\n"
        (a: "    [rgb(${rgb (builtins.elemAt a 0)}), rgb(${rgb (builtins.elemAt a 1)})],")
        t.art;

      quoted = xs: lib.concatMapStringsSep ", " (x: "'${x}'") xs;
      pair = xs: "[${quoted xs}]";

      tsTheme = name: dark: t: lib.concatStringsSep "\n" ([
        "export const ${name} = {"
        "  dark: ${lib.boolToString dark},"
      ] ++ map (k: "  ${k}: '${t.${k}}',") [
        "bg"
        "raised"
        "card"
        "cardHigh"
        "border"
        "text"
        "dim"
        "faint"
        "accent"
        "accentSoft"
        "onAccent"
        "danger"
        "good"
        "scrim"
      ] ++ [
        "  glow: ${pair t.glow},"
        "  art: ["
      ] ++ map (a: "    ${pair a},") t.art ++ [
        "  ],"
        "} as const;"
      ]);
    in
    {
      files.file = {
        "iced/src/palette.rs".text = ''
          //! The palette, generated from `branding/nix/palette.nix`.
          //!
          //! Do not edit: `nix run .#write-files` writes this and `nix flake
          //! check` fails while it differs. The colors live next door to the
          //! icon that uses the same gold, and reach the phone as
          //! `mobile/src/palette.ts` from the same description.
          //!
          //! libcosmic picks Light or Dark and hands every style closure the
          //! theme it picked; [`of`] reads which of the two that was and
          //! answers with ours. So the rule that makes dark mode work is
          //! unchanged — nothing writes a color down, it asks — and what it
          //! asks is this file rather than the toolkit's own.
          use cosmic::iced::theme::palette::Extended;
          use cosmic::iced::theme::Palette;
          use cosmic::iced::Color;
          use cosmic::Theme;
          use std::sync::OnceLock;

          const fn rgb(r: u8, g: u8, b: u8) -> Color {
              Color::from_rgb8(r, g, b)
          }

          /// Black, white text, gold.
          pub const DARK: Palette = Palette {
          ${icedPalette p.dark}
          };

          /// …and the same, against white. The gold is darker here because
          /// text has to be legible *on* it.
          pub const LIGHT: Palette = Palette {
          ${icedPalette p.light}
          };

          /// Stand-in artwork, as the two stops of a gradient.
          ///
          /// Nothing in the log carries a cover, and nothing should: `media`
          /// has a title, a creator, a length and the name of a file, and a
          /// picture would be a column every kind pays for so that one kind
          /// can have one. So a record's art is *derived* from its name, in
          /// [`crate::art`] — deterministically, so the same album is the same
          /// square on every device, which is most of what a cover does in a
          /// list.
          pub const DARK_ART: [[Color; 2]; 6] = [
          ${icedArt p.dark}
          ];

          /// …and against white, where the pair has to stay dark enough that
          /// the note drawn on it reads.
          pub const LIGHT_ART: [[Color; 2]; 6] = [
          ${icedArt p.light}
          ];

          /// Ours, for whichever of the two libcosmic picked.
          ///
          /// Generated once each rather than per widget per frame: `Extended`
          /// is forty-odd colors derived from six, and deriving them at
          /// sixty hertz would be forty-odd divisions nobody asked for.
          pub fn of(theme: &Theme) -> &'static Extended {
              static DARK_EXT: OnceLock<Extended> = OnceLock::new();
              static LIGHT_EXT: OnceLock<Extended> = OnceLock::new();

              if theme.theme_type.is_dark() {
                  DARK_EXT.get_or_init(|| Extended::generate(DARK))
              } else {
                  LIGHT_EXT.get_or_init(|| Extended::generate(LIGHT))
              }
          }
        '';

        "mobile/src/palette.ts".text = ''
          /**
           * The palette, generated from `branding/nix/palette.nix`.
           *
           * Do not edit: `nix run .#write-files` writes this and `nix flake
           * check` fails while it differs. `theme.ts` beside it is the hook
           * and the type; this is only the colors, and the desktop client
           * gets the same ones as `iced/src/palette.rs`.
           */
          ${tsTheme "DARK" true p.dark}

          ${tsTheme "LIGHT" false p.light}
        '';
      };
    };
}
