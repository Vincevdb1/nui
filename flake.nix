{
  description = "Flake of NUI project";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };

  outputs =
    {
      self,
      nixpkgs,
    }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
      manifest = (pkgs.lib.importTOML ./Cargo.toml).package;
    in
    {
      packages.${system} = rec {
        # Plain build; expects nix-search-cli and nxv to be on PATH at runtime.
        nui-unwrapped = pkgs.rustPlatform.buildRustPackage {
          pname = manifest.name;
          version = manifest.version;
          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = with pkgs; [
            pkg-config
            nix-search-cli
          ];

          buildInputs = with pkgs; [
          ];

          meta.mainProgram = "nui";
        };

        # Wrapped build with nix-search-cli and nxv on PATH.
        nui = pkgs.symlinkJoin {
          name = "${manifest.name}-${manifest.version}";
          paths = [ nui-unwrapped ];
          nativeBuildInputs = [ pkgs.makeWrapper ];
          postBuild = ''
            wrapProgram $out/bin/nui \
              --prefix PATH : ${
                pkgs.lib.makeBinPath [
                  pkgs.nix-search-cli
                  pkgs.nxv
                ]
              }
          '';
          meta.mainProgram = "nui";
        };

        default = nui;
      };

      devShells.${system}.default = pkgs.mkShell {
        name = "nui";

        inputsFrom = [ self.packages.${system}.nui-unwrapped ];

        buildInputs = with pkgs; [
          cargo-generate
          rust-analyzer
          clippy
          rustfmt
          nix-search-cli
          nxv
        ];

        shellHook = "";
      };
    };
}
