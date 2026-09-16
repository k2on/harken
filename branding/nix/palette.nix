# The palette, and the only place in this program a color is written down.
#
# Two clients, two languages, one set of colors. The desktop asks iced for
# `theme.extended_palette()` and the phone asks `theme.ts`, and both used to
# decide for themselves what gold meant — so the two were the same only for as
# long as somebody remembered to change both. They are generated from here
# now: `iced/src/palette.rs` and `mobile/src/palette.ts` are written by
# `nix run .#write-files` and checked by `nix flake check`, like `README.md`.
#
# **The greys are AppKit's, because the program this looks like is Music.app.**
# Every neutral below is a macOS system color rather than one chosen here, and
# the semantic ones — the planes, the labels, the separator — are named beside
# their value so the next person can check them against Apple rather than
# against taste. Where a color is one Apple defines as an *alpha* over a
# plane, the value here is that alpha flattened over the plane it is actually
# drawn on, because a generated palette has to hand each client an opaque
# string and cannot hand it a rule.
#
# What is *not* Apple's is the accent. `controlAccentColor` is the system
# tint, which on a stock Mac is blue and which Music.app uses for the playing
# indicator, the selected row and every control it fills — and that slot is
# the gold here. The rest of the scheme is Apple's precisely so that the gold
# is the only thing in the window that is ours.
#
# Two consequences worth stating rather than discovering:
#
# - **The phone gets macOS's greys, not iOS's.** They are not the same family:
#   iOS's page is `#000000` and its greys are cool (`#1C1C1E`, `#2C2C2E`),
#   AppKit's page is `#1E1E1E` and its greys are neutral. One description for
#   two clients means one of them is quoting the other platform's system
#   colors, and this is the direction that was asked for.
# - **The zebra lands on macOS's alternating row by arithmetic.** The desktop
#   draws the odd row as `text` at 4.5% over `background`, which on these two
#   is `#272727` on dark and `#F5F5F5` on light — against AppKit's own
#   `alternatingContentBackgroundColors`, which are about `#252525` and
#   `#F4F5F5`. Nobody tuned that; it falls out of using Apple's plane and
#   Apple's label together, and it is the tell that the pair is right.
{ lib, flake-parts-lib, ... }: {
  options.perSystem = flake-parts-lib.mkPerSystemOption {
    options.branding.palette = lib.mkOption {
      type = lib.types.attrsOf (lib.types.attrsOf lib.types.anything);
      description = "The colors, per theme: `dark` and `light`.";
    };
  };

  config.perSystem.branding.palette = {
    dark = {
      # The planes, in the order AppKit stacks them. `bg` is the track list
      # and `card` is the toolbar the now-playing capsule sits in, which is
      # why they are two steps apart rather than one.
      bg = "#1E1E1E"; # controlBackgroundColor / textBackgroundColor
      raised = "#282828"; # underPageBackgroundColor — the sidebar's plane
      card = "#323232"; # windowBackgroundColor — the toolbar's
      cardHigh = "#464646"; # unemphasizedSelectedContentBackgroundColor
      # separatorColor is white at 10%, so it lands on `#353535` over the list
      # and `#464646` over the toolbar. One opaque value has to sit between.
      border = "#3A3A3A";
      # The three label ranks, flattened over `bg`. `text` is *not* white:
      # `labelColor` in Dark Mode is white at 85%, and a track title in
      # Music.app is drawn with it.
      text = "#DDDDDD"; # labelColor — white 85%
      dim = "#9A9A9A"; # secondaryLabelColor — white 55%
      faint = "#565656"; # tertiaryLabelColor — white 25%
      # The gold, standing where `controlAccentColor` does: what is playing,
      # what is hearted, what to press. The one color here Apple did not pick.
      accent = "#E9BB45";
      accentSoft = "#433A25"; # …at 18% over `bg`, which is what a tint is
      onAccent = "#16120A";
      danger = "#FF453A"; # systemRed, dark
      good = "#32D74B"; # systemGreen, dark
      scrim = "rgba(0,0,0,0.55)";
      # The three stops of the now-playing gradient, top to bottom: the accent
      # at a quarter, at a twelfth, and then the plane itself. Music.app takes
      # this from the artwork; nothing in the log carries any, so it is gold.
      glow = [ "#514528" "#2E2B21" "#1E1E1E" ];
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
      bg = "#FFFFFF"; # controlBackgroundColor / textBackgroundColor
      raised = "#F4F5F5"; # alternatingContentBackgroundColors — the second one
      card = "#ECECEC"; # windowBackgroundColor
      cardHigh = "#DCDCDC"; # unemphasizedSelectedContentBackgroundColor
      border = "#E5E5E5"; # separatorColor — black 10%, over the list
      text = "#262626"; # labelColor — black 85%
      dim = "#808080"; # secondaryLabelColor — black 50%
      faint = "#BDBDBD"; # tertiaryLabelColor — black 26%
      # Darker and browner than the dark theme's, so `onAccent` can be read on
      # it. A leaf gold takes no text at all. This is the one asymmetry here,
      # and it is the reason the light theme cannot simply be the dark one
      # inverted the way Apple's tints are.
      accent = "#9A741A";
      accentSoft = "#F0EADD"; # …at 15% over `bg`
      onAccent = "#FFFDF6";
      danger = "#FF3B30"; # systemRed, light
      good = "#28CD41"; # systemGreen, light — macOS's, which is not iOS's
      scrim = "rgba(0,0,0,0.35)";
      glow = [ "#E6DCC6" "#F7F4ED" "#FFFFFF" ];
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
