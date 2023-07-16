{
  description = "gnu-bionic-pipe";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-23.05";

  outputs = { self, nixpkgs }:

    let
      pkgs = import nixpkgs { system = "x86_64-linux"; };
      armGnuPkgs = pkgs.pkgsCross.aarch64-multiplatform;
      armBionicPkgs = pkgs.pkgsCross.aarch64-android-prebuilt;
      bionicEnd = armBionicPkgs.rustPlatform.buildRustPackage {
        name = "gnubionicpipe-bionic-end";
        src = ./.;
        cargoBuildFlags = "--package bionic-end";
        cargoLock.lockFile = ./Cargo.lock;
        buildType = "debug";
      };
      androidNdk = armBionicPkgs.buildPackages.androidndkPkgs.binaries;
      gnuEnd = armGnuPkgs.rustPlatform.buildRustPackage {
        name = "gnubionicpipe-gnu-end";
        src = ./.;
        preConfigure = ''
          ln -s ${androidNdk}/toolchain gnu-end/build-inputs/ndk-toolchain
          ln -s ${bionicEnd}/bin/bionic-end gnu-end/build-inputs/
        '';
        LIBCLANG_PATH = "${armBionicPkgs.buildPackages.clang.cc.lib}/lib";
        cargoBuildFlags = "--package gnu-end";
        cargoLock.lockFile = ./Cargo.lock;
        buildType = "debug";
      };

    in { defaultPackage.x86_64-linux = gnuEnd; };
}
