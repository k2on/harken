# The typeface, copied to where each client can reach it.
#
# `branding/font/` is the one vendored copy — Inter, SIL OFL 1.1, with its
# licence beside it, the same rule `trumpet.svg` and `LICENSE.trumpet` follow:
# take it on the terms offered and say so where it can be seen.
#
# The copies exist because the two clients are built from two narrow source
# trees and neither of them contains `branding/`. `engineSrc` is `domain`,
# `server`, `iced`, `Cargo.toml` and `Cargo.lock`, so an `include_bytes!`
# reaching `../../branding` compiles on a laptop and not in the sandbox; and
# `expo prebuild` links fonts from paths under `mobile/`. So this copies, the
# way `nix run .#icons` copies the rasters, and for the same reason: `files`
# compares strings and a TTF is bytes.
#
# **Which weights each client gets is what it actually draws, not all four.**
# The phone writes `600`, `700` and `800` and takes the fourth for body text.
# iced names no weight anywhere — it draws one face — and a wasm module is
# pushed over the wire, so three faces nothing draws would be 1.2 MB of
# bundle for a future that has not arrived. Add the face the day something
# asks for the weight.
{
  perSystem = { pkgs, script, ... }:
    let
      font = ../font;

      fonts = pkgs.runCommand "harken-fonts" { } ''
        mkdir -p $out/iced $out/mobile
        cp ${font}/Inter-Regular.ttf $out/iced/
        cp ${font}/Inter-Regular.ttf ${font}/Inter-SemiBold.ttf \
           ${font}/Inter-Bold.ttf ${font}/Inter-ExtraBold.ttf $out/mobile/
      '';
    in
    {
      packages.fonts = fonts;

      apps.fonts.program = script "fonts" {
        text = ''
          cp -f ${fonts}/iced/*.ttf iced/assets/
          cp -f ${fonts}/mobile/*.ttf mobile/assets/fonts/
          chmod u+w iced/assets/*.ttf mobile/assets/fonts/*.ttf
          echo "wrote iced/assets and mobile/assets/fonts from ${fonts}"
        '';
      };
    };
}
