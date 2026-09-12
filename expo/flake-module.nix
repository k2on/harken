# The phone. Everything about building it is `petros-js`'s — the `ubrn`
# generator, the two-layer cross-compile, the Expo project's gradle state
# layer, the SDK — and what is left here is what only this app knows: its
# files, its hashes, and which of its crates are its own.
#
# Every derivation on the way is a package, because each is worth building
# alone when something is slow or broken; the names are what the workflows
# gc-root, so they stay.
{ inputs, ... }: {
  perSystem = { pkgs, toolchain, petrosJs, sources, self', ... }:
    let
      app = petrosJs.mkApp {
        name = "harken";
        crate = "harken";
        src = sources.workspace;
        engineSrc = sources.engineWorkspace;
        appDir = "expo";
        appSrc = ./.;
        appCrates = [ "domain" "server" "iced" ];

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

        # Gradle's Maven graph, recorded by `harken gradle-deps`.
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

      devShells = {
        # Everything the default shell has, plus the two tools that turn Rust
        # into an Android library.
        #
        #     nix develop .#android -c harken expo-android
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
              echo "harken android shell — harken expo-android"
              echo "  sdk: $ANDROID_HOME"
              echo "  ndk: ''${ANDROID_NDK_HOME:-<none found>}"
            fi
          '';
        };
      };
    };
}
