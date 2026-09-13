{
  description = "harken — a self-hosted, local-first music system";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
    petros = {
      url = "github:k2on/petros";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  # Every `*.nix` under this tree is a flake-parts module; each directory's
  # `nix/` is what is true about it. What they share is the engine's.
  outputs = inputs: inputs.petros.lib.mkApp inputs ./.;
}
