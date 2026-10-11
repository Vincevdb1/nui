{
  description = "devShells, a NixOS configuration and packages in one flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-25.11";
  };

  outputs =
    { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in
    {
      packages.${system}.default = pkgs.hello;

      nixosConfigurations.server = nixpkgs.lib.nixosSystem {
        inherit system;
        modules = [
          {
            boot.loader.grub.enable = false;
            fileSystems."/" = {
              device = "/dev/null";
            };
            system.stateVersion = "25.11";
            environment.systemPackages = [ pkgs.htop ];
          }
        ];
      };

      devShells.${system} = {
        tools = pkgs.mkShell {
          name = "tools";
          buildInputs = with pkgs; [ curl ];
        };
        default = pkgs.mkShell {
          name = "mixed default";
          packages = [ pkgs.hello ];
        };
      };
    };
}
