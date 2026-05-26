{
  description = "Flake of NUI project";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    nixpkgs-stable.url = "github:nixos/nixpkgs/nixos-25.11";
    nxv.url = "github:utensils/nxv";
  };

  outputs =
    {
      self,
      nixpkgs,
      nixpkgs-stable,
      nxv,
    }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
      manifest = (pkgs.lib.importTOML ./Cargo.toml).package;
    in
    {
      packages.${system}.default = pkgs.rustPlatform.buildRustPackage {
        pname = manifest.name;
        version = manifest.version;
        src = ./.;

        cargoLock = {
          lockFile = ./Cargo.lock;
        };

        nativeBuildInputs = with pkgs; [
          pkg-config
          makeWrapper
          nix-search-cli
        ];

        buildInputs = with pkgs; [
        ];

        postInstall = ''
          wrapProgram $out/bin/nui \
            --prefix PATH : ${
              pkgs.lib.makeBinPath [
                pkgs.nix-search-cli
                nxv.packages.${system}.default
              ]
            }
        '';
      };

      devShells.${system}.default = pkgs.mkShell {
        name = "nui";

        inputsFrom = [ self.packages.${system}.default ];

        buildInputs = with pkgs; [
          cargo-generate
          rust-analyzer
          clippy
          rustfmt
          nix-search-cli
          nxv.packages.${system}.default
        ];

        shellHook = "";
      };
    };
}
