# The desktop and browser clients' part of README.md: installing them, and
# where the browser one is served from.
{
  perSystem.readme.sections.iced = {
    order = 30;
    text = ''
      ## The desktop client

      The library in a window, on Linux and macOS. It keeps its own database
      and works offline; the server is where it syncs and where it signs in.

      A sidebar down the left browses the library: the playlists, then the
      albums, then the artists. Moving onto one shows it — the cursor there
      is the selection, so nothing needs confirming. The tracks are a table
      of Name, Artist, Album and Time, and a now-playing bar sits along the
      bottom. Light or dark follows the system: every colour comes from the
      theme, so there is none to get wrong, and every icon is drawn rather
      than typed, because the embedded font has no glyph for any of them.

      It is driven from the keyboard, with vim's motions: `j` and `k` move,
      `h` and `l` cross between the sidebar, the track list and the player,
      `5j` and `gg` and `7G` do what they do in vim, `/` searches the pane
      you are in, `<Enter>` opens and `<Space>` hearts. Press `?` for the
      rest.

      The bar sounds nothing on the desktop — there is no audio device wired
      up, and it says so rather than pretending — so playback is the
      browser's today. See `CLAUDE.md` for what closing that would take.

      ```
      nix run github:k2on/harken#harken-iced -- --server https://harken.example.com
      nix profile install github:k2on/harken#harken-iced   # …or keep it around
      harken-iced --server https://harken.example.com
      ```

      On NixOS, as a package:

      ```nix
      environment.systemPackages = [
        inputs.harken.packages.''${pkgs.stdenv.hostPlatform.system}.harken-iced
      ];
      ```

      The first run opens a browser tab at the provider; the login is kept
      under `~/.config/harken/` and the tab is not opened again. Against a dev
      server (`nix run .#serve`) there is no tab: `--user alice` is honoured
      as the name, which is what makes two peers on one laptop two commands.

      ```
      nix run .#serve          # terminal 1
      nix run .#iced alice     # terminal 2
      nix run .#iced bob       # terminal 3 — then take one offline and watch them sync
      ```

      ## The demo

      The client compiled to wasm with no server, no sign-in and a seeded
      library, published to GitHub Pages on every push to `main`. It is the
      real client — the same `apply`, the same maintained view, the same
      rebase — with a feature that removes the parts needing somewhere to
      connect to: there is no sign-in and no going offline, because there is
      nothing to be offline from.

      Its library is public-domain classical recordings streamed from
      Wikimedia Commons, so the bar plays. Pick a composer under Artists, or
      a work under Albums, and press a track.

      ```
      nix build .#harken-demo    # index.html and pkg/, ready to serve
      ```

      A separate build from the real client rather than a runtime flag: a flag
      can be set by accident, a feature has to be asked for.

      ## The browser client

      The same program compiled to wasm, in a page. It is served by the
      server: `services.harken.web` on NixOS, or `HARKEN_WEB` pointed at a
      built one, and the page signs in against the origin it came from.

      ```
      nix build .#harken-web                       # index.html and pkg/, ready to serve
      HARKEN_WEB=result nix run .#serve            # …served by the server, at /
      nix run .#web                                # on a laptop: rebuilt and served on :8080
      ```

      `nix run .#web` serves the page from a port of its own, so it is told
      where the server is: `http://localhost:8080/?server=http://127.0.0.1:8787`.
      Served by the server there is nothing to tell.
    '';
  };
}
