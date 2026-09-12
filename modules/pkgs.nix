# nixpkgs with the Rust overlay, and what every package module reads from it.
#
# One `pkgs` for the whole flake, and for everything imported into it:
# `petros-js`, `expo.nix` and `android.nix` each build their library over
# *this* package set rather than their own nixpkgs, so there is one copy of
# nixpkgs in the closure and the SDK is composed from the same one as the
# server.
{ inputs, ... }: {
  perSystem = { system, ... }:
    let
      pkgs = import inputs.nixpkgs {
        inherit system;
        overlays = [ (import inputs.rust-overlay) ];
      };

      # The same toolchain the devshell uses, from the same file, so a
      # package and a `cargo build` inside `nix develop` are the same build.
      toolchain = pkgs.rust-bin.fromRustupToolchainFile ../rust-toolchain.toml;
    in
    {
      _module.args = {
        inherit pkgs toolchain;
        rustPlatform = pkgs.makeRustPlatform {
          cargo = toolchain;
          rustc = toolchain;
        };

        # What iced dlopens at runtime, and what clippy has to find to compile
        # the client at all.
        icedLibs = with pkgs; [
          wayland
          libxkbcommon
          libGL
          vulkan-loader
          fontconfig
        ];
      };
    };
}
