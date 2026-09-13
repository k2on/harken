# The domain: the crate whose mutations are the wasm module, and the
# workspace's vendored dependencies.
{
  perSystem = { script, ... }: {
    petros = {
      # The hash of `cargo vendor` over `Cargo.lock`; nix prints the right one
      # when the lock moves.
      cargoVendorHash = "sha256-rdkdTsilXfgzYgXPAFWACDzkH1Psn/8wHLRmJRHYIuw=";
      mutators.crate = "harken";
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
