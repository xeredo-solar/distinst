{
  description = "distinst patched for solarOS";

  inputs.solar.url = "github:ssd-solar/solaros-nix/flake";

  outputs = { self, nixpkgs, solar }: {
    defaultPackage = solar.lib.forAllSystems({ pkgs, ... }: pkgs.callPackage ./package.nix { nixpkgs = solar.lib.nixpkgs; });
  };
}
