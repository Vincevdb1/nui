{
  description = "A devShell made with flake-parts perSystem";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-25.11";
    flake-parts.url = "github:hercules-ci/flake-parts";
  };

  outputs =
    args@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inputs = args; } {
      systems = [
        "x86_64-linux"
        "aarch64-darwin"
      ];
      perSystem =
        { pkgs, ... }:
        {
          devShells.default = pkgs.mkShell {
            packages = [
              pkgs.git
              pkgs.jq
            ];
          };
        };
    };
}
