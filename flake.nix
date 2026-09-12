{
  description = "harken — a self-hosted, local-first music system";

  inputs = {
    # Pinned to a release branch here; the exact revision lives in flake.lock,
    # which is what actually makes the shell reproducible. Run `nix flake update`
    # deliberately, never as a side effect.
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
    import-tree.url = "github:vic/import-tree";

    # The engine, for building. `Cargo.toml` names it by git and
    # `.cargo/config.toml` patches it to the checkout next door for local work
    # — and that patch is what shaped the committed `Cargo.lock`, which records
    # petros as a path with no revision at all. So a hermetic build has to
    # supply the same patch, pointing at a pinned copy instead of a sibling
    # directory. This is that copy; `flake.lock` pins it, and the packages check
    # it against the revision in `Cargo.toml` rather than trusting two pins to
    # stay equal on their own.
    #
    # A flake now rather than `flake = false`, because the engine brings the
    # nix that knows its crates, its patch and its code generator.
    petros = {
      url = "github:k2on/petros";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-overlay.follows = "rust-overlay";
      inputs.flake-parts.follows = "flake-parts";
      inputs.import-tree.follows = "import-tree";
    };

    # How a Petros app reaches a phone: `ubrn`, the two-layer cross-compile,
    # and — through its own `expo.nix` and `android.nix` inputs — the Expo
    # project's gradle layer and the SDK. Every `follows` here is what keeps
    # this closure to one nixpkgs: each of those flakes follows the one above
    # it, and the top of the chain is this file.
    petros-js = {
      url = "github:k2on/petros-js";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-parts.follows = "flake-parts";
      inputs.import-tree.follows = "import-tree";
      inputs.petros.follows = "petros";
    };
  };

  # Dendritic: every file under `modules/` is a flake-parts module, and this
  # file names no outputs. `modules/imports.nix` brings in the engine's and
  # `petros-js`'s modules, which put `petros`, `petrosJs`, `expo` and
  # `android` in scope of every `perSystem`.
  outputs = inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } (inputs.import-tree ./modules);
}
