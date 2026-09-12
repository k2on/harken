# The three checks, as derivations — fmt, clippy, the suite — and also as
# packages, because `nix flake check` runs them all and says little, while
# `nix build .#check-clippy` runs one and shows what it said.
{
  perSystem = { pkgs, toolchain, rustPlatform, icedLibs, sources, self', ... }:
    let
      inherit (sources) checkWorkspace cargoDeps;
      mutators = self'.packages.mutators;

      # What every check starts from: the narrow tree, the vendored
      # dependencies, and the module staged where `include_bytes!` looks
      # for it.
      checkBase = {
        src = checkWorkspace;
        inherit cargoDeps;
        preBuild = ''
          mkdir -p target/wasm32-unknown-unknown/mutators
          cp ${mutators}/harken.wasm \
            target/wasm32-unknown-unknown/mutators/harken.wasm
        '';
        installPhase = "mkdir -p $out";
        dontFixup = true;
      };

      # Formatting. This is the one the old CI step could not do: `just`
      # runs `fmt` before `lint`, so `cargo fmt --all` rewrote the tree and
      # `cargo fmt --all --check` then measured what it had just written. A
      # badly formatted commit passed.
      check-fmt = pkgs.stdenv.mkDerivation (checkBase // {
        name = "harken-check-fmt";
        nativeBuildInputs = [ rustPlatform.cargoSetupHook toolchain ];
        buildPhase = "cargo fmt --all --check";
      });

      check-clippy = pkgs.stdenv.mkDerivation (checkBase // {
        name = "harken-check-clippy";
        nativeBuildInputs = [
          rustPlatform.cargoSetupHook
          toolchain
          pkgs.pkg-config
        ];
        buildInputs = icedLibs;
        buildPhase = ''
          runHook preBuild
          cargo clippy --workspace --all-features --all-targets --offline \
            -- -D warnings
          runHook postBuild
        '';
      });

      check-tests = pkgs.stdenv.mkDerivation (checkBase // {
        name = "harken-check-tests";
        nativeBuildInputs = [
          rustPlatform.cargoSetupHook
          toolchain
          pkgs.cargo-nextest
          pkgs.pkg-config
        ];
        buildInputs = icedLibs;
        buildPhase = ''
          runHook preBuild
          cargo nextest run --workspace --all-features --offline
          cargo test --workspace --all-features --doc --offline
          runHook postBuild
        '';
      });
    in
    {
      packages = { inherit check-fmt check-clippy check-tests; };
      checks = { inherit check-fmt check-clippy check-tests; };
    };
}
