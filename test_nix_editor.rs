use nix_editor;

fn main() {
    let content = r#"
{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };
  outputs = { self, nixpkgs, ... }: {
    nixosConfigurations.my-host = nixpkgs.lib.nixosSystem { };
    devShells.x86_64-linux.default = nixpkgs.legacyPackages.x86_64-linux.mkShell { };
    homeConfigurations."user@host" = nixpkgs.lib.homeManagerConfiguration { };
  };
}
"#;
    if let Ok(collection) = nix_editor::parse::get_collection(content.to_string()) {
        for (key, val) in collection {
            println!("{} = {}", key, val);
        }
    } else {
        println!("Failed to parse collection");
    }
}
