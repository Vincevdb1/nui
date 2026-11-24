# Test flakes

Real flakes used by the test suite in `tests/`. Most tests read the `flake.nix` and
`flake.lock` files as text (parsing and editing). Tests marked `#[ignore]` also evaluate
them with `nix` and need network access: `cargo test -- --ignored`.

| Flake | What it exercises |
|---|---|
| `devshells/` | Two devShells under `devShells.${system} = { ... }`, `with pkgs; [ ... ]` lists, an input locked by branch and one resolved from the registry |
| `nixos/` | A `nixosConfigurations` output with `environment.systemPackages` |
| `home-manager/` | A standalone `homeConfigurations` output with `home.packages`, an input using `follows` |
| `darwin/` | A `darwinConfigurations` output (evaluating it needs a Darwin builder) |
| `flat-paths/` | Dotted `devShells.x86_64-linux.<name>` paths with a literal system; `packages`, `buildInputs` and `nativeBuildInputs` lists; an empty `mkShell { }` |
| `pinned/` | A package pinned to its own nixpkgs input, an `inputs@{ ... }` outputs pattern |
| `minimal/` | No `inputs` block and a single-line `{ self }:` pattern; not evaluable on purpose (no lock, no nixpkgs) |
| `mixed/` | devShells, a NixOS configuration and `packages` in one flake |
| `flake-utils/` | flake-utils `eachDefaultSystem (system: ...)` with `devShells.default`; text only |
| `for-all-systems/` | `devShells = forAllSystems (pkgs: ...)` with the shell bound in `let`; text only |
| `flake-parts/` | flake-parts `perSystem = { pkgs, ... }:` and an `args@{ ... }` outputs pattern; text only |
| `imported/` | `devShells.${system}.default = import ./shell.nix { inherit pkgs; }`; text only |
| `concat/` | `packages = with pkgs; [ ... ] ++ lib.optionals ...`; text only |
| `unknown/` | A shell made by a helper that nui cannot follow, for the managed file fallback; text only |

Captured tool output (`nix flake show`, `nix-search`, `nxv`, ...) lives in `tests/fixtures/`.
