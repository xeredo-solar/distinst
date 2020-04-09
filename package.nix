{ stdenv
, rustPlatform
, parted
, pkgconfig
, dbus
, gettext
, fetchFromGitHub
, lib
}:

let
  gitignoreSrc = fetchFromGitHub {
    owner = "hercules-ci";
    repo = "gitignore";
    # put the latest commit sha of gitignore Nix library here:
    rev = "2ced4519f865341adcb143c5d668f955a2cb997f";
    # use what nix suggests in the mismatch message here:
    sha256 = "sha256-X8xHVRr8N6SzI8Ju87V+A75r3ZwF+CEuXcx5nfZbhTk=";
  };
  inherit (import gitignoreSrc { inherit lib; }) gitignoreSource;
in
rustPlatform.buildRustPackage rec {
  pname = "distinst";
  version = "0.0.1";

  src = gitignoreSource ./.;

  cargoSha256 = "sha256-f6g8gZCmKyTIhd+tnUf0t29PSRDGhSMWqZTGlwp/Hbk=";

  nativeBuildInputs = [
    pkgconfig
    gettext
  ];

  buildInputs = [
    parted
    dbus
  ];

  meta = with stdenv.lib; {
    description = "An installer backend";
    homepage = "https://github.com/pop-os/distinst";
    license = licenses.lgpl3;
    maintainers = with maintainers; [ mkg20001 ];
    platforms = [ "x86_64-linux" ];
  };
}
