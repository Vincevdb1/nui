{
  description = "devShells made with forAllSystems and a shell bound in let";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-25.11";
  };

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-darwin"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      devShells = forAllSystems (
        pkgs:
        let
          shell = pkgs.mkShell {
            packages = with pkgs; [
              git
              jq
            ];
          };
        in
        {
          default = shell;
        }
      );
    };
}
