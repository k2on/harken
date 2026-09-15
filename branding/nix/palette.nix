# The palette, and the only place in this program a color is written down.
#
# Two clients, two languages, one set of colors. The desktop asks iced for
# `theme.extended_palette()` and the phone asks `theme.ts`, and both used to
# decide for themselves what gold meant — so the two were the same only for as
# long as somebody remembered to change both. They are generated from here
# now: `iced/src/palette.rs` and `mobile/src/palette.ts` are written by
# `nix run .#write-files` and checked by `nix flake check`, like `README.md`.
#
# Black and white with gold. The background is the actual extreme — `#000000`
# on dark, `#FFFFFF` on light — rather than the warm near-black this had
# before, because a music library is a list of names and the thing that should
# carry color is what is playing, not the sheet under it.
#
# The two golds are not the same gold, and that is the one asymmetry here. The
# dark theme can afford a bright leaf; the light one cannot, because `onAccent`
# has to be legible *on* the accent and nothing is legible on bright gold.
{ lib, flake-parts-lib, ... }: {
  options.perSystem = flake-parts-lib.mkPerSystemOption {
    options.branding.palette = lib.mkOption {
      type = lib.types.attrsOf (lib.types.attrsOf lib.types.anything);
      description = "The colors, per theme: `dark` and `light`.";
    };
  };

  config.perSystem.branding.palette = {
    dark = {
      # The sheet everything sits on, and the three steps up from it.
      #
      # Not `#000000`: a table of rows on absolute black is a void with text
      # in it, and the zebra has nowhere to go but a long way up. A tenth of a
      # step off black still reads as black on any screen and gives the rest
      # of the ramp somewhere to sit.
      bg = "#0A0A0A";
      raised = "#141414";
      card = "#1A1A19";
      cardHigh = "#242422";
      border = "#2B2B29";
      # Text, and the two dimmer ranks of it: an artist, then a hint.
      text = "#FFFFFF";
      dim = "#A1A09C";
      faint = "#6B6A66";
      # The gold: what is playing, what is hearted, what to press.
      accent = "#E9BB45";
      accentSoft = "#2A2210";
      onAccent = "#16120A";
      danger = "#E4685C";
      good = "#5FBE88";
      scrim = "rgba(0,0,0,0.72)";
      # The three stops of the now-playing gradient, top to bottom. The last is
      # `bg`, so the artwork sits in the page rather than on it.
      glow = [ "#3A2E0E" "#16150F" "#0A0A0A" ];
      # Stand-in artwork, as gradients. Nothing in the log carries a cover, so
      # a record's art is derived from its name — and the set is small and all
      # of one family, so twenty of them read as one library.
      art = [
        [ "#7A5C15" "#201907" ]
        [ "#8A6B22" "#1B1509" ]
        [ "#6B5A2A" "#17140B" ]
        [ "#8F7327" "#221B09" ]
        [ "#5E4A18" "#141007" ]
        [ "#A08137" "#261E08" ]
      ];
    };

    light = {
      bg = "#FFFFFF";
      raised = "#FFFFFF";
      card = "#F7F6F3";
      cardHigh = "#EDEBE5";
      border = "#E2E0D9";
      text = "#0A0A09";
      dim = "#5C5A55";
      faint = "#8E8B84";
      # Darker and browner than the dark theme's, so `onAccent` can be read on
      # it. A leaf gold takes no text at all.
      accent = "#9A741A";
      accentSoft = "#F5EDD8";
      onAccent = "#FFFDF6";
      danger = "#B2352A";
      good = "#2E7C51";
      scrim = "rgba(10,10,9,0.38)";
      glow = [ "#F0E3BC" "#FAF7EE" "#FFFFFF" ];
      art = [
        [ "#E8D19A" "#C9A85F" ]
        [ "#EFDCAE" "#D2B370" ]
        [ "#E2D2AC" "#BFA671" ]
        [ "#F0DFA8" "#CBAA5C" ]
        [ "#E6D0A0" "#C3A05A" ]
        [ "#F3E6BE" "#D6B877" ]
      ];
    };
  };
}
