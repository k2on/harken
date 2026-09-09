#!/usr/bin/env sh
#
# Build the domain as a wasm module, and the TypeScript types that come out of
# it.
#
# This is the whole of what `crates/harken-wasm` and a pair of justfile lines
# used to be. It exists as a script because three different things need it and
# only one of them has `just`: a laptop running `just mutators`, the EAS build
# hook, and CI.
#
# What differs between them is exactly one thing — where `petros-codegen` comes
# from. Beside the checkout when there is one, so an engine edit is picked up
# without a commit; from the published branch otherwise, because a build
# container has no sibling directory.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

# `--no-default-features` is what keeps the engine, SQLite and serde_json out of
# the module. It is a flag on the build, not a property of a package, which is
# why a separate crate bought nothing.
cargo build -p harken --no-default-features \
  --target wasm32-unknown-unknown --profile mutators

wasm="$root/target/wasm32-unknown-unknown/mutators/harken.wasm"
out="$root/clients/expo/src/mutators.gen.ts"

if [ -f "$root/../petros/Cargo.toml" ]; then
  cargo run -q --manifest-path "$root/../petros/Cargo.toml" -p petros-codegen -- \
    "$wasm" "$out"
else
  # Kept in target/ so a second build in the same container does not pay for it
  # again. The branch has to match the one Cargo.toml resolves the engine from,
  # or the generator and the module disagree about the schema section.
  bin="$root/target/codegen/bin/petros-codegen"
  if [ ! -x "$bin" ]; then
    echo "--- installing petros-codegen (no sibling checkout)"
    cargo install --quiet --git https://github.com/k2on/petros --branch main \
      --root "$root/target/codegen" petros-codegen
  fi
  "$bin" "$wasm" "$out"
fi
