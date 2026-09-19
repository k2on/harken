# The phone. Everything about building it is `petros-js`'s mobile module,
# imported from the `@petros/client` revision `package.json` pins — the same
# repository as the client the screens call, so the two cannot drift. What is
# left here is what only this app knows: its name, its hashes, and what the
# gradle layer must not see. `eas.nix` beside this is the EAS profiles.
let
  pin = (builtins.fromJSON (builtins.readFile ../package.json)).dependencies."@petros/client";
  petrosJs = builtins.fetchGit {
    url = "https://github.com/k2on/petros-js";
    rev = builtins.elemAt (builtins.match "github:k2on/petros-js#([0-9a-f]+)" pin) 0;
    allRefs = true;
  };
in
{
  imports = [ "${petrosJs}/nix/app" ];

  perSystem = {
    mobile = {
      name = "harken";

      # One hash per platform, because the content really is different: bun
      # resolves the optional dependencies that carry native binaries for the
      # machine it installs on. Both move whenever `package.json` or
      # `bun.lock` does, and each can only be computed on the machine it
      # belongs to. When one goes stale, nix prints the right one —
      #
      # **— on a store that does not already hold the old answer.** This is a
      # fixed-output derivation, and nix decides whether one needs building
      # from its *hash alone*: a store that has an output with this hash has
      # this derivation built, whatever `package.json` says now. So a pin bump
      # beside an unmoved hash is not a build that fails and names itself. On
      # CI it is a build that restores the last run's store, finds yesterday's
      # `node_modules` under today's hash, and ships it green: v0.1.10 carried
      # an `@petros/client` without the `tick` the phone's listening pump
      # rides, and every phone on it stood in the room saying nothing. The
      # mechanism only fires cold. **Move both hashes with the pin, even if
      # one of them has to be a wrong value you cannot compute here** — a
      # wrong hash fails and prints the right one; an old hash passes.
      #
      # Nothing can compute a hash for a machine it is not on, so from an
      # aarch64 laptop the x86_64 one is asked of ark — which is x86_64, and
      # is also the Gitea runner, so its store *is* the one holding the stale
      # output. `--rebuild` is what makes nix build it again regardless and
      # then say what it got:
      #
      #   ssh ark nix shell nixpkgs#git --command \
      #     nix build github:k2on/harken/<rev>#expoModules --rebuild --no-link
      nodeModulesHash = {
        aarch64-linux = "sha256-uxYBKb8kJva4e4JfTU5S0TOE3YpvvRc1rNhGcnm0KLs=";
        x86_64-linux = "sha256-glX2/MhnM+R/8ZH7eeHLAIEpyk4U/y4I8QFq2eRrYHI=";
      };

      # The generator ships no lockfile — the npm package is the built CLI and
      # its Rust sources — so `ubrn-Cargo.lock` is committed here and the
      # vendored result is pinned. Both move when `bun.lock` moves the
      # generator's version.
      ubrnHash = "sha256-YdwO0AOhoQjRbo5psMAxSSCRM7Ok8OsrjcAE13QMXP0=";

      # Gradle 9.3.1, and the version is not negotiable: `pkgs.gradle_9` is
      # 9.4.1, which carries `kotlin-stdlib-2.3.0`, and Expo SDK 57's gradle
      # plugins are compiled with Kotlin 2.1.0, which reads metadata up to
      # 2.2.0. It arrives as `Internal compiler error` against Expo's own
      # settings plugin, naming no versions.
      gradle = {
        version = "9.3.1";
        hash = "sha256-smbV/2uQ6tptw7IMsJDjcxMC5VOifF0+TfHw12vq/wY=";
      };

      # The gradle layer's source is this directory minus what changes per
      # commit: prose, and a route `app.config.ts` asks `expo-router` for,
      # never rendered — a debug APK bundles no JavaScript at all.
      stateSrcExclude = [ "README.md" "LICENSE" ];
      stateSrcFiles."src/app/index.tsx" = "export default function Index() { return null; }\n";
    };

    # `nix run .#mutators` writes the TypeScript Metro watches here.
    petros.mutators.ts = "mobile/src/mutators.gen.ts";
  };
}
