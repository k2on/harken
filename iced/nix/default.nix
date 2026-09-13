# The desktop client. `nix run .#iced alice` is a peer; run it twice with
# different names to watch them sync. `web.nix` beside this is the browser one.
{
  perSystem = { pkgs, lib, crate, script, ... }:
    let
      # What iced dlopens at runtime, and what clippy has to find to compile
      # the client at all.
      icedLibs = with pkgs; [ wayland libxkbcommon libGL vulkan-loader fontconfig ];
    in
    {
      # The runtime libraries go on the RPATH rather than being hoped for.
      packages.harken-iced = crate "harken-iced" {
        nativeBuildInputs = [ pkgs.makeWrapper ];
        buildInputs = icedLibs;
        postInstall = ''
          wrapProgram $out/bin/harken-iced \
            --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath icedLibs}
        '';
      };

      apps.iced.program = script "iced" {
        text = ''cargo run -p harken-iced -- --user "''${1:-alice}" --server "''${2:-127.0.0.1:8787}"'';
      };

      petros.buildInputs = lib.optionals pkgs.stdenv.isLinux icedLibs;
      petros.shell.packages = lib.optionals pkgs.stdenv.isLinux icedLibs;
      petros.shell.hook = lib.optionalString pkgs.stdenv.isLinux ''
        export LD_LIBRARY_PATH="${lib.makeLibraryPath icedLibs}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
        # NixOS keeps the host's GPU drivers under /run/opengl-driver, linked
        # against the host's libwayland. LD_LIBRARY_PATH beats a library's own
        # RUNPATH, so on a machine tracking a newer channel than this flake's
        # pin the wayland above shadows the one Mesa was built for: every Mesa
        # Vulkan driver fails to load, wgpu finds no adapter, and iced quietly
        # falls back to its software renderer. Let the host's own copy win.
        for driver in /run/opengl-driver/lib/libvulkan_*.so; do
          [ -e "$driver" ] || continue
          hostWayland=$(LD_LIBRARY_PATH= ldd "$driver" 2>/dev/null \
            | sed -n 's|.*=> \(.*\)/libwayland-client\.so\.0 .*|\1|p' \
            | head -1)
          if [ -n "$hostWayland" ]; then
            export LD_LIBRARY_PATH="$hostWayland:$LD_LIBRARY_PATH"
            break
          fi
        done
      '';
    };
}
