use nui::app::effect::update_jobs;
use nui::flake::Input;

fn input(name: &str, url: &str) -> Input {
    Input {
        name: name.into(),
        url: url.into(),
        branch: None,
        rev: None,
    }
}

#[test]
fn update_jobs_resolve_channels_and_drop_duplicates() {
    let inputs = [input("stable", "github:NixOS/nixpkgs/nixos-25.11")];
    let packages = vec![
        ("hello".to_string(), None),
        ("hello".to_string(), None),
        ("ripgrep".to_string(), Some("stable".to_string())),
        ("fd".to_string(), Some("missing".to_string())),
    ];
    assert_eq!(
        update_jobs(packages, &inputs),
        [
            ("fd".to_string(), "nixos-unstable".to_string()),
            ("hello".to_string(), "nixos-unstable".to_string()),
            ("ripgrep".to_string(), "nixos-25.11".to_string()),
        ]
    );
}
