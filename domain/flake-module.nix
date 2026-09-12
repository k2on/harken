# The domain compiled to wasm, and the TypeScript generated from it — this is
# `mutators.sh` as derivations, built by the engine's own library from the
# engine `flake.lock` pins.
#
# The checks need it and not merely the tests do: `foreign_peer!` does
# `include_bytes!` of the module, so *compiling* the crate with
# `--all-features` needs the file to exist. That is why `harken lint` depends
# on `mutators`, and why a check that skipped it would fail in a way that
# reads like a broken checkout.
{ inputs, ... }: {
  perSystem = { toolchain, petros, sources, self', ... }: {
    packages = {
      # The generator, exposed because `nix build .#mutators` is how you find
      # out whether it is the module or your own code that is broken.
      petrosCodegen = petros.mkCodegen {
        inherit toolchain;
        src = inputs.petros;
      };

      mutators = petros.mkMutators {
        name = "harken-mutators";
        inherit toolchain;
        inherit (sources) cargoDeps;
        src = sources.checkWorkspace;
        codegen = self'.packages.petrosCodegen;
        crate = "harken";
      };
    };
  };
}
