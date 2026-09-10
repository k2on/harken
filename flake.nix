{
  description = "harken — a self-hosted, local-first music system";

  inputs = {
    # Pinned to a release branch here; the exact revision lives in flake.lock,
    # which is what actually makes the shell reproducible. Run `nix flake update`
    # deliberately, never as a side effect.
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
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

          # The patch `.cargo/config.toml` provides locally, pointing at the
          # pinned engine instead of `../petros`. Without it cargo cannot
          # resolve the lockfile at all, because the lockfile was written with
          # the patch in place.
          cargoPatch = pkgs.writeText "petros-patch.toml" ''
            [patch."https://github.com/k2on/petros"]
            petros = { path = "${petros}/crates/petros" }
            petros-wasm-host = { path = "${petros}/crates/petros-wasm-host" }
            petros-schema = { path = "${petros}/crates/petros-schema" }
            petros-sql = { path = "${petros}/crates/petros-sql" }
            petros-testkit = { path = "${petros}/crates/petros-testkit" }
            petros-wasm-guest = { path = "${petros}/crates/petros-wasm-guest" }
            petros-axum = { path = "${petros}/crates/petros-axum" }
          '';

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

          # The Android SDK, pinned to what Expo SDK 57 asks gradle for.
          #
          # Google publishes these for `linux-x86_64` and nothing else, so on an
          # ARM machine they run under qemu — which works, transparently, and
          # inside a build sandbox, provided `binfmt_misc` is registered for
          # x86_64. `nix build .#apk` says so plainly when it is not.
          androidSdk =
            let
              x86 = import nixpkgs {
                system = "x86_64-linux";
                config = {
                  allowUnfree = true;
                  android_sdk.accept_license = true;
                };
              };
            in
            (x86.androidenv.composeAndroidPackages {
              # The versions the CI workflow pins, for the same reason: so that
              # `cargo ndk` and the Android Gradle Plugin look at one NDK rather
              # than two, and a new image cannot change the build.
              ndkVersions = [ "27.1.12297006" ];
              platformVersions = [ "36" ];
              buildToolsVersions = [ "35.0.0" "36.0.0" ];
              # The turbo module has an `externalNativeBuild`, so gradle needs
              # CMake as well as the NDK.
              cmakeVersions = [ "3.22.1" ];
              includeNDK = true;
              includeEmulator = false;
              includeSystemImages = false;
            }).androidsdk;

          # Gradle, at the version the wrapper asks for. The wrapper would
          # download it, which a sandbox cannot do; 25.05 ships 8.14.3 and the
          # generated project wants 9.3.1.
          gradle9 = pkgs.stdenv.mkDerivation {
            pname = "gradle";
            version = "9.3.1";
            src = pkgs.fetchzip {
              url = "https://services.gradle.org/distributions/gradle-9.3.1-bin.zip";
              hash = "sha256-BrrTDxZWrXyIZ/U4gBclxGLRqHv/cZy94/Wy8DrZ6nE=";
            };
            nativeBuildInputs = [ pkgs.makeWrapper ];
            installPhase = ''
              mkdir -p $out
              cp -r . $out/gradle
              makeWrapper $out/gradle/bin/gradle $out/bin/gradle \
                --set JAVA_HOME ${pkgs.jdk17}
            '';
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
            outputHash = "sha256-usHdiS9E96QuhIj38q8xi5jPn9nLKZ57KN1jJGdWpOA=";
          };

          # The Android development build.
          #
          # Everything Google publishes for Android is a `linux-x86_64` binary
          # — the NDK's clang, aapt2, d8, CMake — so on an ARM machine this
          # depends on `binfmt_misc` being registered for x86_64. That works,
          # including inside a build sandbox, and it is slow: the emulated
          # compile of SQLite's amalgamation alone is about a minute per ABI.
          #
          # Not a fixed-output derivation, and it cannot be one: an APK is a zip
          # and a signed one at that, so it is not reproducible byte-for-byte.
          # Everything it needs from the network is fetched by a derivation that
          # *is* — `expoModules` and `gradleDeps` — and the build itself runs
          # offline.
          apk = pkgs.stdenv.mkDerivation {
            name = "harken-debug-apk";
            src = workspace;

            nativeBuildInputs = [
              toolchain
              gradle9
              androidSdk
              pkgs.cargo-ndk
              pkgs.jdk17
              pkgs.nodejs
              pkgs.python3
              pkgs.unzip
              pkgs.which
            ];

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
            CC_aarch64_unknown_linux_gnu = "gcc";
            AR_aarch64_unknown_linux_gnu = "ar";

            buildPhase = ''
              runHook preBuild

              export HOME=$TMPDIR
              export CARGO_HOME=$TMPDIR/cargo

              if ! ${pkgs.coreutils}/bin/test -e /proc/sys/fs/binfmt_misc/x86_64-linux; then
                echo "" >&2
                echo "This needs to run x86_64 binaries: everything Google ships" >&2
                echo "for Android is linux-x86_64 only. On NixOS:" >&2
                echo "" >&2
                echo "  boot.binfmt.emulatedSystems = [ \"x86_64-linux\" ];" >&2
                echo "" >&2
                exit 1
              fi

              echo "--- the mutator module"
              ./scripts/mutators.sh

              echo "--- node_modules"
              ln -s ${expoModules} clients/expo/node_modules

              echo "--- the engine, cross-compiled (emulated; this is the slow part)"
              cd clients/expo/modules/harken-native
              ../../node_modules/.bin/ubrn build android \
                --config ubrn.config.yaml --and-generate --release
              cd ../../../..

              runHook postBuild
            '';

            installPhase = ''
              runHook preInstall
              mkdir -p $out
              cp -r clients/expo/modules/harken-native/android/src/main/jniLibs $out/ || true
              runHook postInstall
            '';
          };

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

      formatter = forAllSystems (system: nixpkgs.legacyPackages.${system}.nixpkgs-fmt);
    };
}
