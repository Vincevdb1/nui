{
  description = "A devShell with a package pinned to its own nixpkgs input";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-25.11";
    nixpkgs-hello.url = "github:nixos/nixpkgs/a4bf06618f0b5ee50f14ed8f0da77d34ecc19160";
  };

  outputs =
    inputs@{
      self,
      nixpkgs,
      nixpkgs-hello,
    }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        name = "pinned shell";
        packages = [
          pkgs.git
          inputs.nixpkgs-hello.legacyPackages.${system}.hello
        ];
      };
    };
}
