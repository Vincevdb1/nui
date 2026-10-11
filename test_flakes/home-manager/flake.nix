{
  description = "A test flake with a standalone Home Manager configuration";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-25.11";
    home-manager = {
      url = "github:nix-community/home-manager/release-25.11";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { nixpkgs, home-manager, ... }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in
    {
      homeConfigurations."tester" = home-manager.lib.homeManagerConfiguration {
        inherit pkgs;
        modules = [
          {
            home.username = "tester";
            home.homeDirectory = "/home/tester";
            home.stateVersion = "25.11";
            home.packages = with pkgs; [
              ripgrep
              fd
              jq
            ];
          }
        ];
      };
    };
}
