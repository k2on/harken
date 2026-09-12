{
  description = "harken — a self-hosted, local-first music system";

  inputs = {
    # Pinned to a release branch here; the exact revision lives in flake.lock,
    # which is what actually makes the shell reproducible. Run `nix flake update`
    # deliberately, never as a side effect.
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };

    # The engine, for building. `Cargo.toml` names it by git and
    # `.cargo/config.toml` patches it to the checkout next door for local work
    # — and that patch is what shaped the committed `Cargo.lock`, which records
    # petros as a path with no revision at all. So a hermetic build has to
    # supply the same patch, pointing at a pinned copy instead of a sibling
    # directory. This is that copy; `flake.lock` pins it, and the packages check
    # it against the revision in `Cargo.toml` rather than trusting two pins to
    # stay equal on their own.
    #
    # A flake, because the engine brings the nix that knows its crates, its
    # patch and its code generator.
    petros = {
      url = "github:k2on/petros";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-overlay.follows = "rust-overlay";
      inputs.flake-parts.follows = "flake-parts";
      inputs.import-tree.follows = "import-tree";
    };

    # How a Petros app reaches a phone: `ubrn`, the two-layer cross-compile,
    # and — through its own `expo.nix` and `android.nix` inputs — the Expo
    # project's gradle layer and the SDK. Every `follows` here is what keeps
    # this closure to one nixpkgs: each of those flakes follows the one above
    # it, and the top of the chain is this file.
    petros-js = {
      url = "github:k2on/petros-js";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-parts.follows = "flake-parts";
      inputs.import-tree.follows = "import-tree";
      inputs.petros.follows = "petros";
    };

    # Not used here — this file lists its modules by hand, one per directory —
    # but the two libraries above are dendritic and each brings its own copy
    # otherwise. Naming it once is what lets them all follow one.
    import-tree.url = "github:vic/import-tree";
  };

  # Four directories, four modules: each of `domain/`, `server/`, `iced/` and
  # `expo/` carries the nix for what is in it as `flake-module.nix`. What is
  # left — one nixpkgs and one toolchain for all four, the source trees, the
  # workspace-wide checks, the devshell and its `harken` command — is below,
  # because it belongs to the workspace rather than to any one of them.
  outputs = inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [
        ./domain/flake-module.nix
        ./server/flake-module.nix
        ./iced/flake-module.nix
        ./expo/flake-module.nix
        # `petros-js`'s module imports the engine's, `expo.nix`'s and
        # `android.nix`'s in turn, so these two are the whole list — and each
        # builds over this flake's `pkgs`, not its own.
        inputs.petros.flakeModules.default
        inputs.petros-js.flakeModules.default
      ];

      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];

      perSystem = { system, self', icedLibs, petros, ... }:
        let
          # nixpkgs with the Rust overlay. One `pkgs` for the whole flake and
          # for everything imported into it: `petros-js`, `expo.nix` and
          # `android.nix` each build their library over *this* package set, so
          # there is one copy of nixpkgs in the closure and the SDK is composed
          # from the same one as the server.
          pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [ (import inputs.rust-overlay) ];
          };

          # The toolchain, named here and nowhere else. There is no
          # `rust-toolchain.toml`: a contributor works inside `nix develop`,
          # and a build container gets the same version as `RUST_VERSION` in
          # `expo/eas.json` — the one place this number is written down twice,
          # because EAS has no nix to read it from.
          #
          # Not the crates' MSRV: `rust-version` in `Cargo.toml` declares the
          # floor, and this is the version development actually happens on.
          #
          # Every target any client is built for. The Apple entries cost a
          # Linux machine two `rust-std` tarballs it will never link, which is
          # cheaper than a toolchain that differs by host.
          toolchain = pkgs.rust-bin.stable."1.90.0".default.override {
            extensions = [ "rustfmt" "clippy" "rust-analyzer" "rust-src" ];
            targets = [
              "wasm32-unknown-unknown"
              "aarch64-linux-android"
              "x86_64-linux-android"
              "aarch64-apple-ios"
              "aarch64-apple-ios-sim"
            ];
          };
          rustPlatform = pkgs.makeRustPlatform {
            cargo = toolchain;
            rustc = toolchain;
          };

          # ------------------------------------------------------- the trees
          #
          # What everything is built from: the cleaned source, the narrow
          # engine tree, each with the engine patch installed and the two
          # pins for one engine checked against each other. `_module.args.
          # sources`, so every directory's module reads the same tree.

          # `cleanSource` filters VCS files, not gitignored ones — so it would
          # happily copy this machine's `.cargo/config.toml`, whose patch points
          # at `/home/max/...`. A build that quietly used a developer's working
          # copy would be the worst possible kind of reproducible.
          src = pkgs.lib.cleanSourceWith {
            src = ./.;
            filter = path: type:
              let base = baseNameOf path; in
                !(builtins.elem base [ ".cargo" "target" "node_modules" "result" ]);
          };

          # What the Android engine build reads, and nothing else.
          #
          # The point is what is *missing*: `expo/src`, `app.config.ts`, the
          # assets. Editing a screen must not be an input to a cross-compile.
          # Measured on run 21 the engine half is 529s of every APK build — the
          # `ubrn` CLI compiled from source, then the workspace compiled twice,
          # once per Android ABI — and it was being paid again for a changed
          # `.tsx`, because the derivation's source was the whole repository.
          #
          # `iced` and `server` are here only because they are workspace members
          # and cargo will not parse the workspace without them.
          engineSrc = pkgs.lib.cleanSourceWith {
            src = ./.;
            name = "harken-engine-src";
            filter = path: type:
              let
                rel = pkgs.lib.removePrefix (toString ./. + "/") (toString path);
                wanted = [
                  "Cargo.toml"
                  "Cargo.lock"
                  "domain"
                  "server"
                  "iced"
                  "expo/modules/harken-native"
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

          sources = {
            inherit src engineSrc cargoPatch workspace engineWorkspace checkWorkspace cargoDeps;
          };

          # ------------------------------------------------------ the checks
          #
          # fmt, clippy, the suite — as derivations, and also as packages,
          # because `nix flake check` runs them all and says little, while
          # `nix build .#check-clippy` runs one and shows what it said.

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

          # ------------------------------------------------- the devshell
          #
          # `harken` is what the justfile used to be: every developer task as
          # a subcommand of one program the devshell carries, and that `nix
          # run .#harken` runs without the shell. `harken` alone is fmt, lint
          # and test, the way `just` alone was.

          # SQLite's C, compiled for wasm. Unwrapped on purpose: nix's wrapped
          # clang injects this system's glibc headers, which is exactly what
          # breaks a wasm32-unknown-unknown build.
          wasmClang = pkgs.llvmPackages.clang-unwrapped;
          # clang's own builtin headers. `getLib` rather than a literal path
          # because nixpkgs may put them in the `lib` output, and an unwrapped
          # clang cannot find them on its own.
          wasmResourceDir =
            "${pkgs.lib.getLib wasmClang}/lib/clang/"
            + pkgs.lib.versions.major wasmClang.version;

          # Re-record gradle's Maven graph into `expo/gradle-deps.json`.
          #
          # Working out what a gradle build fetches is a Turing-complete
          # question, so nixpkgs answers it empirically: run the build once
          # behind a recording proxy and keep what came back. This drives
          # that, and is the only thing that should ever write the file. Run
          # it when the build's dependencies move — an Expo or React Native
          # bump, a new config plugin, a gradle or AGP change. A stale
          # recording is noisy rather than silent: the replayed build fails on
          # whatever it is missing.
          #
          # Needs the network, and takes about as long as a full APK build,
          # because it is one — which is why the `gradle-deps` workflow runs
          # it on a runner rather than a laptop. Its own program rather than a
          # `harken` subcommand so that `nix run .#gradle-deps` costs no Rust
          # toolchain; `harken gradle-deps` runs the same thing. Anything after
          # `--` is passed to `nix build`, which is how you supply
          # `--override-input` on a machine that cannot reach every fetcher.
          gradle-deps = pkgs.writeShellApplication {
            name = "gradle-deps";
            text = ''
              repo=$(git rev-parse --show-toplevel)

              # `fetchDeps` reads this at evaluation time, so it has to parse
              # before it has anything in it.
              [ -s "$repo/expo/gradle-deps.json" ] || echo '{}' > "$repo/expo/gradle-deps.json"

              script=$(nix build --no-link --print-out-paths \
                "$repo#apk-debug.mitmCache.updateScript" "$@")

              # The script writes its output relative to the working
              # directory: the path it was given is inside the flake's source
              # in the store, and it keeps everything after the store path.
              cd "$repo"

              # Without bubblewrap. It is there to stop a careless build script
              # writing to `$HOME`, and it clears the environment — which on a
              # machine that reaches the network through a proxy leaves nix
              # unable to fetch what it does not have.
              USE_BWRAP=0 "$script"

              echo "wrote $repo/expo/gradle-deps.json"
            '';
          };

          harken = pkgs.writeShellApplication {
            name = "harken";
            runtimeInputs = [ toolchain gradle-deps ] ++ (with pkgs; [
              cargo-nextest
              watchexec
              bun
              nodejs_22
              python3
              git
            ]);
            # `bashOptions` is the default `set -euo pipefail`; the recipes
            # below were written under it in the justfile too.
            text = ''
              # The repository root is where the workspace is, however deep the
              # shell happens to be.
              root=$(git rev-parse --show-toplevel 2>/dev/null || true)
              if [ -z "$root" ] || [ ! -f "$root/Cargo.toml" ]; then
                echo "harken: not inside the harken repository" >&2
                exit 1
              fi
              cd "$root"

              expo=expo
              # bun hoists a workspace's binaries to the app, not to the
              # library itself.
              ubrn="$root/$expo/node_modules/.bin/ubrn"
              case "$(uname -s)" in
                Darwin) native_lib=libharken.dylib ;;
                *)      native_lib=libharken.so ;;
              esac

              usage() {
                cat <<'USAGE'
              harken — developer tasks. `nix develop` provides everything these need.

                harken                        fmt, lint, test
                harken test                   the whole suite. Must stay under 30 seconds
                harken lint                   clippy over everything, warnings are errors; fmt --check
                harken fmt                    cargo fmt --all
                harken serve [addr]           the sync server (127.0.0.1:8787; 0.0.0.0:8787 for a phone)
                harken iced [user] [addr]     a desktop peer. Run it twice to watch them sync
                harken web                    the iced client in a browser, at localhost:8080
                harken web-build              …just built, into iced/web/pkg
                harken mutators               rebuild the domain module and hand it to Metro
                harken mutators-watch         …on every save. Leave it running beside `bun start`
                harken latency                where the time goes in one mutation
                harken expo-install           install the JS side (once, and after a dependency change)
                harken bindings               regenerate the Expo client's TypeScript from the domain crate
                harken expo-android           build the Rust for Android, prebuild, run the app
                harken expo-ios               the same for iOS (needs Xcode)
                harken doc                    the engine's docs
                harken engine <rev>           move the engine pin in Cargo.toml
                harken gradle-deps [-- …]     re-record gradle's Maven graph (see the workflow)
              USAGE
              }

              # The mutator module: `harken test` and `harken lint` both start
              # here, because the conformance test runs the real module and
              # `foreign_peer!` does `include_bytes!` of it — an artifact that
              # does not exist yet is a build error, not a skipped test, and
              # on a fresh clone it reads like a broken checkout rather than a
              # missing build step.
              mutators() { ./domain/mutators.sh; }

              expo_install() { (cd "$expo" && bun install); }

              # The iced client in a browser.
              #
              # `CC_wasm32_unknown_unknown` because gcc cannot target wasm at
              # all, and `cc-rs` falls back to plain `CC` when no
              # target-specific override is set — so any shell that exports
              # `CC` (the nix devshell does) hijacks the wasm build and fails
              # deep inside glibc's headers. An *unwrapped* clang, since nix's
              # wrapped one injects glibc include paths and would fail the
              # same way; clang then needs its own builtin headers passed in,
              # because nix keeps them in a separate output.
              #
              # `SQLITE3_LIB_DIR` because `libsqlite3-sys` still emits
              # `-lsqlite3` even when it is not the one providing SQLite, so it
              # is pointed at an empty archive; the real symbols come from
              # `sqlite-wasm-rs`, linked into the same module. `!<arch>` is a
              # valid empty `ar` archive, which saves keeping a binary in the
              # tree.
              web_build() {
                cc="''${WASM_CC:-${wasmClang}/bin/clang}"
                cflags="''${WASM_CFLAGS:--resource-dir ${wasmResourceDir}}"
                ar="''${WASM_AR:-${pkgs.llvmPackages.llvm}/bin/llvm-ar}"

                # The generated glue and the wasm module carry a bindgen schema
                # version that must match exactly, so whatever wasm-bindgen
                # happens to be on PATH is not good enough — nixpkgs' is pinned
                # to its own release and ours moves with Cargo.lock. Fetch the
                # matching one into ./target rather than asking anyone to keep
                # a global install in step.
                want="$(sed -n '/^name = "wasm-bindgen"$/{n;s/^version = "\(.*\)"$/\1/p;q}' Cargo.lock)"
                have="$(wasm-bindgen --version 2>/dev/null | awk '{print $2}' || true)"
                if [ "$have" = "$want" ]; then
                  bindgen=wasm-bindgen
                else
                  bindgen="$PWD/target/wasm-tools/bin/wasm-bindgen"
                  if [ "$("$bindgen" --version 2>/dev/null | awk '{print $2}' || true)" != "$want" ]; then
                    echo "wasm-bindgen ''${have:-none} on PATH, need $want — fetching it into target/"
                    cargo install wasm-bindgen-cli --locked --version "$want" \
                      --root "$PWD/target/wasm-tools"
                  fi
                fi

                mkdir -p target/wasm-sqlite-stub iced/web/pkg
                printf '!<arch>\n' > target/wasm-sqlite-stub/libsqlite3.a
                CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS='--cfg getrandom_backend="wasm_js"' \
                CC_wasm32_unknown_unknown="$cc" \
                AR_wasm32_unknown_unknown="$ar" \
                CFLAGS_wasm32_unknown_unknown="$cflags" \
                SQLITE3_LIB_DIR="$PWD/target/wasm-sqlite-stub" SQLITE3_STATIC=1 \
                  cargo build -p harken-iced --target wasm32-unknown-unknown --release
                "$bindgen" --target web --no-typescript \
                  --out-dir iced/web/pkg \
                  target/wasm32-unknown-unknown/release/harken-iced.wasm
              }

              cmd="''${1:-}"
              [ $# -gt 0 ] && shift
              case "$cmd" in
                "")
                  cargo fmt --all
                  mutators
                  cargo clippy --workspace --all-features --all-targets -- -D warnings
                  cargo fmt --all --check
                  cargo nextest run --workspace --all-features
                  cargo test --workspace --all-features --doc
                  ;;
                test)
                  mutators
                  cargo nextest run --workspace --all-features
                  cargo test --workspace --all-features --doc
                  ;;
                lint)
                  mutators
                  cargo clippy --workspace --all-features --all-targets -- -D warnings
                  cargo fmt --all --check
                  ;;
                fmt) cargo fmt --all ;;
                serve) cargo run -p harken-server -- "''${1:-127.0.0.1:8787}" ;;
                iced) cargo run -p harken-iced -- --user "''${1:-alice}" --server "''${2:-127.0.0.1:8787}" ;;
                web-build) web_build ;;
                web)
                  web_build
                  echo "serving on http://localhost:8080"
                  cd iced/web && python3 -m http.server 8080
                  ;;
                mutators) mutators ;;
                mutators-watch)
                  echo "watching domain/ — save a file and check the phone"
                  watchexec --project-origin . --watch domain --exts rs \
                    --on-busy-update=restart -- "$0" mutators
                  ;;
                # Ignored by `harken test` because it is a measurement and it is
                # slow; run it when a number is in question.
                latency)
                  cargo test -p harken --all-features --release --test client_latency -- \
                    --ignored --nocapture --test-threads=1
                  ;;
                expo-install) expo_install ;;
                # Reads the UniFFI metadata straight back out of a host build of
                # the crate, so it needs no NDK and no Xcode — "did my change to
                # the Rust reach the client?" is answered in seconds, and the
                # `tsc` at the end turns "the app still calls the old API" into
                # a compile error rather than a crash on a device.
                #
                # `generate bindings` reads the crate name from `cargo metadata`
                # so it runs at the workspace root; `generate turbo-module`
                # reads the library's package.json so it runs there.
                bindings)
                  expo_install
                  cargo build -p harken --features foreign
                  # Wipe what was generated before regenerating it. ubrn writes
                  # new files and never removes old ones, so a working copy
                  # that has outlived a rename ends up with several generations
                  # side by side; the symptom is Expo's autolinking generating
                  # a `PackageList.java` that names a class nothing defines.
                  rm -rf "$expo/modules/harken-native/src/generated" \
                         "$expo/modules/harken-native/cpp/generated" \
                         "$expo/modules/harken-native/android/src/main/java"
                  "$ubrn" generate jsi bindings "target/debug/$native_lib" --library --no-format \
                    --ts-dir "$expo/modules/harken-native/src/generated" \
                    --cpp-dir "$expo/modules/harken-native/cpp/generated"
                  (cd "$expo/modules/harken-native" && "$ubrn" generate jsi turbo-module \
                    --config ubrn.config.yaml --native-bindings harken)
                  (cd "$expo" && ./node_modules/.bin/tsc --noEmit)
                  ;;
                # Needs the SDK and the NDK, which the default shell
                # deliberately does not carry: `nix develop .#android -c harken
                # expo-android`. Expo's native projects are generated rather
                # than committed, so `prebuild` runs first and `android/` never
                # enters the tree.
                expo-android)
                  expo_install
                  (cd "$expo/modules/harken-native" && "$ubrn" build android \
                    --config ubrn.config.yaml --and-generate --release)
                  (cd "$expo" && bunx expo prebuild --platform android --clean)
                  (cd "$expo" && bunx expo run:android)
                  ;;
                expo-ios)
                  expo_install
                  (cd "$expo/modules/harken-native" && "$ubrn" build ios \
                    --config ubrn.config.yaml --and-generate --release)
                  (cd "$expo" && bunx expo prebuild --platform ios --clean)
                  (cd "$expo" && bunx expo run:ios)
                  ;;
                doc) cargo doc -p petros --no-deps --all-features --open ;;
                # Move the pin on the engine. Seven dependency lines share one
                # revision, and a build container resolves exactly what is
                # written here — so they move together or not at all.
                #
                #     harken engine $(git -C ../petros rev-parse HEAD)
                engine)
                  rev="''${1:?usage: harken engine <rev>}"
                  sed -i "s|rev = \"[0-9a-f]\{40\}\"|rev = \"$rev\"|g" Cargo.toml
                  cargo update -p petros --precise 0.1.0 2>/dev/null || true
                  echo "  $(grep -c "rev = \"$rev\"" Cargo.toml) dependencies pinned to $rev"
                  ;;
                gradle-deps) gradle-deps "$@" ;;
                -h|--help|help) usage ;;
                *)
                  echo "harken: unknown command '$cmd'" >&2
                  usage >&2
                  exit 2
                  ;;
              esac
            '';
          };
        in
        {
          _module.args = {
            inherit pkgs toolchain rustPlatform sources;
          };

          formatter = pkgs.nixpkgs-fmt;

          packages = { inherit check-fmt check-clippy check-tests harken gradle-deps; };
          checks = { inherit check-fmt check-clippy check-tests; };

          apps = {
            harken = { type = "app"; program = "${harken}/bin/harken"; };
            gradle-deps = { type = "app"; program = "${gradle-deps}/bin/gradle-deps"; };
          };

          devShells = {
            default = pkgs.mkShell {
              packages = [ toolchain harken gradle-deps ] ++ (with pkgs; [
                cargo-nextest
                # `harken web` uses this only when its version happens to match the
                # wasm-bindgen crate in Cargo.lock, which nixpkgs cannot promise —
                # the schema versions must be identical. Otherwise the recipe
                # fetches the matching one into ./target by itself.
                wasm-bindgen-cli
                llvmPackages.llvm
                # The Expo client. Metro and the Expo CLI are node programs even
                # when bun installs and runs them, so both are here.
                bun
                nodejs_22
                # `harken mutators-watch` rebuilds the mutator module on save. The
                # whole hot-reload loop is this plus Metro, which is already
                # watching for the .ts it writes.
                watchexec
                sqlite
                pkg-config
                # For the audio server's transcoding (phase 3).
                ffmpeg

                python3
                eas-cli
              ]) ++ pkgs.lib.optionals pkgs.stdenv.isLinux icedLibs;

              # iced loads these at runtime rather than linking them.
              LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (pkgs.lib.optionals pkgs.stdenv.isLinux icedLibs);

              # `harken web` reads these. They are explicit paths rather than a bare
              # `clang` because the devshell also exports `CC=gcc`, and gcc cannot
              # target wasm.
              WASM_CC = "${wasmClang}/bin/clang";
              WASM_AR = "${pkgs.llvmPackages.llvm}/bin/llvm-ar";
              WASM_CFLAGS = "-resource-dir ${wasmResourceDir}";

              shellHook = ''
                # NixOS keeps the host's GPU drivers under /run/opengl-driver, and
                # they are linked against the host's libwayland. LD_LIBRARY_PATH
                # beats a library's own RUNPATH, so on a machine tracking a newer
                # channel than this flake's pin, the wayland above shadows the one
                # Mesa was built for. Every Mesa Vulkan driver then fails to load
                # ("undefined symbol: wl_fixes_interface"), wgpu finds no adapter
                # at all, and iced quietly falls back to its software renderer —
                # which draws the right pixels far too slowly to scroll, and
                # leaves stale ones behind where its damage tracking undershoots.
                # Nothing says so out loud, so let the host's own copy win.
                for driver in /run/opengl-driver/lib/libvulkan_*.so; do
                  [ -e "$driver" ] || continue
                  # Ask the driver itself, with our own path out of the way, so
                  # this keeps working when the host moves on again.
                  hostWayland=$(LD_LIBRARY_PATH= ldd "$driver" 2>/dev/null \
                    | sed -n 's|.*=> \(.*\)/libwayland-client\.so\.0 .*|\1|p' \
                    | head -1)
                  if [ -n "$hostWayland" ]; then
                    export LD_LIBRARY_PATH="$hostWayland:$LD_LIBRARY_PATH"
                    break
                  fi
                done

                echo "harken devshell — run \`harken\` for the commands"
              '';
            };
          };
        };
    };
}
