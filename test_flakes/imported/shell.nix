{ pkgs }:
pkgs.mkShell {
  packages = [
    pkgs.git
    pkgs.jq
  ];
}
