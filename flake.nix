{
  description = "";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-24.05";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
      };
    };
  };

  outputs = {
    nixpkgs,
    flake-utils,
    rust-overlay,
    ...
  }: let
    overlays = [
      rust-overlay.overlays.default
      (final: prev: {
        rustToolchain = final.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
      })
    ];
  in
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {inherit system overlays;};
      rustPlatform = pkgs.makeRustPlatform {
        cargo = pkgs.rustToolchain;
        rustc = pkgs.rustToolchain;
      };
    in {
      formatter = pkgs.alejandra;

      devShells = {
        default = with pkgs;
          mkShell {
            buildInputs = [
              libiconv
              pkg-config
              rustToolchain
              (cargo-watch.override {
                inherit rustPlatform;
              })
            ];
          };
      };

      packages = let
        lib = pkgs.lib;
        package = (lib.importTOML ./Cargo.toml).package;
      in rec {
        ymsp =
          rustPlatform
          .buildRustPackage {
            pname = package.name;
            version = package.version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;

            doCheck = false;
            meta = {
              description = package.description;
              homepage = package.repository;
              license = lib.licenses.mit;
              mainProgram = package.name;
              maintainers = [
                {
                  name = "TheYoxy";
                  email = "floryansimar@gmail.com";
                }
              ];
            };
          };
        default = ymsp;
      };
    });
}
