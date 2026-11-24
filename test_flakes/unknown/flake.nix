{
  description = "A devShell made by a helper that nui cannot follow";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-25.11";
  };

  outputs =
    { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
      shells = builtins.mapAttrs (name: packages: pkgs.mkShell { inherit packages; });
    in
    {
      devShells.${system} = shells { default = [ pkgs.git ]; };
    };
}
