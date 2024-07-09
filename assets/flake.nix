{
  description = "Nix for macOS configuration";

  ##################################################################################################################
  #
  # Want to know Nix in details? Looking for a beginner-friendly tutorial?
  # Check out https://github.com/ryan4yin/nixos-and-flakes-book !
  #
  ##################################################################################################################

  # This is the standard format for flake.nix. `inputs` are the dependencies of the flake,
  # Each item in `inputs` will be passed as a parameter to the `outputs` function after being pulled and built.
  inputs = {
    # nixpkgs-darwin.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    nixpkgs-darwin.url = "github:nixos/nixpkgs/nixpkgs-24.05-darwin";
    flake-utils.url = "github:numtide/flake-utils";
    darwin = {
      url = "github:lnl7/nix-darwin";
      inputs.nixpkgs.follows = "nixpkgs-darwin";
    };
  };

  # The `outputs` function will return all the build results of the flake.
  # A flake can have many use cases and different types of outputs,
  # parameters in `outputs` are defined in `inputs` and can be referenced by their names.
  # However, `self` is an exception, this special parameter points to the `outputs` itself (self-reference)
  # The `@` syntax here is used to alias the attribute set of the inputs's parameter, making it convenient to use inside the function.
  outputs = inputs @ {
    self,
    nixpkgs,
    darwin,
    flake-utils,
    ...
  }: let
    # TODO replace with your own username, system and hostname
    username = "darwin";
    hostname = "darwin-rebuild-rs";

    specialArgs =
      inputs
      // {
        inherit username hostname;
      };
  in
    flake-utils.lib.eachDefaultSystem (system: {
      # nix code formatter
      formatter.${system} = nixpkgs.legacyPackages.${system}.alejandra;
    })
    // {
      darwinConfigurations."${hostname}" = darwin.lib.darwinSystem {
        inherit specialArgs;
        system = "x86_64-linux";
        modules = [];
      };
    };
}
