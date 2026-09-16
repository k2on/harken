# The mark, and every raster made from it.
#
# The trumpet is not drawn here: `../trumpet.svg` is Pictogrammers' Material
# Design Icons glyph, vendored unmodified under Apache 2.0 with its licence
# beside it in `../LICENSE.trumpet`. Drawing one by hand was tried first and
# the honest result is that an icon set's trumpet is better than mine — and
# vendoring one is a file and a licence rather than a drawing to maintain.
#
# What is here is everything around it: the angle, the optical centring, the
# ground, and the sizes each platform asks for. One glyph, composed per use.
{
  perSystem = { pkgs, lib, config, script, ... }:
    let
      p = config.branding.palette;

      # The path data out of the vendored file. It is one line and changes
      # only when someone re-vendors it, and a mismatch is an eval error
      # naming this line rather than a silently blank icon.
      glyph =
        let raw = lib.removeSuffix "\n" (builtins.readFile ../trumpet.svg);
        in builtins.elemAt (builtins.match ''.*<path d="([^"]*)".*'' raw) 0;

      # Measured rather than guessed. The glyph is centred on the pixels it
      # actually inks and not on its 24×24 box — the trumpet sits low and left
      # in that box, so centring the box leaves the mark visibly off-centre.
      # These put the inked bounds in the middle of a 512 canvas at 60% of its
      # width, turned to the angle below.
      angle = "-18";
      centre = { x = 256 - 8; y = 256 + 6; };
      # Strings rather than floats because `toString 13.1` is "13.100000",
      # and this file is compared against a committed copy character for
      # character.
      scale = "13.1";
      # Android crops an adaptive icon's foreground to the middle 66%, so the
      # mark is smaller there or a launcher nobody tested on cuts off the bell.
      safeScale = "8.646";

      # The icon's ground, which used to end at `p.dark.bg` and now does not.
      #
      # A gradient needs somewhere to go. `#1C1811` to `#050505` is a warm
      # near-black fading to black and you can see it; `#1C1811` to AppKit's
      # `#1E1E1E` is two shades of the same dark and the gradient disappears —
      # which is the whole thing an icon has that a flat rectangle does not.
      # The two were one color while the window's background was a near-black
      # chosen here, and they stopped being one when the window took Music's.
      # A launcher composites this against a wallpaper anyway, not against the
      # track list, so it never had to agree with the window in the first
      # place; it agreed by accident, and that is what ended.
      night = { from = "#1C1811"; to = "#050505"; };

      mark = { fill, at ? scale }: ''
        <g transform="translate(${toString centre.x} ${toString centre.y}) rotate(${angle}) scale(${at}) translate(-12 -12)"><path fill="${fill}" d="${glyph}"/></g>'';

      # A vertical gradient rather than a flat fill, which is the one thing
      # every icon on a home screen has and a flat rectangle does not.
      ground = { from, to }: ''
        <defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${from}"/><stop offset="1" stop-color="${to}"/></linearGradient></defs>
        <rect width="512" height="512" fill="url(#g)"/>'';

      svg = body: ''
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="512" height="512">
        ${body}
        </svg>
      '';

      # The favicon carries both themes itself. A tab strip is light or dark
      # and the page is never told which, so the file answers with a media
      # query and one file is right on either.
      favicon = svg ''
        <style>
          path { fill: ${p.light.accent} }
          @media (prefers-color-scheme: dark) { path { fill: ${p.dark.accent} } }
        </style>
        ${mark { fill = p.dark.accent; }}'';

      sheets = {
        # The app icon: the mark on its ground, one per theme.
        icon-dark = svg (ground night + mark { fill = p.dark.accent; });
        icon-light = svg (ground { from = p.light.bg; to = "#EFEADD"; }
          + mark { fill = p.light.accent; });
        # Android's adaptive icon is two layers, and the foreground is cropped.
        adaptive-foreground = svg (mark { fill = p.dark.accent; at = safeScale; });
        adaptive-background = svg (ground night);
        # …and a monochrome layer, which the launcher tints itself.
        adaptive-monochrome = svg (mark { fill = "#FFFFFF"; at = safeScale; });
        # The splash has no ground: Expo paints one behind it.
        splash = svg (mark { fill = p.dark.accent; at = safeScale; });
      };

      file = name: pkgs.writeText "${name}.svg" sheets.${name};

      # resvg rather than a browser: it is a renderer and nothing else, so the
      # derivation is small and the answer is the same on every machine.
      icons = pkgs.runCommand "harken-icons" { nativeBuildInputs = [ pkgs.resvg ]; } ''
        mkdir -p $out/mobile
        png() { resvg "$1" "$2" --width "$3" --height "$3"; }

        png ${file "icon-dark"}           $out/mobile/icon.png                         1024
        png ${file "adaptive-foreground"} $out/mobile/android-icon-foreground.png      1024
        # The development build's banner is drawn at 1024 and composited at the
        # origin, so on a smaller foreground it lands off the canvas and
        # nothing appears. The same image, under the name that needs it.
        png ${file "adaptive-foreground"} $out/mobile/android-icon-foreground-1024.png 1024
        png ${file "adaptive-background"} $out/mobile/android-icon-background.png      1024
        png ${file "adaptive-monochrome"} $out/mobile/android-icon-monochrome.png      1024
        png ${file "splash"}              $out/mobile/splash-icon.png                   512
        png ${file "icon-dark"}           $out/mobile/favicon.png                        48

        cp ${file "icon-dark"}  $out/icon-dark.svg
        cp ${file "icon-light"} $out/icon-light.svg
      '';
    in
    {
      packages.icons = icons;

      # The favicon is text, so it is committed and checked like the palettes.
      # The rasters below cannot be: they are bytes, and `files` compares
      # strings — so they are written into the tree by the app below, the way
      # the mutator module is.
      files.file."iced/web/favicon.svg".text = favicon;

      apps.icons.program = script "icons" {
        text = ''
          cp -f ${icons}/mobile/*.png mobile/assets/images/
          chmod u+w mobile/assets/images/*.png
          echo "wrote mobile/assets/images/*.png from ${icons}"
        '';
      };
    };
}
