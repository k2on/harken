# The domain: the wasm module a mutation lives in, and the TypeScript generated
# from it — `mutators.sh` as derivations, built by the engine's own library.
#
# `nix build .#mutators` is the derivation; `nix run .#mutators` is the same two
# steps into the working tree, which is what Metro watches and what a local
# `cargo test` reads.
{ inputs, ... }: {
  perSystem = { pkgs, toolchain, petros, sources, script, self', ... }: {
    packages = {
      petrosCodegen = petros.mkCodegen { inherit toolchain; src = inputs.petros; };
      mutators = petros.mkMutators {
        name = "harken-mutators";
        inherit toolchain;
        inherit (sources) cargoDeps;
        src = sources.engineWorkspace;
        codegen = self'.packages.petrosCodegen;
        crate = "harken";
      };
    };

    apps = {
      # Into the tree: `expo/src/mutators.gen.ts` and the wasm under `target/`.
      mutators.program = script "mutators" { text = "./domain/mutators.sh"; };
      # …on every save. Leave it running beside `bun start`.
      mutators-watch.program = script "mutators-watch" {
        runtimeInputs = [ pkgs.watchexec ];
        text = ''
          echo "watching domain/ — save a file and check the phone"
          watchexec --project-origin . --watch domain --exts rs \
            --on-busy-update=restart -- ./domain/mutators.sh
        '';
      };
      # Where the time goes in one mutation. Not part of the suite: it is a
      # measurement and it is slow.
      latency.program = script "latency" {
        text = ''
          cargo test -p harken --all-features --release --test client_latency -- \
            --ignored --nocapture --test-threads=1 "$@"
        '';
      };
    };

    workspace.packages = [ pkgs.watchexec ];
  };
}
