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
        appDir = "clients/expo";
        appSrc = ../clients/expo;
        appCrates = [ "crates/harken" "crates/server" "clients/iced" ];

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
        ubrnLock = ../clients/expo/ubrn-Cargo.lock;
        ubrnHash = "sha256-YdwO0AOhoQjRbo5psMAxSSCRM7Ok8OsrjcAE13QMXP0=";

        # Gradle's Maven graph, recorded by `scripts/gradle-deps.sh`.
        gradleDeps = ../gradle-deps.json;

        # Gradle 9.3.1, and the version is not negotiable: `pkgs.gradle_9`
        # is 9.4.1, which carries `kotlin-stdlib-2.3.0`, and Expo SDK 57's
        # gradle plugins are compiled with Kotlin 2.1.0, which reads metadata
        # up to 2.2.0. It arrives as `Internal compiler error` against Expo's
        # own settings plugin, naming no versions.
        gradle = {
          version = "9.3.1";
          hash = "sha256-smbV/2uQ6tptw7IMsJDjcxMC5VOifF0+TfHw12vq/wY=";
        };

        # The gradle state layer's source is `clients/expo` minus what changes
        # per commit or belongs to another derivation. EAS's config and the
        # scripts are neither gradle's business nor stable.
        stateSrcExclude = [ "README.md" "LICENSE" "eas.json" "scripts" ];
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
    };
}
