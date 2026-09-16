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
      # `serde_json` were all already vendored for something else.
      #
      # One round: the lock moved and the old hash is already in the store, so
      # nix substituted the *stale* vendor directory instead of rebuilding it
      # and the failure was the lockfile consistency check rather than a hash
      # mismatch — which prints no `got:` to copy. A hash nothing can have
      # forces the fetch, and the next run prints the real one. Written out
      # rather than as `lib.fakeHash` so that a wrong guess about what
      # `perSystem` is handed cannot cost the round this is spending.
      cargoVendorHash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
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
