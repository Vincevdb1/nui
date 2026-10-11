{
  description = "A test flake with a nix-darwin configuration";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-25.11-darwin";
    nix-darwin = {
      url = "github:nix-darwin/nix-darwin/nix-darwin-25.11";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { self, nixpkgs, nix-darwin }:
    {
      darwinConfigurations."test-mac" = nix-darwin.lib.darwinSystem {
        modules = [
          (
            { pkgs, ... }:
            {
              nixpkgs.hostPlatform = "aarch64-darwin";
              system.stateVersion = 6;
              environment.systemPackages = with pkgs; [
                coreutils
                wget
              ];
            }
          )
        ];
      };
    };
}
