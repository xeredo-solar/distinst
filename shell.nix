let
  solaros = (builtins.fetchTarball https://nix.ssd-solar.dev/dev/solaros/nixexprs.tar.xz);
  solarnixpkgs = (builtins.fetchTarball https://nix.ssd-solar.dev/dev/nixpkgs/nixexprs.tar.xz);
in
with (import "${solaros}/dev.nix");
callPackage ./package.nix {
  shellHookAppend = ''
    export NIX_PATH="solaros=${solaros}:nixpkgs=${solarnixpkgs}"
  '';
}
