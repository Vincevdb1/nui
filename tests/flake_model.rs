use nui::flake::{Input, Output, OutputKind, Package, ShellTarget};

fn input(name: &str, url: &str) -> Input {
    Input {
        name: name.to_string(),
        url: url.to_string(),
        ..Default::default()
    }
}

#[test]
fn nixpkgs_inputs_are_recognised_by_url() {
    assert!(input("nixpkgs", "github:nixos/nixpkgs/nixos-25.11").is_nixpkgs());
    assert!(input("pkgs", "github:NixOS/nixpkgs").is_nixpkgs());
    assert!(!input("hm", "github:nix-community/home-manager").is_nixpkgs());
}

#[test]
fn upstream_channel_comes_from_the_url_branch() {
    let cases = [
        ("github:nixos/nixpkgs/nixos-25.11", "nixos-25.11"),
        ("github:nixos/nixpkgs/nixpkgs-unstable", "nixpkgs-unstable"),
        (
            "https://github.com/NixOS/nixpkgs/archive/nixos-24.05.tar.gz",
            "nixos-24.05",
        ),
        ("https://example.org/nixpkgs-23.11.tar.gz", "nixpkgs-23.11"),
        // No recognisable branch, or not nixpkgs at all, falls back to unstable.
        ("github:nixos/nixpkgs", "nixos-unstable"),
        (
            "github:nix-community/home-manager/nixos-25.11",
            "nixos-unstable",
        ),
    ];
    for (url, expected) in cases {
        assert_eq!(input("x", url).upstream_channel(), expected, "url {url}");
    }
}

#[test]
fn channel_prefers_branch_then_rev_then_upstream() {
    let mut i = input("nixpkgs", "github:nixos/nixpkgs/nixos-25.11");
    assert_eq!(i.channel(), "nixos-25.11");

    i.rev = Some("abc123".to_string());
    assert_eq!(i.channel(), "abc123");

    i.branch = Some("nixos-unstable".to_string());
    assert_eq!(i.channel(), "nixos-unstable");
}

const ALL_KINDS: [OutputKind; 6] = [
    OutputKind::DevShell,
    OutputKind::Darwin,
    OutputKind::Home,
    OutputKind::Nixos,
    OutputKind::Packages,
    OutputKind::LegacyPackages,
];

#[test]
fn output_kind_attr_round_trips() {
    for kind in ALL_KINDS {
        assert_eq!(OutputKind::from_attr(kind.attr()), Some(kind));
    }
    assert_eq!(OutputKind::from_attr("checks"), None);
    assert_eq!(OutputKind::from_attr(""), None);
}

#[test]
fn only_per_system_outputs_nest_under_a_system() {
    let per_system: Vec<_> = ALL_KINDS.iter().filter(|k| k.is_per_system()).collect();
    assert_eq!(
        per_system,
        [
            &OutputKind::DevShell,
            &OutputKind::Packages,
            &OutputKind::LegacyPackages
        ]
    );
}

#[test]
fn output_kind_labels() {
    assert_eq!(OutputKind::DevShell.label(), "SHELL");
    assert_eq!(OutputKind::Nixos.label(), "OS");
    assert_eq!(OutputKind::Home.label(), "HM");
    assert_eq!(OutputKind::Darwin.label(), "DRWN");
}

fn output(kind: OutputKind, path: &str) -> Output {
    Output {
        kind,
        path: path.to_string(),
        name: None,
        content: None,
    }
}

#[test]
fn only_devshells_are_edit_targets() {
    assert_eq!(
        output(OutputKind::DevShell, "x86_64-linux.default").shell_target(),
        Some(ShellTarget {
            system: "x86_64-linux".to_string(),
            name: "default".to_string()
        })
    );
    assert_eq!(output(OutputKind::Nixos, "host").shell_target(), None);
    assert_eq!(
        output(OutputKind::Home, "x86_64-linux.me").shell_target(),
        None
    );
    // A devShell path without a system cannot be targeted.
    assert_eq!(output(OutputKind::DevShell, "default").shell_target(), None);
}

fn package(name: &str, attr: Option<&str>) -> Package {
    Package {
        name: name.to_string(),
        description: String::new(),
        version: "1.0".to_string(),
        is_unfree: false,
        source: None,
        attr: attr.map(str::to_string),
    }
}

#[test]
fn qualified_name_prefers_the_written_attribute() {
    assert_eq!(
        package("git", Some("pkgs.git")).qualified_name(),
        "pkgs.git"
    );
    assert_eq!(package("git", None).qualified_name(), "git");
}

#[test]
fn packages_from_inputs_are_pinned() {
    let pinned = package(
        "hello",
        Some("inputs.nixpkgs-hello.legacyPackages.${system}.hello"),
    );
    assert!(pinned.is_pinned());
    assert!(!package("git", Some("pkgs.git")).is_pinned());
    assert!(!package("git", None).is_pinned());
}
