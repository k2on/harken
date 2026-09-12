# The two libraries this app builds with. `petros-js`'s module imports the
# engine's, `expo.nix`'s and `android.nix`'s in turn, so this is the whole
# list — and each of them builds over this flake's `pkgs`, not its own.
{ inputs, ... }: {
  imports = [
    inputs.petros.flakeModules.default
    inputs.petros-js.flakeModules.default
  ];
}
