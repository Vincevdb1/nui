# Nui

A modern Terminal User Interface (TUI) for the Nix package manager, designed to make managing Flakes and Nix shells more intuitive and efficient.

## Features

### Flake Management (Flake Mode)
- **Visualize your Flake**: View system packages, Home Manager packages, and `devShells` in a structured layout.
- **Input Management**: Add, remove, and update Flake inputs with intelligent branch suggestions directly from `nixpkgs`.
- **Package Inspection**: View detailed information about packages, including descriptions, versions, and license status (free/unfree).
- **Interactive Editing**: Modify your `flake.nix` through the UI.

### Interactive Shells (Shell Mode)
- **On-the-fly Environments**: Quickly spin up a Nix shell with specific packages without creating a permanent `flake.nix`.

### Advanced Package Search
- **Comprehensive Search**: Search for packages across `nixpkgs` (unstable/stable) and other channels.
- **Version Selection**: Choose specific versions of packages when adding them to your environment.
- **Detailed Metadata**: Inspect package licenses, supported platforms, and full descriptions before installation.

## Installation

### Using Nix
You can run `nui` directly using Nix:

```bash
nix run github:Vincevdb1/nui
```

To use it in your own Flake:

```nix
{
  inputs.nui.url = "github:Vincevdb1/nui";
  # ...
  outputs = { self, nixpkgs, nui }: {
    # Use nui.packages.${system}.default
  };
}
```

## Getting Started

### Navigation & Controls
- **Tab / 1-5**: Switch between different focus areas (Inputs, Packages, Configs, Logs).
- **m**: Toggle between **Flake** and **Shell** modes.
- **a**: Add a new package or input (depending on focus).
- **d**: Remove the selected package or input.
- **i**: View detailed information for the selected package.
- **s**: (Shell Mode) Start a Nix shell with the selected packages.
- **j / k** or **Arrows**: Navigate lists and tables.
- **q / Esc**: Quit or close popups.

## Requirements
- **Nix**: Must have the Nix package manager installed with Flakes enabled (`experimental-features = nix-command flakes`).

> **Note**: When installed via the provided Flake, **nh** and **nxv** are automatically bundled and available to `nui` at runtime.

- **nh** (Optional if not using Flake): [Yet another Nix Helper](https://github.com/viperML/nh) - Required for fast, interactive package searching.
- **nxv** (Optional if not using Flake): [Nix Version search](https://github.com/vincevdb1/nxv) - Required for fetching and selecting specific package versions.


## License

Copyright (c) Vincevdb1 <vincent.vandebosch@icloud.com>

This project is licensed under the MIT license ([LICENSE] or <http://opensource.org/licenses/MIT>)

[LICENSE]: ./LICENSE
