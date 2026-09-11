#!/usr/bin/env bash
# Re-record gradle's Maven graph into `gradle-deps.json`.
#
# Working out what a gradle build fetches is a Turing-complete question, so
# nixpkgs answers it empirically: run the build once behind a recording proxy
# and keep what came back. This drives that, and is the only thing that should
# ever write `gradle-deps.json`.
#
# Run it when the build's dependencies move — an Expo or React Native bump, a
# new config plugin, a gradle or AGP version change. A stale recording is noisy
# rather than silent: the replayed build fails on whatever it is missing.
#
# Needs the network, and takes about as long as a full APK build, because it is
# one. Anything after `--` is passed to `nix build`, which is how you supply
# `--override-input` on a machine that cannot reach every fetcher.
set -euo pipefail

repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

# `fetchDeps` reads this at evaluation time, so it has to parse before it has
# anything in it.
[ -s "$repo/gradle-deps.json" ] || echo '{}' >"$repo/gradle-deps.json"

script=$(nix build --no-link --print-out-paths \
  "$repo#apk-debug.mitmCache.updateScript" "$@")

# The script writes its output relative to the working directory: the path it
# was given is inside the flake's source in the store, and it keeps everything
# after the store path.
cd "$repo"

# Without bubblewrap. It is there to stop a careless build script writing to
# `$HOME`, and it clears the environment — which on a machine that reaches the
# network through a proxy leaves nix unable to fetch what it does not have.
USE_BWRAP=0 "$script"

echo "wrote $repo/gradle-deps.json"
