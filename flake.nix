{
  description = "Flake of NUI project";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        name = "nui";

        buildInputs = with pkgs; [
          rustc
          cargo
          cargo-generate
          rust-analyzer
          clippy
          rustfmt
          pkg-config
        ];

        shellHook = "";
      };
    };
}
