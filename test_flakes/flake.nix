{
  description = "This is a test flake";

  inputs = {
    nixpkgs-stable.url = "github:nixos/nixpkgs/nixos-25.11";
  };

  outputs =
    {
      self,
      nixpkgs,
      nixpkgs-stable,
    }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
    in
    {
      devShells.${system} = {
        default = pkgs.mkShell {
          name = "python shell";

          buildInputs = with pkgs; [
            python312Full
            isort
          ];

          shellHook = "";
        };

        another = pkgs.mkShell {
          name = "another shell";

          buildInputs = with pkgs; [
            hello
            testssl
          ];

          shellHook = "";
        };
      };
    };
}
