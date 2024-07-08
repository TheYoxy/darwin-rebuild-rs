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
    self,
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
      p = builtins.map (f: f.override {inherit rustPlatform;}) (with pkgs; [cargo-watch]);
    in {
      formatter = pkgs.alejandra;

      devShells = {
        default = with pkgs;
          mkShell {
            buildInputs =
              [
                libiconv
                pkg-config
                rustToolchain
                nix-output-monitor
                nvd
              ]
              ++ p;
          };
      };

      packages = let
        inherit (pkgs) lib;
        inherit (lib.importTOML ./Cargo.toml) package;
        rev = self.shortRev or self.dirtyShortRev or "dirty";
        use-nom = true;
        runtimeDeps = [pkgs.nvd] ++ lib.optionals use-nom [pkgs.nix-output-monitor];
      in rec {
        darwin-rebuild =
          rustPlatform
          .buildRustPackage {
            pname = package.name;
            version = "${package.version}-${rev}";
            src = lib.fileset.toSource {
              root = ./.;
              fileset =
                lib.fileset.intersection
                (lib.fileset.fromSource (lib.sources.cleanSource ./.))
                (lib.fileset.unions [
                  ./src
                  ./Cargo.toml
                  ./Cargo.lock
                ]);
            };
            cargoLock.lockFile = ./Cargo.lock;

            strictDeps = true;

            nativeBuildInputs = with pkgs; [
              installShellFiles
              makeBinaryWrapper
            ];

            preFixup = ''
              mkdir completions

              $out/bin/${package.name} completions bash > completions/${package.name}.bash
              $out/bin/${package.name} completions zsh > completions/${package.name}.zsh
              $out/bin/${package.name} completions fish > completions/${package.name}.fish

              installShellCompletion completions/*
            '';

            postFixup = ''
              wrapProgram $out/bin/${package.name} \
                --prefix PATH : ${lib.makeBinPath runtimeDeps}
            '';

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
        default = darwin-rebuild;
      };
    });
}
