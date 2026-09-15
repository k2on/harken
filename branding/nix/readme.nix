# The branding's part of README.md: what is in the directory, and what it
# writes into the two clients.
{
  perSystem.readme.sections.branding = {
    order = 50;
    text = ''
      ## Branding

      One description of what the program looks like, in `branding/`: the
      palette, and the mark every icon is made from.

      Black and white with gold — `#000000` behind a dark theme, `#FFFFFF`
      behind a light one, and one accent that means "this is playing", "this
      is hearted", "press this". The two golds are not the same gold: a dark
      sheet can take a bright leaf, a white one needs a darker, browner gold
      or nothing is legible on it.

      Neither client reads a color at run time — one is Rust, the other is
      TypeScript — so both are generated from `branding/nix/palette.nix`:

      ```
      iced/src/palette.rs     the six colors iced generates a theme from
      mobile/src/palette.ts   the same, as the two tables `theme.ts` picks from
      iced/web/favicon.svg    the mark, carrying both themes in a media query
      ```

      They are checked files like `README.md`: `nix run .#write-files` writes
      them and `nix flake check` fails while a committed copy differs, which
      is what stops the desktop and the phone drifting to two different golds.

      The mark is a trumpet — Pictogrammers' Material Design Icons glyph,
      vendored unmodified under Apache 2.0 with its licence in
      `branding/LICENSE.trumpet`. Everything around it is here: the angle, the
      optical centring on the pixels it inks rather than on its box, the
      gradient ground, and the size each platform asks for.

      ```
      nix build .#icons     # every SVG and PNG, in the store
      nix run .#icons       # …and written into mobile/assets/images/
      ```

      The rasters are written into the tree rather than consumed from the
      store because Expo reads them off disk before nix is involved. They
      cannot be checked files the way the palettes are: `files` compares
      strings and a PNG is bytes.
    '';
  };
}
