{
  description = "harken — a self-hosted, local-first music system";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
    # Generates the files that would otherwise repeat what this file knows —
    # `Cargo.toml` (the engine's revision), `expo/eas.json` (the Rust version)
    # — and checks the committed copies against them. `flake = false`: it is
    # imported by path.
    files = {
      url = "github:mightyiam/files";
      flake = false;
    };
    # The engine's nix: the crate list, the `[patch]` that makes the lockfile
    # resolvable, the code generator. `Cargo.toml` pins the same revision,
    # generated from this lock.
    petros = {
      url = "github:k2on/petros";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-overlay.follows = "rust-overlay";
      inputs.flake-parts.follows = "flake-parts";
      inputs.import-tree.follows = "import-tree";
    };
    # How a Petros app reaches a phone. Brings expo.nix and android.nix along;
    # every `follows` keeps this closure to one nixpkgs.
    petros-js = {
      url = "github:k2on/petros-js";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-parts.follows = "flake-parts";
      inputs.import-tree.follows = "import-tree";
      inputs.petros.follows = "petros";
    };
    # Not used here; named once so the two above can follow it.
    import-tree.url = "github:vic/import-tree";
  };

  outputs = inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } ({ lib, flake-parts-lib, ... }: {
      # One directory, one module. Each carries the nix for what is in it.
      imports = [
        ./domain/flake-module.nix
        ./server/flake-module.nix
        ./iced/flake-module.nix
        ./expo/flake-module.nix
        "${inputs.files}/flake-module.nix"
        inputs.petros.flakeModules.default
        inputs.petros-js.flakeModules.default
      ];

      # What a directory contributes to the workspace: tools for the devshell,
      # libraries the whole-workspace checks need, a shell hook.
      options.perSystem = flake-parts-lib.mkPerSystemOption {
        options.workspace = {
          packages = lib.mkOption { type = lib.types.listOf lib.types.package; default = [ ]; };
          buildInputs = lib.mkOption { type = lib.types.listOf lib.types.package; default = [ ]; };
          shellHook = lib.mkOption { type = lib.types.lines; default = ""; };
        };
      };

      config.systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];

      config.perSystem = { config, system, self', petros, ... }:
        let
          pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [ (import inputs.rust-overlay) ];
          };

          # The one place the toolchain is named. `expo/eas.json` carries the
          # version too, generated from here, because an EAS container has no
          # nix. Not the crates' MSRV: `rust-version` in `Cargo.toml` is the
          # floor, and this is what development happens on.
          rustVersion = "1.90.0";
          toolchain = pkgs.rust-bin.stable.${rustVersion}.default.override {
            extensions = [ "rustfmt" "clippy" "rust-analyzer" "rust-src" ];
            targets = [
              "wasm32-unknown-unknown"
              "aarch64-linux-android"
              "x86_64-linux-android"
              "aarch64-apple-ios"
              "aarch64-apple-ios-sim"
            ];
          };
          rustPlatform = pkgs.makeRustPlatform { cargo = toolchain; rustc = toolchain; };

          # A program for `nix run`, run from the repository root.
          script = name: { runtimeInputs ? [ ], text }: pkgs.writeShellApplication {
            inherit name;
            runtimeInputs = [ toolchain pkgs.git ] ++ runtimeInputs;
            text = ''
              cd "$(git rev-parse --show-toplevel)"
            '' + text;
          };

          # The trees everything is built from. `cleanSource` filters VCS
          # files, not gitignored ones, so the local `.cargo/config.toml` —
          # whose patch points at a working copy — is excluded by hand; the
          # engine patch pointing at the pinned copy is installed instead.
          # `engineSrc` is the Rust and the module's config, so that a screen
          # edit is not an input to a cross-compile.
          clean = name: keep: lib.cleanSourceWith {
            inherit name;
            src = ./.;
            filter = path: _: keep (lib.removePrefix (toString ./. + "/") (toString path));
          };
          src = clean "harken-src" (rel: !(builtins.elem (baseNameOf rel) [ ".cargo" "target" "node_modules" "result" ]));
          engineSrc = clean "harken-engine-src" (rel: lib.any (w: lib.hasPrefix w rel || lib.hasPrefix rel w)
            [ "Cargo.toml" "Cargo.lock" "domain" "server" "iced" "expo/modules/harken-native" ]);
          patched = name: tree: pkgs.runCommand name { } ''
            cp -r ${tree} $out
            chmod -R u+w $out
            install -Dm444 ${petros.mkCargoPatch inputs.petros} $out/.cargo/config.toml
            if grep -q 'source = "git+https://github.com/k2on/petros' $out/Cargo.lock; then
              echo "Cargo.lock records the engine as a git source: something ran cargo" >&2
              echo "without .cargo/config.toml. Run: git checkout -- Cargo.lock" >&2
              exit 1
            fi
          '';
          sources = {
            workspace = patched "harken-workspace" src;
            engineWorkspace = patched "harken-engine-workspace" engineSrc;
            # The dependencies, vendored by cargo itself — `importCargoLock`
            # and `fetchCargoVendor` send a User-Agent crates.io answers 403
            # to. A fixed-output derivation, so the network is allowed and the
            # result is pinned by hash.
            cargoDeps = pkgs.stdenv.mkDerivation {
              name = "harken-cargo-vendor";
              src = sources.workspace;
              nativeBuildInputs = [ toolchain pkgs.cacert pkgs.git ];
              buildPhase = ''
                export CARGO_HOME=$PWD/.cargo-home
                mkdir -p $out
                cargo vendor --locked --versioned-dirs $out > $out/config.toml
                cp Cargo.lock $out/Cargo.lock
              '';
              dontInstall = true;
              dontFixup = true;
              outputHashMode = "recursive";
              outputHashAlgo = "sha256";
              outputHash = "sha256-z3c9ZDiu29FuDF6JOY7pseb7FXEmppvaYSPRXa2k+bI=";
            };
          };

          # fmt, clippy, the suite — as derivations over the narrow tree, so CI
          # and a laptop run one expression and a check whose inputs have not
          # moved does not run at all. The module is staged first: `foreign_peer!`
          # does `include_bytes!` of it, so compiling with `--all-features` needs
          # the file to exist.
          check = name: { command, tools ? [ ] }: pkgs.stdenv.mkDerivation {
            name = "harken-check-${name}";
            src = sources.engineWorkspace;
            inherit (sources) cargoDeps;
            nativeBuildInputs = [ rustPlatform.cargoSetupHook toolchain pkgs.pkg-config ] ++ tools;
            buildInputs = config.workspace.buildInputs;
            buildPhase = ''
              runHook preBuild
              mkdir -p target/wasm32-unknown-unknown/mutators
              cp ${self'.packages.mutators}/harken.wasm target/wasm32-unknown-unknown/mutators/
              ${command}
              runHook postBuild
            '';
            installPhase = "mkdir -p $out";
            dontFixup = true;
          };
        in
        {
          _module.args = { inherit pkgs toolchain rustVersion rustPlatform script sources; };

          files.file."Cargo.toml".source = pkgs.replaceVars ./Cargo.toml.in { petros = inputs.petros.rev; };
          files.writer.app = true;

          checks = {
            check-fmt = check "fmt" { command = "cargo fmt --all --check"; };
            check-clippy = check "clippy" {
              command = "cargo clippy --workspace --all-features --all-targets --offline -- -D warnings";
            };
            check-tests = check "tests" {
              tools = [ pkgs.cargo-nextest ];
              command = ''
                cargo nextest run --workspace --all-features --offline
                cargo test --workspace --all-features --doc --offline
              '';
            };
          };
          packages = { inherit (config.checks) check-fmt check-clippy check-tests; };

          # The same three on a laptop, against whatever `target/` is lying
          # around. `test` and `lint` stage the module first, for the reason
          # above.
          apps = {
            fmt.program = script "fmt" { text = "cargo fmt --all"; };
            lint.program = script "lint" {
              text = ''
                ${self'.apps.mutators.program}
                cargo clippy --workspace --all-features --all-targets -- -D warnings
                cargo fmt --all --check
              '';
            };
            test.program = script "test" {
              runtimeInputs = [ pkgs.cargo-nextest ];
              text = ''
                ${self'.apps.mutators.program}
                cargo nextest run --workspace --all-features
                cargo test --workspace --all-features --doc
              '';
            };
          };

          devShells.default = pkgs.mkShell {
            packages = [ toolchain pkgs.cargo-nextest config.files.writer.drv ] ++ config.workspace.packages;
            shellHook = config.workspace.shellHook + ''
              echo "harken devshell — nix run .#test | .#lint | .#serve | .#iced | .#web | .#mutators"
            '';
          };

          formatter = pkgs.nixpkgs-fmt;
        };
    });
}
