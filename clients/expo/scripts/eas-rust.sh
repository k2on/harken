#!/usr/bin/env bash
# The Rust half of an EAS build.
#
# EAS gives you two slots and no slot between them: `eas-build-pre-install` runs
# before node_modules exist, and `eas-build-post-install` runs after `expo
# prebuild`. That is fine here, because React Native autolinks at gradle
# *configure* time rather than at prebuild time — so a turbo module that appears
# during post-install is still found, and gradle is what runs next.
#
#   pre    install the toolchain rust-toolchain.toml asks for. No node_modules
#          yet, so nothing else can happen here.
#   post   build the mutator module, hand it to the bundler, then cross-compile
#          the engine and generate the turbo module for gradle to pick up.
#
# Run from the app directory; the cargo workspace is the repository root, which
# is found by walking up rather than by counting `..`, so moving either one does
# not silently break the build.
set -euo pipefail

stage="${1:?usage: eas-rust.sh pre|post}"

root="$PWD"
while [ ! -f "$root/Cargo.toml" ] && [ "$root" != "/" ]; do
  root="$(dirname "$root")"
done
if [ ! -f "$root/Cargo.toml" ]; then
  echo "eas-rust: no cargo workspace above $PWD" >&2
  exit 1
fi
app="$PWD"

case "$stage" in
  pre)
    echo "--- installing Rust for $(uname -m)"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
      | sh -s -- -y --default-toolchain none --profile minimal --no-modify-path
    # shellcheck disable=SC1091
    . "$HOME/.cargo/env"
    # `rustup show` in the workspace provisions exactly what rust-toolchain.toml
    # names: the channel, the components and every target. That file stays the
    # one place a version is written down, here as much as in the devshell.
    (cd "$root" && rustup show)
    cargo install cargo-ndk --locked --version '^3'
    echo "--- $(rustc --version)"
    ;;

  post)
    # shellcheck disable=SC1091
    . "$HOME/.cargo/env"

    # cargo-ndk looks for these in turn. EAS images set ANDROID_SDK_ROOT and
    # usually ANDROID_NDK_HOME too; derive it when they do not, rather than
    # failing several minutes into a build.
    if [ -z "${ANDROID_NDK_HOME:-}" ] && [ -d "${ANDROID_SDK_ROOT:-}/ndk" ]; then
      ANDROID_NDK_HOME="$(ls -d "$ANDROID_SDK_ROOT"/ndk/* | sort -V | tail -1)"
      export ANDROID_NDK_HOME
    fi
    echo "--- ndk: ${ANDROID_NDK_HOME:-<none>}"

    echo "--- building the mutator module"
    # The same script `just mutators` runs. It writes src/mutators.gen.ts, which
    # is gitignored and therefore not in the upload — the bundler needs it to
    # exist before it runs, which is now. There is no sibling petros checkout
    # here, so the script installs the generator from the published branch.
    "$root/scripts/mutators.sh"

    echo "--- cross-compiling the engine and generating the turbo module"
    cd "$app/modules/harken-native"
    "$app/node_modules/.bin/ubrn" build android \
      --config ubrn.config.yaml --and-generate --release
    ;;

  *)
    echo "eas-rust: unknown stage \"$stage\"" >&2
    exit 1
    ;;
esac
