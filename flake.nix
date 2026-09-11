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
    # The engine, for building. `Cargo.toml` names it by git and
    # `.cargo/config.toml` patches it to the checkout next door for local work
    # — and that patch is what shaped the committed `Cargo.lock`, which records
    # petros as a path with no revision at all. So a hermetic build has to
    # supply the same patch, pointing at a pinned copy instead of a sibling
    # directory. This is that copy; `flake.lock` pins it, and the packages check
    # it against the revision in `Cargo.toml` rather than trusting two pins to
    # stay equal on their own.
    petros = {
      url = "github:k2on/petros";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, rust-overlay, petros }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f system);
    in
    {
      devShells = forAllSystems (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ (import rust-overlay) ];
          };

          # Read from ./rust-toolchain.toml rather than repeated here, so the
          # version cannot drift between the devshell and a plain `cargo`
          # outside it — and so bumping it is a one-line change in one file.
          toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

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

          # Runtime libraries the iced client will dlopen (phase 5). Wired up
          # now so the shell does not need revisiting when that lands.
          icedLibs = pkgs.lib.optionals pkgs.stdenv.isLinux (with pkgs; [
            wayland
            libxkbcommon
            libGL
            vulkan-loader
            fontconfig
          ]);
        in
        {
          default = pkgs.mkShell {
            packages = [ toolchain ] ++ (with pkgs; [
              cargo-nextest
              just
              # `just web` uses this only when its version happens to match the
              # wasm-bindgen crate in Cargo.lock, which nixpkgs cannot promise —
              # the schema versions must be identical. Otherwise the recipe
              # fetches the matching one into ./target by itself.
              wasm-bindgen-cli
              llvmPackages.llvm
              # The Expo client. Metro and the Expo CLI are node programs even
              # when bun installs and runs them, so both are here.
              bun
              nodejs_22
              # Rebuilds the mutator module on save. The whole hot-reload loop
              # is this plus Metro, which is already watching for the .ts it
              # writes.
              watchexec
              sqlite
              pkg-config
              # For the audio server's transcoding (phase 3).
              ffmpeg

              python3
              eas-cli
            ]) ++ icedLibs;

            # iced loads these at runtime rather than linking them.
            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath icedLibs;

            # `just web` reads these. They are explicit paths rather than a bare
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

              echo "harken devshell — just test | just lint | just offline | just serve"
            '';
          };

          # Everything the default shell has, plus the two tools that turn Rust
          # into an Android library.
          #
          #     nix develop .#android -c just expo-android
          #
          # The SDK and the NDK are *not* here, and that is deliberate. This
          # flake once composed them with `androidenv`; gradle then failed with
          # "The SDK directory is not writable", because the Android Gradle
          # Plugin resolves versions against the SDK directory and installs
          # whatever is missing — and a nix store path is read-only by
          # construction. Pinning every version to match Expo's exactly would
          # postpone that fight rather than win it: Expo moves its `compileSdk`
          # and `ndkVersion` on its own schedule, and nixpkgs moves on another.
          #
          # So the SDK comes from where it comes from for every other React
          # Native project — Android Studio locally, the runner image in CI —
          # and nix pins the part that is actually ours: the Rust toolchain, its
          # Android targets, cargo-ndk and bun. `crates/ffi` cross-compiles
          # identically either way.
          android = pkgs.mkShell {
            inputsFrom = [ self.devShells.${system}.default ];

            packages = with pkgs; [
              # Gives `cargo build` the NDK's toolchain, sysroot and target
              # triples, and drops the result where gradle looks for it.
              cargo-ndk
              jdk17
              # `eas build --local` shells out to these.
              jq
            ];

            shellHook = ''
              if [ -z "''${ANDROID_HOME:-}" ] && [ -n "''${ANDROID_SDK_ROOT:-}" ]; then
                export ANDROID_HOME="$ANDROID_SDK_ROOT"
              fi
              if [ -z "''${ANDROID_HOME:-}" ]; then
                echo "android shell: no ANDROID_HOME." >&2
                echo "  Install the SDK (Android Studio, or sdkmanager) and export it." >&2
                echo "  Note that Google publishes no aarch64-linux NDK, so an ARM" >&2
                echo "  Linux machine cannot build this at all — use x86_64, or CI." >&2
              else
                # cargo-ndk looks for these in turn; be explicit rather than
                # depending on which one an SDK install happened to set.
                export ANDROID_SDK_ROOT="''${ANDROID_SDK_ROOT:-$ANDROID_HOME}"
                if [ -z "''${ANDROID_NDK_HOME:-}" ] && [ -d "$ANDROID_HOME/ndk" ]; then
                  export ANDROID_NDK_HOME="$(ls -d "$ANDROID_HOME"/ndk/* | sort -V | tail -1)"
                fi
                echo "harken android shell — just expo-android"
                echo "  sdk: $ANDROID_HOME"
                echo "  ndk: ''${ANDROID_NDK_HOME:-<none found>}"
              fi
            '';
          };
        });

      packages = forAllSystems (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ (import rust-overlay) ];
          };

          # The same toolchain the devshell uses, from the same file, so a
          # package and a `cargo build` inside `nix develop` are the same build.
          toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          rustPlatform = pkgs.makeRustPlatform {
            cargo = toolchain;
            rustc = toolchain;
          };

          # Building a Petros app for Android is the engine's business, not this
          # app's: the SDK pinned the way every Petros app needs it, the `ubrn`
          # command, the `[patch]` that makes this lockfile resolvable, and the
          # two-layer cross-compile that stops a changed mutation recompiling a
          # hundred crates. All of it used to be in this file, where the second
          # Petros app would have had to copy it.
          android = import "${petros}/nix/android.nix" { inherit pkgs nixpkgs; };

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

          # cc-rs looks for a compiler under the target triple with dashes
          # turned into underscores, and prefers that over the bare `CC`. These
          # name the *build* machine's triple, so the same expression is right
          # on an ARM laptop and an x86_64 runner.
          hostCc = "CC_${builtins.replaceStrings [ "-" ] [ "_" ] pkgs.stdenv.buildPlatform.config}";
          hostAr = "AR_${builtins.replaceStrings [ "-" ] [ "_" ] pkgs.stdenv.buildPlatform.config}";

          # What the Android engine build reads, and nothing else.
          #
          # The point is what is *missing*: `clients/expo/src`, `app.json`, the
          # assets. Editing a screen must not be an input to a cross-compile.
          # Measured on run 21 the engine half is 529s of every APK build — the
          # `ubrn` CLI compiled from source, then the workspace compiled twice,
          # once per Android ABI — and it was being paid again for a changed
          # `.tsx`, because the derivation's source was the whole repository.
          #
          # `clients/iced` is here only because it is a workspace member and
          # cargo will not parse the workspace without it.
          engineSrc = pkgs.lib.cleanSourceWith {
            src = ./.;
            name = "harken-engine-src";
            filter = path: type:
              let
                rel = pkgs.lib.removePrefix (toString ./. + "/") (toString path);
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
          # the patch in place. The engine names its own crates.
          cargoPatch = android.mkCargoPatch petros;

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
            if [ "$want" != "${petros.rev}" ]; then
              echo "engine pin mismatch:" >&2
              echo "  Cargo.toml: $want" >&2
              echo "  flake.lock: ${petros.rev}" >&2
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
            if [ "$want" != "${petros.rev}" ]; then
              echo "engine pin mismatch: Cargo.toml $want, flake.lock ${petros.rev}" >&2
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

          # The engine's own dependency graph, vendored, so the code
          # generator can be built without the network. Petros is not a
          # workspace member here — `petros-codegen` does not appear in this
          # repository's lockfile at all — so it brings its own.
          petrosCargoDeps = pkgs.stdenv.mkDerivation {
            name = "petros-cargo-vendor";
            src = petros;
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
            outputHash = "sha256-WcgEW07qNxOpjun5jJnxcsYOp8wwzrF0I2lRXGNUcB8=";
          };

          # The generator that turns the wasm module into TypeScript. A
          # derivation rather than `cargo install --git`, which is what
          # `mutators.sh` falls back to when there is no sibling checkout — and
          # which is the reason the engine build reaches the network.
          petrosCodegen = rustPlatform.buildRustPackage {
            pname = "petros-codegen";
            version = "0.1.0";
            src = petros;
            cargoDeps = petrosCargoDeps;
            cargoBuildFlags = [ "-p" "petros-codegen" ];
            doCheck = false;
            meta.mainProgram = "petros-codegen";
          };

          # The domain compiled to wasm, and the TypeScript generated from
          # it. This is `scripts/mutators.sh` as a derivation.
          #
          # The checks need it and not merely the tests do: `foreign_peer!`
          # does `include_bytes!` of the module, so *compiling* the crate with
          # `--all-features` needs the file to exist. That is why `just lint`
          # depends on `mutators`, and why a check that skipped it would fail
          # in a way that reads like a broken checkout.
          mutators = pkgs.stdenv.mkDerivation {
            name = "harken-mutators";
            src = checkWorkspace;
            nativeBuildInputs = [
              rustPlatform.cargoSetupHook
              toolchain
              petrosCodegen
            ];
            inherit cargoDeps;
            buildPhase = ''
              runHook preBuild
              cargo build -p harken --no-default-features \
                --target wasm32-unknown-unknown --profile mutators --offline
              petros-codegen \
                target/wasm32-unknown-unknown/mutators/harken.wasm \
                mutators.gen.ts
              runHook postBuild
            '';
            installPhase = ''
              runHook preInstall
              mkdir -p $out
              cp target/wasm32-unknown-unknown/mutators/harken.wasm $out/
              cp mutators.gen.ts $out/
              runHook postInstall
            '';
          };

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

          icedLibs = with pkgs; [
            wayland
            libxkbcommon
            libGL
            vulkan-loader
            fontconfig
          ];

          # wasm-bindgen's generated glue and the module it generates for carry
          # a schema version that must match *exactly*, so the CLI has to be the
          # version in Cargo.lock and not whatever a channel happens to ship —
          # nixos-25.05 has 0.2.100 and 0.2.104, and the lock wants 0.2.128.
          # Reading the version out of the lockfile keeps the two in step: bump
          # the crate and this follows, and only the hashes need a human.
          wasmBindgenVersion =
            let
              lock = builtins.readFile ./Cargo.lock;
              after = pkgs.lib.strings.removePrefix
                "name = \"wasm-bindgen\"\nversion = \""
                (builtins.elemAt (builtins.split "name = \"wasm-bindgen\"\nversion = \"" lock) 2);
            in
            builtins.head (builtins.split "\"" after);

          # From static.crates.io rather than `fetchCrate`, for the same reason
          # the vendor directory is built by cargo: the `api/v1/…/download`
          # endpoint answers 403 to the User-Agent nixpkgs' fetchers send. This
          # host serves the identical tarball and has no such opinion.
          wasmBindgenSrc = pkgs.fetchzip {
            url = "https://static.crates.io/crates/wasm-bindgen-cli/wasm-bindgen-cli-${wasmBindgenVersion}.crate";
            hash = "sha256-a7lcXJnnZkYReja+iUO7NqqrWyv3toxnUgQb8s4IS5s=";
            extension = "tar.gz";
          };

          wasm-bindgen-cli = rustPlatform.buildRustPackage {
            pname = "wasm-bindgen-cli";
            version = wasmBindgenVersion;
            src = wasmBindgenSrc;
            cargoDeps = pkgs.stdenv.mkDerivation {
              name = "wasm-bindgen-cli-vendor";
              src = wasmBindgenSrc;
              nativeBuildInputs = [ toolchain pkgs.cacert pkgs.git ];
              buildPhase = ''
                export CARGO_HOME=$PWD/.cargo-home
                mkdir -p $out
                cargo vendor --versioned-dirs $out > $out/config.toml
                cp Cargo.lock $out/Cargo.lock
              '';
              dontInstall = true;
              dontFixup = true;
              outputHashMode = "recursive";
              outputHashAlgo = "sha256";
              outputHash = "sha256-xhQmH6oqGIEKdnovKaEwnwg6zMUkSyD6P9PPKjDx2ns=";
            };
            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = [ pkgs.openssl ];
            doCheck = false;
          };
        in
        rec {
          default = harken-server;

          # The generator, and the wasm module it reads. Exposed because
          # `nix build .#mutators` is how you find out whether it is the module
          # or your own code that is broken.
          inherit petrosCodegen mutators;

          # The three checks, also as packages: `nix flake check` runs them all
          # and says little, while `nix build .#check-clippy` runs one and shows
          # what it said.
          inherit check-fmt check-clippy check-tests;

          # The Android SDK, pinned to what Expo SDK 57 asks gradle for. The
          # versions are this app's; how to compose them without the Gradle
          # Plugin trying to install its own is the engine's.
          androidSdk = android.mkSdk { };

          # Gradle 9.3.1, built with nixpkgs' own machinery rather than
          # fetched and wrapped by hand.
          #
          # The version is not negotiable, and `pkgs.gradle_9` is the wrong one.
          # 26.05 ships 9.4.1, which carries `kotlin-stdlib-2.3.0`; Expo SDK
          # 57's gradle plugins are compiled with Kotlin 2.1.0, which reads
          # metadata up to 2.2.0 and refuses:
          #
          #   Class 'kotlin.reflect.KProperty' was compiled with an
          #   incompatible version of Kotlin. The actual metadata version is
          #   2.3.0, but the compiler version 2.1.0 can read versions up to
          #   2.2.0.
          #
          # which arrives as `Internal compiler error` against Expo's own
          # settings plugin. That is what the template's 9.3.1 pin is for.
          #
          # `mkGradle` builds that version and `wrapGradle` puts nixpkgs' setup
          # hook and `passthru.fetchDeps` on it — which is the whole point, and
          # the thing `mkGradle` in petros could never do: it unpacks the
          # distribution zip and wraps the launcher, and that is all.
          #
          # JDK 17 rather than the 21 nixpkgs defaults to, because that is what
          # this build already used.
          gradle9 =
            let
              unwrapped = pkgs.gradle-packages.mkGradle {
                version = "9.3.1";
                hash = "sha256-smbV/2uQ6tptw7IMsJDjcxMC5VOifF0+TfHw12vq/wY=";
                defaultJava = pkgs.jdk17;
              };
            in
            pkgs.callPackage pkgs.gradle-packages.wrapGradle {
              gradle-unwrapped = unwrapped;
            };

          # The Expo app's JavaScript dependencies.
          #
          # A fixed-output derivation, because `bun install` needs the network.
          # Built natively rather than under emulation: qemu cannot run bun at
          # all — it wants an address-space layout qemu will not give it, and
          # says so with "Unable to find a guest_base".
          expoModules = pkgs.stdenv.mkDerivation {
            name = "harken-expo-node-modules";
            src = pkgs.lib.cleanSourceWith {
              src = ./clients/expo;
              filter = path: type:
                !(builtins.elem (baseNameOf path) [ "node_modules" "android" "ios" ".expo" ]);
            };
            nativeBuildInputs = [ pkgs.bun pkgs.cacert ];
            buildPhase = ''
              export HOME=$TMPDIR
              bun install --frozen-lockfile --no-progress
            '';
            installPhase = ''
              mkdir -p $out
              cp -r node_modules/. $out/
            '';
            dontFixup = true;
            outputHashMode = "recursive";
            outputHashAlgo = "sha256";
            # One hash per platform, because the content really is different:
            # bun resolves the optional dependencies that carry native binaries
            # — `lightningcss-linux-arm64-gnu` against `lightningcss-linux-x64-
            # gnu` — for the machine it installs on. `--cpu=x64` does not
            # override that, because `bun.lock` was written on one platform and
            # `--frozen-lockfile` means the lock wins.
            #
            # Both move whenever `package.json` or `bun.lock` does, and each can
            # only be computed on the machine it belongs to. This one was last
            # refreshed on x86_64; if an ARM build fails on the hash, nix prints
            # the right one and it goes here.
            outputHash = {
              aarch64-linux = "sha256-usHdiS9E96QuhIj38q8xi5jPn9nLKZ57KN1jJGdWpOA=";
              x86_64-linux = "sha256-nVY9X+FkNSvMlDy8oi/7AebhqpXiCc6V/aSLytq/RgQ=";
            }.${system} or (throw "no node_modules hash recorded for ${system}");
          };

          # The `ubrn` command, built once rather than compiled from source by
          # the shim in `node_modules` on every build.
          ubrn = android.mkUbrn {
            inherit toolchain;
            nodeModules = expoModules;
            # The generator ships no lockfile — the npm package is the built
            # CLI and its Rust sources, and cargo is expected to resolve
            # wherever it runs. So one is committed here, and the vendored
            # result is pinned by hash. Both move when `bun.lock` moves the
            # generator's version.
            #
            # `clients/expo/node_modules/…/Cargo.lock` is not this file: cargo
            # writes one there whenever it runs under this tree, and it comes
            # out carrying the engine's seven crates because `[patch]` applies.
            lockFile = ./clients/expo/ubrn-Cargo.lock;
            depsHash = "sha256-YdwO0AOhoQjRbo5psMAxSSCRM7Ok8OsrjcAE13QMXP0=";
          };

          # The engine cross-compiled for Android, and the bindings generated
          # from the same metadata.
          #
          # Two layers, and the first is why: `androidEngine.thirdParty`
          # compiles the dependency graph from a tree where this app's crates
          # *and the engine's* are stubs, and the engine build starts from that
          # `target/`. Changing a mutation recompiles harken; bumping the engine
          # recompiles petros; neither recompiles `ciborium` and the hundred
          # others whose versions are fixed in a lockfile that did not move.
          #
          # `engineSrc` is what it is allowed to read, and what is deliberately
          # missing from it is `clients/expo/src` and `app.json`: a screen is
          # not an input to a cross-compile.
          androidEngine = android.mkEngine {
            name = "harken";
            src = engineWorkspace;
            inherit toolchain ubrn;
            sdk = androidSdk;
            nodeModules = expoModules;
            petrosSrc = petros;
            vendor = cargoDeps;
            moduleDir = "clients/expo/modules/harken-native";
            appCrates = [ "crates/harken" "crates/server" "clients/iced" ];

            # `foreign_peer!` does `include_bytes!` of the module, so the crate
            # does not compile until it exists.
            preEngine = ''
              # The module, already built, rather than `scripts/mutators.sh`.
              #
              # The script reaches the network: with no sibling checkout it
              # installs `petros-codegen` with `cargo install --git`, which is
              # one of the two remaining reasons this derivation asks for
              # `__noChroot`. The derivation builds the same two files from the
              # pinned engine.
              #
              # The wasm goes back where `include_bytes!` expects it, because
              # compiling `harken` with `--features foreign` reads it — that is
              # what makes this a build input and not a test fixture.
              echo "--- the mutator module, prebuilt"
              mkdir -p clients/expo/src target/wasm32-unknown-unknown/mutators
              cp ${mutators}/mutators.gen.ts clients/expo/src/mutators.gen.ts
              cp ${mutators}/harken.wasm \
                target/wasm32-unknown-unknown/mutators/harken.wasm
              chmod -R u+w clients/expo/src target/wasm32-unknown-unknown

              # The generator resolves react-native's headers through this.
              echo "--- node_modules"
              cp -a ${expoModules} clients/expo/node_modules
              chmod -R u+w clients/expo/node_modules
            '';

            extraInstall = ''
              install -Dm444 clients/expo/src/mutators.gen.ts $out/mutators.gen.ts
            '';
          };

          # The first layer on its own, for when the question is whether it is
          # the dependencies or this app that is slow.
          androidDeps = androidEngine.thirdParty;

          # The Android build, in two variants — see `apk-release` below.
          #
          # Everything Google publishes for Android is a `linux-x86_64` binary
          # — the NDK's clang, aapt2, d8, CMake — so on an ARM machine this
          # depends on `binfmt_misc` being registered for x86_64. That works,
          # including inside a build sandbox, and it is slow: the emulated
          # compile of SQLite's amalgamation alone is about a minute per ABI.
          #
          # One thing does *not* work under emulation: the SDK's CMake, which
          # qemu refuses with "Unable to find a guest_base to satisfy all guest
          # address mapping requirements". On x86_64 it runs and nothing is
          # needed. On ARM, `local.properties` has to name a native CMake with
          # `cmake.dir` — and it must be 3.22.x, because React Native declares a
          # minimum that 4.x rejects and 3.31 does not find `ReactAndroid` where
          # the prefab puts it. 25.05 ships neither, and 3.22.1 does not compile
          # against its curl, so that is unfinished: `.#apk` is x86_64 for now,
          # which is what CI is.
          #
          # Not a fixed-output derivation, and it cannot be one: an APK is a zip
          # and a signed one at that, so it is not reproducible byte-for-byte.
          # Everything it needs from the network is fetched by a derivation that
          # *is* — `expoModules` and `gradleDeps` — and the build itself runs
          # offline.
          apk-debug = pkgs.stdenv.mkDerivation (finalAttrs: {
            name = "harken-debug-apk";
            src = workspace;

            # `debug` or `release`. A derivation attribute, so the builder gets
            # it as a shell variable: it is gradle's build type, the directory
            # the APK lands in, and — capitalised — half the name of the task
            # that builds it. `apk-release` overrides this one attribute and
            # nothing else.
            variant = "debug";

            nativeBuildInputs = [
              toolchain
              gradle9
              androidSdk
              pkgs.cacert
              pkgs.cargo-ndk
              pkgs.git
              pkgs.jdk17
              pkgs.ninja
              pkgs.nodejs
              pkgs.python3
              pkgs.unzip
              pkgs.which
            ];

            # Gradle's own dependency graph, recorded once and replayed from
            # the store — the last reason this derivation reached the network.
            #
            # There is no lockfile to vendor a Maven graph from, because working
            # out what a gradle build fetches is a Turing-complete question; so
            # nixpkgs answers it by running the build once behind a recording
            # proxy and keeping what came back. `gradle-deps.json` is that
            # recording, and `scripts/gradle-deps.sh` regenerates it.
            #
            # An APK is a signed zip and is not reproducible byte-for-byte, so
            # the *APK* can never be a fixed-output derivation. Its dependencies
            # can, which is the part that matters.
            mitmCache = gradle9.fetchDeps {
              pkg = finalAttrs.finalPackage;
              data = ./gradle-deps.json;
            };

            # What the recording runs, instead of nixpkgs' `nixDownloadDeps`.
            #
            # That task resolves every resolvable configuration, which is right
            # for a plain JVM project and wrong for an Android one: the variant
            # metadata is deliberately ambiguous until a build type picks a
            # side, so it fails on configurations no build ever resolves.
            #
            #   Could not resolve project :expo-modules-core.
            #   … we cannot choose between the following variants:
            #     - debugApiElements
            #     - releaseApiElements
            #
            # Both assembles, because `apk-release` is this derivation with one
            # attribute changed and shares this recording — so it has to cover
            # Hermes and the release toolchain too, not just the debug half.
            gradleUpdateTask = "assembleDebug assembleRelease";

            # No `__noChroot`. Nothing here reaches the network any more: the
            # Maven graph is replayed from the recording above, the engine's
            # module is a derivation, and `ubrn` is pinned by a lockfile.
            #
            # Which also buys a stable path. A sandboxed build runs at `/build`
            # everywhere; an impure one runs somewhere ending in a pid and a
            # random number. Gradle's task history and ninja's `.cxx` record
            # absolute paths, so carrying native build state between
            # derivations needs this — and that state is worth carrying:
            # measured on one module, a second assemble with it restored is 35s
            # against 3m30s.

            ANDROID_HOME = "${androidSdk}/libexec/android-sdk";
            ANDROID_SDK_ROOT = "${androidSdk}/libexec/android-sdk";
            ANDROID_NDK_HOME = "${androidSdk}/libexec/android-sdk/ndk/27.1.12297006";
            JAVA_HOME = "${pkgs.jdk17}";

            # `cargo-ndk` sets `CC` for its child, and cc-rs reads it for *host*
            # artifacts too — and `petros-sql` is a proc macro that links
            # SQLite, so this build compiles libsqlite3-sys for the host as
            # well. Without this it does so with the NDK's clang, which has no
            # glibc sysroot, and fails on a missing `stdio.h`. A
            # target-qualified variable wins over the bare one.
            # …and the triple is the *build* machine's, not a fixed one. It
            # read `aarch64` here, which is the development box; on the x86_64
            # runner nothing was set at all, and the only reason that built is
            # that `__noChroot` let the NDK's clang find the host's
            # `/usr/include`. Under a real sandbox it fails on `stdio.h`.
            ${hostCc} = "gcc";
            ${hostAr} = "ar";

            # Preparing the project is `configurePhase`, not `buildPhase`, and
            # that is load-bearing rather than tidiness. `fetchDeps`' update
            # script runs `unpackPhase patchPhase configurePhase` and then
            # gradle — it never calls `buildPhase`. With the preparation in
            # `buildPhase` there would be no `android/` for it to record from.
            configurePhase = ''
              runHook preConfigure

              export HOME=$TMPDIR
              export CARGO_HOME=$TMPDIR/cargo

              # Only where emulation is actually involved. A native x86_64
              # builder has no `binfmt_misc` entry for its own architecture and
              # does not want one, so asking for it there fails a build that
              # would otherwise run — which is what CI hit.
              ${pkgs.lib.optionalString (system != "x86_64-linux") ''
                if ! ${pkgs.coreutils}/bin/test -e /proc/sys/fs/binfmt_misc/x86_64-linux; then
                  echo "" >&2
                  echo "This needs to run x86_64 binaries: everything Google ships" >&2
                  echo "for Android is linux-x86_64 only. On NixOS:" >&2
                  echo "" >&2
                  echo "  boot.binfmt.emulatedSystems = [ \"x86_64-linux\" ];" >&2
                  echo "" >&2
                  exit 1
                fi
              ''}

              echo "--- node_modules"
              # A writable copy rather than a symlink into the store. `expo
              # prebuild`, gradle and Metro all write into `node_modules`, and a
              # store path refuses.
              cp -a ${expoModules} clients/expo/node_modules
              chmod -R u+w clients/expo/node_modules

              # The engine and its bindings, already built. This used to be
              # `mutators.sh` and `ubrn build android` inline, which is 529s of
              # compiling the workspace once per ABI — paid again whenever any
              # file in the repository changed, because that was this
              # derivation's source. It is `androidEngine` now, whose source is
              # the Rust and nothing else.
              echo "--- the engine, prebuilt"
              m=clients/expo/modules/harken-native
              rm -rf $m
              cp -r ${androidEngine}/module $m
              cp ${androidEngine}/mutators.gen.ts clients/expo/src/mutators.gen.ts
              chmod -R u+w $m clients/expo/src

              echo "--- the native project"
              cd clients/expo
              ./node_modules/.bin/expo prebuild --platform android --no-install

              # The ABIs gradle compiles C++ for, which the template sets to all
              # four. Two of them are dead weight here: `ubrn.config.yaml` builds
              # the Rust for arm64-v8a and x86_64 only, so an armeabi-v7a or x86
              # APK would carry a JNI library with no engine in it. Measured on
              # run 21, the first ABI costs about 5m50s of CMake and each one
              # after it about 85s — so the two that cannot work were costing
              # nearly three minutes to be unusable.
              #
              # These two lists have to agree. If you add a target there, add it
              # here.
              #
              # A properties file keeps the last value for a key, so appending
              # replaces the template's — including its `-Xmx2048m` and no
              # parallelism, which is a laptop's answer where a runner has four
              # cores and 16GB, and the Android build guide's own advice is a
              # bigger heap and the parallel collector when GC is a visible
              # share of the build.
              #
              # The leading newline is not cosmetic. `expo prebuild` writes this
              # file with no trailing one, and its last line is
              #
              #   expo.inlineModules.watchedDirectories=[]
              #
              # so an appended line lands on the *end* of it. That gives the
              # property the value `[]reactNativeArchitectures=…`, which Expo's
              # autolinking plugin hands to `JSON.parse`, and the ABI list is
              # never set at all. Gradle reports the first half as
              #
              #   Process 'command 'node''' finished with non-zero exit value 1
              #
              # (the third quote is nix's escape for the pair before it,
              # which would otherwise end this string)
              #
              # with node's own message nowhere in the log, which is what runs
              # 22, 23 and 27 died of.
              printf '\n' >> android/gradle.properties
              cat >> android/gradle.properties <<'PROPS'
              reactNativeArchitectures=arm64-v8a,x86_64
              org.gradle.jvmargs=-Xmx6g -XX:MaxMetaspaceSize=1g -XX:+UseParallelGC
              org.gradle.parallel=true
              PROPS

              # AGP resolves the versions a project asks for against the SDK
              # directory and installs whatever is missing, which a store path
              # can never allow. A copy it can write to is the only way through
              # — and every component it could want is pinned above, so it never
              # actually installs anything.
              cp -r $ANDROID_HOME $TMPDIR/sdk
              chmod -R u+w $TMPDIR/sdk
              # nixpkgs puts the NDK in two places and AGP complains about the
              # second one on every task.
              rm -rf $TMPDIR/sdk/ndk-bundle
              export ANDROID_HOME=$TMPDIR/sdk ANDROID_SDK_ROOT=$TMPDIR/sdk
              export ANDROID_NDK_HOME=$TMPDIR/sdk/ndk/27.1.12297006

              cat > android/local.properties <<EOF
              sdk.dir=$TMPDIR/sdk
              EOF

              # `GRADLE_USER_HOME` explicitly, because the JVM does not read
              # `$HOME`: `user.home` comes from the passwd entry, which for a
              # nix build user is `/var/empty`. The `export HOME=$TMPDIR` above
              # is invisible to anything running on the JVM, so gradle would put
              # its caches somewhere it cannot write however that is set. The
              # setup hook honours it if it is already set.
              export GRADLE_USER_HOME=$TMPDIR/gradle

              # Leave the shell in the gradle project. The update script runs
              # gradle straight after this phase and does no `cd` of its own.
              cd android

              runHook postConfigure
            '';

            # `gradle`, not `./gradlew`: the wrapper downloads its own copy from
            # services.gradle.org, which buys nothing over the pinned one and
            # costs the thing this derivation is trying to keep. It is also the
            # shell function the setup hook defines rather than the binary, so
            # `--no-daemon`, `--console plain`, the init script and — when the
            # dependencies are being replayed — the proxy and truststore flags
            # are all added for us.
            buildPhase = ''
              runHook preBuild
              gradle "assemble''${variant^}"
              runHook postBuild
            '';

            installPhase = ''
              runHook preInstall
              mkdir -p $out
              cp app/build/outputs/apk/$variant/*.apk $out/
              runHook postInstall
            '';
          });

          # The same build, gradle's release type: the JavaScript compiled to
          # Hermes bytecode and bundled into the APK rather than fetched from a
          # dev server, resources crunched, no debuggable flag. Minification
          # stays off, which is the template's default.
          #
          # Signed with the debug keystore Expo's template ships — its
          # `release` block says `signingConfig signingConfigs.debug` under a
          # comment telling you to generate your own. So this installs and runs
          # anywhere, and is not something to put on Play: the key is public.
          # A real key is a different job than building, because a nix store is
          # world-readable and a signing key cannot live in one — it would mean
          # producing an unsigned APK here and signing it outside.
          apk-release = apk-debug.overrideAttrs (_: {
            name = "harken-release-apk";
            variant = "release";
          });

          # `nix build .#apk` has always meant the development build. It still
          # does; the variants are named for anything that has to choose.
          apk = apk-debug;

          # The sync server. `HARKEN_WEB` points it at a browser client; the
          # NixOS module sets it to `harken-web` so both are on one port.
          harken-server = rustPlatform.buildRustPackage {
            pname = "harken-server";
            version = "0.1.0";
            src = workspace;
            inherit cargoDeps;
            cargoBuildFlags = [ "-p" "harken-server" ];
            # The workspace's tests need the mutator module, which is a wasm
            # build with its own toolchain. `nix flake check` is not the place
            # for that; `just` is, and CI runs it.
            doCheck = false;
            meta.mainProgram = "harken-server";
          };

          # The desktop client. iced dlopens its graphics stack, so the runtime
          # libraries go on the RPATH rather than being hoped for.
          harken-iced = rustPlatform.buildRustPackage {
            pname = "harken-iced";
            version = "0.1.0";
            src = workspace;
            inherit cargoDeps;
            cargoBuildFlags = [ "-p" "harken-iced" ];
            doCheck = false;
            nativeBuildInputs = [ pkgs.makeWrapper ];
            buildInputs = icedLibs;
            postInstall = ''
              wrapProgram $out/bin/harken-iced \
                --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath icedLibs}
            '';
            meta.mainProgram = "harken-iced";
          };

          # The same client, compiled to wasm, plus the shell that loads it.
          # `just web-build` does this too; the difference is that this one
          # cannot reach the network, so every version is pinned rather than
          # fetched when it turns out not to match.
          harken-web = rustPlatform.buildRustPackage {
            pname = "harken-web";
            version = "0.1.0";
            src = workspace;
            inherit cargoDeps;
            doCheck = false;

            nativeBuildInputs = [ wasm-bindgen-cli pkgs.llvmPackages.clang-unwrapped pkgs.llvmPackages.bintools ];

            buildPhase = ''
              runHook preBuild

              # SQLite's C is not compiled here: the browser peer keeps its
              # database in the page, so the crate needs the symbols to link
              # and never calls them. An empty archive satisfies the linker.
              mkdir -p sqlite-stub
              printf '!<arch>\n' > sqlite-stub/libsqlite3.a

              export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS='--cfg getrandom_backend="wasm_js"'
              export CC_wasm32_unknown_unknown=clang
              export AR_wasm32_unknown_unknown=llvm-ar
              export CFLAGS_wasm32_unknown_unknown="-resource-dir ${pkgs.lib.getLib pkgs.llvmPackages.clang-unwrapped}/lib/clang/${pkgs.lib.versions.major pkgs.llvmPackages.clang-unwrapped.version}"
              export SQLITE3_LIB_DIR="$PWD/sqlite-stub"
              export SQLITE3_STATIC=1

              cargo build -p harken-iced --target wasm32-unknown-unknown --release --offline

              runHook postBuild
            '';

            installPhase = ''
              runHook preInstall
              mkdir -p $out
              cp clients/iced/web/index.html $out/
              wasm-bindgen --target web --no-typescript --out-dir $out/pkg \
                target/wasm32-unknown-unknown/release/harken-iced.wasm
              runHook postInstall
            '';
          };
        });

      # `services.harken.enable = true` and there is a music system on a port:
      # the sync socket and the browser client that talks to it, together,
      # because they are one deployment and splitting them across two ports
      # buys an origin to configure and nothing else.
      nixosModules.default = { config, lib, pkgs, ... }:
        let
          cfg = config.services.harken;
          harken = self.packages.${pkgs.stdenv.hostPlatform.system};
        in
        {
          options.services.harken = {
            enable = lib.mkEnableOption "the Harken sync server and its browser client";

            port = lib.mkOption {
              type = lib.types.port;
              default = 8787;
              description = "Port for both the sync socket and the browser client.";
            };

            address = lib.mkOption {
              type = lib.types.str;
              default = "127.0.0.1";
              example = "0.0.0.0";
              description = ''
                Address to bind. The default is loopback, so reaching this from
                a phone means either setting this and opening the firewall, or
                — better — putting a reverse proxy in front, since the engine
                does no authentication of its own.
              '';
            };

            web = lib.mkOption {
              type = lib.types.nullOr lib.types.package;
              default = harken.harken-web;
              defaultText = lib.literalExpression "harken.packages.\${system}.harken-web";
              description = ''
                The browser client to serve at `/`. Null serves the socket
                alone, for a deployment that only wants the phone.
              '';
            };

            package = lib.mkOption {
              type = lib.types.package;
              default = harken.harken-server;
              defaultText = lib.literalExpression "harken.packages.\${system}.harken-server";
              description = "The server to run.";
            };

            openFirewall = lib.mkOption {
              type = lib.types.bool;
              default = false;
              description = "Open {option}`services.harken.port` in the firewall.";
            };
          };

          config = lib.mkIf cfg.enable {
            systemd.services.harken = {
              description = "Harken sync server";
              wantedBy = [ "multi-user.target" ];
              after = [ "network.target" ];

              environment = lib.optionalAttrs (cfg.web != null) {
                HARKEN_WEB = "${cfg.web}";
              };

              serviceConfig = {
                ExecStart = "${lib.getExe cfg.package} ${cfg.address}:${toString cfg.port}";
                Restart = "on-failure";

                # The log is the whole of the state, so it wants a real place
                # rather than the temp dir the demo uses. `TMPDIR` is what the
                # server reads for it, which is why this is set rather than a
                # flag: the same binary serves `just serve` and this.
                DynamicUser = true;
                StateDirectory = "harken";
                Environment = [ "TMPDIR=%S/harken" ];

                # Nothing here needs any of it.
                NoNewPrivileges = true;
                PrivateDevices = true;
                PrivateTmp = true;
                ProtectClock = true;
                ProtectControlGroups = true;
                ProtectHome = true;
                ProtectHostname = true;
                ProtectKernelLogs = true;
                ProtectKernelModules = true;
                ProtectKernelTunables = true;
                ProtectSystem = "strict";
                RestrictAddressFamilies = [ "AF_INET" "AF_INET6" ];
                RestrictNamespaces = true;
                RestrictRealtime = true;
                SystemCallArchitectures = "native";
                SystemCallFilter = [ "@system-service" "~@privileged" ];
              };
            };

            networking.firewall.allowedTCPPorts = lib.mkIf cfg.openFirewall [ cfg.port ];
          };
        };

      # `nix flake check` is what CI runs, and what a contributor runs. The
      # point is not that it is faster — it is that it is the same expression
      # on both, pinned the same way. Run 37 failed on a `target/` that
      # `Swatinem/rust-cache` had pruned before saving, while the identical
      # `just` passed here; a check keyed on its inputs cannot do that.
      checks = forAllSystems (system: {
        inherit (self.packages.${system}) check-fmt check-clippy check-tests;
      });

      formatter = forAllSystems (system: nixpkgs.legacyPackages.${system}.nixpkgs-fmt);
    };
}
