with (import <nixpkgs> {});
let
  solaros = (builtins.fetchTarball https://nix.ssd-solar.dev/dev/solaros/nixexprs.tar.xz);
  solarnixpkgs = (builtins.fetchTarball https://nix.ssd-solar.dev/dev/nixpkgs/nixexprs.tar.xz);
in
callPackage ./package.nix {
  conf-tool = ((import solaros).conf-tool);
  shellHookAppend = ''
    export NIX_PATH="solaros=${solaros}:nixpkgs=${solarnixpkgs}"
  '';
}
