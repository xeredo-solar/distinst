{
  description = "distinst patched for solarOS";

  inputs.solaros.url = "github:ssd-solar/solaros-nix/flake";

  outputs = { self, nixpkgs, solaros }: {
    defaultPackage = solaros.lib.forAllSystems({ pkgs, ... }: pkgs.callPackage ./package.nix { inherit nixpkgs; /* TODO: inherit from solar */ });
  };
}
