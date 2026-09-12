# The trees everything is built from, and the dependencies vendored for them.
#
# `_module.args.sources` so that every other module reads the same tree: the
# cleaned source, the narrow engine tree, each with the engine patch installed
# and the two pins for one engine checked against each other.
{ inputs, ... }: {
  perSystem = { pkgs, toolchain, petros, ... }:
    let
      # `cleanSource` filters VCS files, not gitignored ones — so it would
      # happily copy this machine's `.cargo/config.toml`, whose patch points
      # at `/home/max/...`. A build that quietly used a developer's working
      # copy would be the worst possible kind of reproducible.
      src = pkgs.lib.cleanSourceWith {
        src = ../.;
        filter = path: type:
          let base = baseNameOf path; in
            !(builtins.elem base [ ".cargo" "target" "node_modules" "result" ]);
      };

      # What the Android engine build reads, and nothing else.
      #
      # The point is what is *missing*: `clients/expo/src`, `app.config.ts`, the
      # assets. Editing a screen must not be an input to a cross-compile.
      # Measured on run 21 the engine half is 529s of every APK build — the
      # `ubrn` CLI compiled from source, then the workspace compiled twice,
      # once per Android ABI — and it was being paid again for a changed
      # `.tsx`, because the derivation's source was the whole repository.
      #
      # `clients/iced` is here only because it is a workspace member and
      # cargo will not parse the workspace without it.
      engineSrc = pkgs.lib.cleanSourceWith {
        src = ../.;
        name = "harken-engine-src";
        filter = path: type:
          let
            rel = pkgs.lib.removePrefix (toString ../. + "/") (toString path);
            wanted = [
              "Cargo.toml"
              "Cargo.lock"
              "rust-toolchain.toml"
              "scripts"
              "crates"
              "clients/iced"
              "clients/expo/modules/harken-native"
            ];
          in
          # Either the path is inside something wanted, or it is a directory
            # on the way to one.
          pkgs.lib.any
            (w: pkgs.lib.hasPrefix w rel || pkgs.lib.hasPrefix rel w)
            wanted;
      };

      # The patch `.cargo/config.toml` provides locally, pointing at the
      # pinned engine instead of `../petros`. Without it cargo cannot
      # resolve the lockfile at all, because the lockfile was written with
      # the patch in place. The engine names its own crates, and the engine
      # writes the patch.
      cargoPatch = petros.mkCargoPatch inputs.petros;

      # The tree every package is built from: the sources, plus the patch
      # that makes the lockfile resolvable, plus a check that the two pins
      # for one engine agree. Done here rather than in each package so that
      # *vendoring* sees the patch too — cargo cannot resolve the lock
      # without it, and vendoring resolves the lock.
      workspace = pkgs.runCommand "harken-workspace"
        {
          cargoToml = "${src}/Cargo.toml";
        } ''
        want=$(grep -o 'rev = "[0-9a-f]\{40\}"' "$cargoToml" | head -1 | cut -d'"' -f2)
        if [ "$want" != "${inputs.petros.rev}" ]; then
          echo "engine pin mismatch:" >&2
          echo "  Cargo.toml: $want" >&2
          echo "  flake.lock: ${inputs.petros.rev}" >&2
          echo "Run: nix flake update petros" >&2
          exit 1
        fi
        cp -r ${src} $out
        chmod -R u+w $out
        if grep -q 'source = "git+https://github.com/k2on/petros' $out/Cargo.lock; then
          echo "Cargo.lock records the engine as a git source." >&2
          echo "The patch installed here shapes it into a path, so cargo" >&2
          echo "would have to re-resolve — and every build below passes" >&2
          echo "--locked. Something ran cargo without .cargo/config.toml." >&2
          echo "Run: git checkout -- Cargo.lock" >&2
          exit 1
        fi
        install -Dm444 ${cargoPatch} $out/.cargo/config.toml
      '';

      # The narrow tree, with the same check the full one gets: the two
      # pins for one engine have to agree. No `.cargo/config.toml` here —
      # the engine build writes its own, because it needs the vendored
      # dependencies in it as well as the patch.
      engineWorkspace = pkgs.runCommand "harken-engine-workspace"
        {
          cargoToml = "${engineSrc}/Cargo.toml";
        } ''
        want=$(grep -o 'rev = "[0-9a-f]\{40\}"' "$cargoToml" | head -1 | cut -d'"' -f2)
        if [ "$want" != "${inputs.petros.rev}" ]; then
          echo "engine pin mismatch: Cargo.toml $want, flake.lock ${inputs.petros.rev}" >&2
          exit 1
        fi
        cp -r ${engineSrc} $out
        chmod -R u+w $out
        if grep -q 'source = "git+https://github.com/k2on/petros' $out/Cargo.lock; then
          echo "Cargo.lock records the engine as a git source." >&2
          echo "Run: git checkout -- Cargo.lock" >&2
          exit 1
        fi
      '';

      # The narrow tree with the engine patch installed: what a check
      # reads. `engineWorkspace` deliberately has no `.cargo/config.toml`,
      # because the Android build writes its own with the vendored
      # dependencies in it; a check wants the same narrow source and the
      # ordinary vendor the setup hook provides.
      checkWorkspace = pkgs.runCommand "harken-check-workspace" { } ''
        cp -r ${engineWorkspace} $out
        chmod -R u+w $out
        install -Dm444 ${cargoPatch} $out/.cargo/config.toml
      '';

      # The dependencies, vendored by cargo itself.
      #
      # Not `importCargoLock` and not `fetchCargoVendor`, both of which
      # download from `crates.io/api/v1/…/download` with whatever
      # User-Agent their fetcher happens to send — and crates.io now
      # answers 403 to the default ones. It is not rate limiting, though it
      # looks like it: a different crate fails each run because the requests
      # are parallel, and every one of them would fail alone.
      #
      #     curl        …/api/v1/crates/atomic-waker/1.1.2/download  -> 403
      #     curl -A …   the same URL                                 -> 200
      #
      # Patching that would mean vendoring nixpkgs' fetcher, which is
      # written inline in `fetch-cargo-vendor.nix` and not an overridable
      # attribute. `cargo vendor` needs no patching: it is the tool whose
      # job this is, it uses the sparse index, and it identifies itself.
      #
      # A fixed-output derivation, so the network is allowed and the result
      # is pinned by hash like any other fetch.
      cargoDeps = pkgs.stdenv.mkDerivation {
        name = "harken-cargo-vendor";
        src = workspace;

        nativeBuildInputs = [ toolchain pkgs.cacert pkgs.git ];

        buildPhase = ''
          export CARGO_HOME=$PWD/.cargo-home
          mkdir -p $out
          cargo vendor --locked --versioned-dirs $out > $out/config.toml
          # The setup hook diffs this against the workspace's, to catch a
          # vendor directory that has drifted from the lock it was made
          # from. Worth keeping: it is the check that a stale `outputHash`
          # would otherwise slip past.
          cp Cargo.lock $out/Cargo.lock
        '';
        dontInstall = true;
        dontFixup = true;

        outputHashMode = "recursive";
        outputHashAlgo = "sha256";
        outputHash = "sha256-z3c9ZDiu29FuDF6JOY7pseb7FXEmppvaYSPRXa2k+bI=";
      };
    in
    {
      _module.args.sources = {
        inherit src engineSrc cargoPatch workspace engineWorkspace checkWorkspace cargoDeps;
      };
    };
}
