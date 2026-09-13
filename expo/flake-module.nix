# The phone. Everything about building it is `petros-js`'s — the `ubrn`
# generator, the two-layer cross-compile, the Expo project's gradle state
# layer, the SDK — and what is left here is what only this app knows: its
# files, its hashes, and which of its crates are its own.
#
# Two files in this directory are generated from here rather than written,
# because they would otherwise repeat what nix already knows: `eas.json`
# (the Rust version and targets an EAS container installs) and the turbo
# module's `ubrn.config.yaml` (the ABIs the APK is built for).
{ inputs, ... }: {
  perSystem = { pkgs, lib, toolchain, rustVersion, petrosJs, sources, script, self', ... }:
    let
      # arm64 covers every device made this decade; x86_64 is what an emulator
      # on a normal laptop runs. Named once: the APK's gradle builds C++ for
      # these, `ubrn` builds the Rust for them, and EAS installs the targets.
      abis = [ "arm64-v8a" "x86_64" ];
      rustTarget = { arm64-v8a = "aarch64-linux-android"; x86_64 = "x86_64-linux-android"; };

      app = petrosJs.mkApp {
        name = "harken";
        crate = "harken";
        src = sources.workspace;
        engineSrc = sources.engineWorkspace;
        appDir = "expo";
        appSrc = ./.;
        appCrates = [ "domain" "server" "iced" ];
        archs = abis;

        inherit toolchain;
        petrosSrc = inputs.petros;
        vendor = sources.cargoDeps;
        mutators = self'.packages.mutators;

        # One hash per platform, because the content really is different:
        # bun resolves the optional dependencies that carry native binaries
        # for the machine it installs on. Both move whenever `package.json`
        # or `bun.lock` does, and each can only be computed on the machine it
        # belongs to. When one goes stale, nix prints the right one.
        nodeModulesHash = {
          aarch64-linux = "sha256-Lo8p11fynW14olU8Sz5HAPQFrwtEe/xS/4t5v9qj5Hc=";
          x86_64-linux = "sha256-nKYJZdiKJQYxI2gSh3V5tQ7Q16Bj9U0Rlk/Tg1kw7BM=";
        };

        # The generator ships no lockfile — the npm package is the built CLI
        # and its Rust sources, and cargo is expected to resolve wherever it
        # runs. So one is committed here, and the vendored result is pinned
        # by hash. Both move when `bun.lock` moves the generator's version.
        ubrnLock = ./ubrn-Cargo.lock;
        ubrnHash = "sha256-YdwO0AOhoQjRbo5psMAxSSCRM7Ok8OsrjcAE13QMXP0=";

        # Gradle's Maven graph, recorded by `nix run .#gradle-deps`.
        gradleDeps = ./gradle-deps.json;

        # Gradle 9.3.1, and the version is not negotiable: `pkgs.gradle_9`
        # is 9.4.1, which carries `kotlin-stdlib-2.3.0`, and Expo SDK 57's
        # gradle plugins are compiled with Kotlin 2.1.0, which reads metadata
        # up to 2.2.0. It arrives as `Internal compiler error` against Expo's
        # own settings plugin, naming no versions.
        gradle = {
          version = "9.3.1";
          hash = "sha256-smbV/2uQ6tptw7IMsJDjcxMC5VOifF0+TfHw12vq/wY=";
        };

        # The gradle state layer's source is this directory minus what changes
        # per commit or belongs to another derivation. EAS's config and the
        # scripts are neither gradle's business nor stable.
        stateSrcExclude = [ "README.md" "LICENSE" "eas.json" "eas-rust.sh" "gradle-deps.json" "flake-module.nix" ];
        # A route, because `app.config.ts` asks for `expo-router` and its
        # config plugin would rather find one. Never rendered.
        stateSrcFiles = {
          "src/app/index.tsx" = "export default function Index() { return null; }\n";
        };
      };

      # `bun install` is the app's; the library's binaries are hoisted here.
      ubrn = "./expo/node_modules/.bin/ubrn";
      expoInstall = "(cd expo && bun install)";
    in
    {
      packages = {
        androidSdk = app.sdk;
        gradle9 = app.gradle;
        expoModules = app.nodeModules;
        inherit (app) ubrn ndk-check gradleState apk-debug apk-release apk;
        androidEngine = app.engine;
        androidDeps = app.deps;
        androidDeps-debug = app.deps-debug;
      };

      files.file = {
        "expo/eas.json".source = (pkgs.formats.json { }).generate "eas.json" {
          cli = { version = ">= 12.0.0"; appVersionSource = "remote"; };
          build = {
            base = {
              android.image = "latest";
              env = {
                RUST_VERSION = rustVersion;
                RUST_TARGETS = lib.concatMapStringsSep " " (abi: rustTarget.${abi}) abis;
                CARGO_TERM_COLOR = "never";
              };
            };
            development = {
              extends = "base";
              developmentClient = true;
              distribution = "internal";
              android.buildType = "apk";
              env.APP_VARIANT = "development";
            };
            preview = {
              extends = "base";
              distribution = "internal";
              android.buildType = "apk";
              env.APP_VARIANT = "production";
            };
            production = {
              extends = "base";
              autoIncrement = true;
              android.buildType = "app-bundle";
              env.APP_VARIANT = "production";
            };
          };
          submit.production = { };
        };
        # How the generated turbo module is wired to the Rust: the domain crate
        # itself, with the binding layer switched on. `--features foreign` is
        # off by default so the desktop client and the server do not build
        # uniffi and a wasm interpreter to reach the same `apply`.
        "expo/modules/harken-native/ubrn.config.yaml".source = (pkgs.formats.yaml { }).generate "ubrn.config.yaml" {
          rust = { directory = "../../.."; manifestPath = "domain/Cargo.toml"; };
          bindings = { cpp = "cpp/generated"; ts = "src/generated"; };
          android = {
            directory = "android";
            targets = abis;
            apiLevel = 24;
            cargoExtras = [ "--features" "foreign" ];
          };
          ios = {
            directory = "ios";
            targets = [ "aarch64-apple-ios" "aarch64-apple-ios-sim" ];
            cargoExtras = [ "--features" "foreign" ];
          };
        };
      };

      apps = {
        # Regenerate the client's TypeScript and C++ from a host build of the
        # domain crate, then typecheck the app against it — no NDK, no Xcode,
        # a couple of seconds. ubrn writes new files and never removes old
        # ones, so what was generated before is wiped first.
        bindings.program = script "bindings" {
          runtimeInputs = [ pkgs.bun pkgs.nodejs_22 ];
          text = ''
            ${expoInstall}
            cargo build -p harken --features foreign
            case "$(uname -s)" in Darwin) lib=libharken.dylib ;; *) lib=libharken.so ;; esac
            rm -rf expo/modules/harken-native/{src/generated,cpp/generated,android/src/main/java}
            ${ubrn} generate jsi bindings "target/debug/$lib" --library --no-format \
              --ts-dir expo/modules/harken-native/src/generated \
              --cpp-dir expo/modules/harken-native/cpp/generated
            (cd expo/modules/harken-native && ../../node_modules/.bin/ubrn generate jsi turbo-module \
              --config ubrn.config.yaml --native-bindings harken)
            (cd expo && ./node_modules/.bin/tsc --noEmit)
          '';
        };
        # Build the Rust for a phone, regenerate, and run the app. Needs the
        # SDK and the NDK: `nix develop .#android -c nix run .#expo-android`.
        expo-android.program = script "expo-android" {
          runtimeInputs = [ pkgs.bun pkgs.nodejs_22 pkgs.cargo-ndk ];
          text = ''
            ${expoInstall}
            (cd expo/modules/harken-native && ../../node_modules/.bin/ubrn build android \
              --config ubrn.config.yaml --and-generate --release)
            (cd expo && bunx expo prebuild --platform android --clean && bunx expo run:android)
          '';
        };
        expo-ios.program = script "expo-ios" {
          runtimeInputs = [ pkgs.bun pkgs.nodejs_22 ];
          text = ''
            ${expoInstall}
            (cd expo/modules/harken-native && ../../node_modules/.bin/ubrn build ios \
              --config ubrn.config.yaml --and-generate --release)
            (cd expo && bunx expo prebuild --platform ios --clean && bunx expo run:ios)
          '';
        };
        # Re-record gradle's Maven graph into `expo/gradle-deps.json`. Working
        # out what a gradle build fetches is a Turing-complete question, so
        # nixpkgs runs the build once behind a recording proxy and keeps what
        # came back; this drives that. It needs the network and costs a full
        # build, which is why the `gradle-deps` workflow runs it on a runner.
        # Anything after `--` goes to `nix build`.
        gradle-deps.program = pkgs.writeShellApplication {
          name = "gradle-deps";
          runtimeInputs = [ pkgs.git ];
          text = ''
            cd "$(git rev-parse --show-toplevel)"
            [ -s expo/gradle-deps.json ] || echo '{}' > expo/gradle-deps.json
            script=$(nix build --no-link --print-out-paths ".#apk-debug.mitmCache.updateScript" "$@")
            # Without bubblewrap: it clears the environment, which on a
            # machine behind a proxy leaves nix unable to fetch.
            USE_BWRAP=0 "$script"
            echo "wrote expo/gradle-deps.json"
          '';
        };
      };

      # Metro and the Expo CLI are node programs even when bun runs them.
      workspace.packages = [ pkgs.bun pkgs.nodejs_22 pkgs.eas-cli ];

      devShells = {
        android = pkgs.mkShell {
          inputsFrom = [ self'.devShells.default ];

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
              echo "harken android shell — nix run .#expo-android"
              echo "  sdk: $ANDROID_HOME"
              echo "  ndk: ''${ANDROID_NDK_HOME:-<none found>}"
            fi
          '';
        };
      };
    };
}
