{
  description = "A test flake with a NixOS configuration";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-25.11";
  };

  outputs = { self, nixpkgs }: {
    nixosConfigurations."test-host" = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        ({ pkgs, ... }: {
          boot.loader.grub.enable = false;
          fileSystems."/" = { device = "/dev/null"; };
          
          environment.systemPackages = with pkgs; [
            curl
            git
            vim
          ];
        })
      ];
    };
  };
}
