# The domain: the crate whose mutations are the wasm module, and the
# workspace's vendored dependencies.
{
  perSystem = { script, ... }: {
    petros = {
      # The hash of `cargo vendor` over `Cargo.lock`; nix prints the right one
      # when the lock moves.
      #
      # It moves for *any* change to the lock, including one that fetches
      # nothing new: the vendored result carries a copy of `Cargo.lock`, so
      # adding a dependency edge to a crate already in the tree moves it too.
      # `serve`'s Home Assistant bridge did exactly that — `ureq`, `serde` and
      # `serde_json` were all already vendored for something else. And the
      # cover decode did it again in a second shape: `image` was already in the
      # lock, because iced's own `image` feature put it there, and becoming a
      # *direct* dependency of `harken-iced` still moved this. "No new crate,
      # so the vendored set is identical" is the reasoning to distrust; the
      # `cp Cargo.lock $out/Cargo.lock` above is why.
      #
      # And when it moves, nix may not *ask*: the old hash names a path already
      # in the store, so it substitutes the stale vendor directory and the
      # failure is nixpkgs' lockfile consistency check, which prints the diff
      # and no `got:`. Putting a hash nothing can have here is what forces the
      # fetch and gets the real one printed.
      cargoVendorHash = "sha256-DyXQD0fMlXi9mZsyntR1IrJPjXcjki/jzRppzx6VHKI=";
      mutators.crate = "harken";
      # Beside the domain rather than at the root, because it describes what is
      # in `functions.rs`. `nix flake check` holds every build to it.
      mutators.log = "domain/mutations.txt";
    };

    # Where the time goes in one mutation. Not part of the suite: it is a
    # measurement and it is slow.
    apps.latency.program = script "latency" {
      text = ''
        cargo test -p harken --all-features --release --test client_latency -- \
          --ignored --nocapture --test-threads=1 "$@"
      '';
    };
  };
}
