{ stdenv
, parted
, pkgconfig
, dbus
, rust
, gettext
, fetchFromGitHub
, lib
, callPackage
, darwin
, llvmPackages
, libxml2
, glib
, libunistring
, writeShellScriptBin
, glibc
, tzdata
, nixStable
, makeWrapper
, nixpkgs
, conf-tool
, shellHookAppend ? "" # only used by shell.nix to add channels to NIX_PATH
, nix ? nixStable
, makeRustPlatform
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

  libcroco = callPackage ./libcroco.nix { };

  ccForBuild="${stdenv.cc}/bin/${stdenv.cc.targetPrefix}cc";
  cxxForBuild="${stdenv.cc}/bin/${stdenv.cc.targetPrefix}c++";
  ccForHost="${stdenv.cc}/bin/${stdenv.cc.targetPrefix}cc";
  cxxForHost="${stdenv.cc}/bin/${stdenv.cc.targetPrefix}c++";
  releaseDir = "target/${rustTarget}/release";
  rustTarget = rust.toRustTarget stdenv.hostPlatform;

  nixPatched = nix.overrideAttrs(p: p // {
    patches = [
      ./json-progress.patch
    ] ++
      (
        if builtins.substring 0 1 nix.version == "3"
        then [ ./disable-experimental-feature-check.patch ]
        else []
      );
  });

  tools =
    with import "${path}/nixos" { configuration = {
      nixpkgs.overlays = [
        (self: super: {
          nix = nixPatched;
          })
      ];
      system = "x86-64_linux";
    }; };
    with config.system.build;
      # https://github.com/NixOS/nixpkgs/pull/87182/files?file-filters%5B%5D=.nix&file-filters%5B%5D=.sh&file-filters%5B%5D=.xml
      /*
      -    nix build --out-link "$outLink" --store "$mountPoint" "${extraBuildFlags[@]}" \
      +    nix-build --out-link "$outLink" --store "$mountPoint" "${extraBuildFlags[@]}" \
               --extra-substituters "$sub" \
      -        -f '<nixpkgs/nixos>' system -I "nixos-config=$NIXOS_CONFIG" ${verbosity[@]} ${buildLogs}
      +        '<nixpkgs/nixos>' -A system -I "nixos-config=$NIXOS_CONFIG" ${verbosity[@]}
      */
      [ nixPatched ] ++ [ nixos-generate-config (nixos-install.overrideAttrs(a: a // {
          postInstall = ''
            sed 's|nix-build|nix build|g' -i $out/bin/nixos-install
            sed "s| '<nixpkgs/nixos>' -A system|-f '<nixpkgs/nixos>' system|g" -i $out/bin/nixos-install
          '';
          })) conf-tool /* nixos-enter manual.manpages */] ++
        [ (writeShellScriptBin "nixos-install-wrapped" (builtins.readFile ./install-wrapper.sh)) ];
in
with rust; (makeRustPlatform packages.stable).buildRustPackage rec {
  inherit tools;

  pname = "distinst";
  version = "0.0.1";

  src = gitignoreSource ./.;

  cargoSha256 = "sha256-0JZrFJ/b+HHZyyVjppX1dcjiN4YSz6FakfONZxSAeT8=";

  nativeBuildInputs = [
    pkgconfig
    makeWrapper
    gettext
  ];

  buildInputs = [
    parted
    dbus
    llvmPackages.clang
    llvmPackages.libclang
    tzdata

    # shadow-deps of gettext rust
    libxml2
    libcroco
    glib
    libunistring
  ];

  preBuild = ''
    export LIBCLANG_PATH=${llvmPackages.libclang}/lib
    export CFLAGS="$CFLAGS -Wno-error=format-security -Wno-error"
    export BINDGEN_EXTRA_CLANG_ARGS="-I${parted.dev}/include -I${glibc.dev}/include -I${llvmPackages.libclang.out}/lib/clang/${llvmPackages.libclang.version}/include" # documented in the code as always... https://github.com/rust-lang/rust-bindgen/pull/1537 # but seems broken due to https://github.com/rust-lang/rust-bindgen/issues/1723
    echo 'pub static ZONE_INFO_LOCATION: &str = "${tzdata}/share/zoneinfo";' > crates/timezones/src/buildconfig.rs
  '';

  buildPhase = with builtins; ''
    runHook preBuild

    for m in cli ffi; do
      (
      set -x
      env \
        "CC_${rust.toRustTarget stdenv.buildPlatform}"="${ccForBuild}" \
        "CXX_${rust.toRustTarget stdenv.buildPlatform}"="${cxxForBuild}" \
        "CC_${rust.toRustTarget stdenv.hostPlatform}"="${ccForHost}" \
        "CXX_${rust.toRustTarget stdenv.hostPlatform}"="${cxxForHost}" \
        cargo build \
          --release \
          --target ${rustTarget} \
          --frozen \
          --manifest-path $m/Cargo.toml
      )
    done

    # rename the output dir to a architecture independent one
    mapfile -t targets < <(find "$NIX_BUILD_TOP" -type d | grep '${releaseDir}$')
    for target in "''${targets[@]}"; do
      rm -rf "$target/../../release"
      ln -srf "$target" "$target/../../"
    done

    runHook postBuild
  '';

  shellHook = ''
    ${preBuild}
    export PATH="${lib.makeBinPath tools}:$PATH"
    ${shellHookAppend}
  '';

  doCheck = false;

  installPhase = ''
    make VENDORED=1 DEBUG=0 RELEASE=release prefix=$out install
    wrapProgram $out/bin/distinst --prefix PATH : ${lib.makeBinPath tools}
  '';

  meta = with lib; {
    description = "An installer backend";
    homepage = "https://github.com/pop-os/distinst";
    license = licenses.lgpl3;
    maintainers = with maintainers; [ mkg20001 ];
    platforms = [ "x86_64-linux" ];
  };
}
