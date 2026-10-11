{
  description = "Dotted devShell paths with a literal system and every package list attribute";

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
      devShells.x86_64-linux.default = pkgs.mkShell {
        name = "flat default";
        packages = [
          pkgs.git
          pkgs.jq
        ];
      };

      devShells.x86_64-linux.native = pkgs.mkShell {
        name = "native inputs";
        nativeBuildInputs = [ pkgs.pkg-config ];
        buildInputs = [ pkgs.openssl ];
      };

      devShells.x86_64-linux.empty = pkgs.mkShell { };
    };
}
