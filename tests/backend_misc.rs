use nui::backend::{github, system, templates};

fn names(branches: &[github::Branch]) -> Vec<&str> {
    branches.iter().map(|b| b.name.as_str()).collect()
}

#[test]
fn branches_list_rolling_then_newest_releases() {
    let ranked = github::rank_branches(
        [
            "nixos-24.05",
            "release-25.11",
            "master",
            "nixos-25.11",
            "staging",
            "nixpkgs-unstable",
            "nixos-25.11-small",
        ]
        .map(String::from)
        .to_vec(),
    );
    assert_eq!(
        names(&ranked),
        [
            "nixpkgs-unstable",
            "master",
            "nixos-25.11-small",
            "nixos-25.11",
            "nixos-24.05"
        ]
    );
    assert!(
        ranked
            .iter()
            .all(|b| b.url == format!("github:nixos/nixpkgs/{}", b.name))
    );
}

#[test]
fn tool_versions_are_read_from_version_output() {
    let cases = [
        ("nxv", "nxv 0.7.3\n", Some("0.7.3")),
        ("nix-search", "NIX-SEARCH 0.3 (rev abc)", Some("0.3")),
        ("nix-search", "unknown-name 1.4", Some("1.4")),
        ("nxv", "", None),
    ];
    for (cmd, output, expected) in cases {
        assert_eq!(
            system::parse_tool_version(cmd, output).as_deref(),
            expected,
            "{cmd}: {output:?}"
        );
    }
}

#[test]
fn template_packages_come_from_every_list() {
    let content = r#"{
  outputs = { self, nixpkgs }: {
    devShells.x86_64-linux.default = pkgs.mkShell {
      packages = [ pkgs.git pkgs.jq ];
      buildInputs = [ openssl "quoted" pkgs.git ];
      nativeBuildInputs = [ pkg-config self.packages.x86_64-linux.tool ];
    };
  };
}"#;
    assert_eq!(
        templates::packages(content),
        ["git", "jq", "openssl", "quoted", "pkg-config"],
        "deduplicated, without `pkgs.` and without references into the flake itself"
    );
}

#[test]
fn template_packages_ignore_with_and_interpolation() {
    let content = "packages = [ with pkgs; ripgrep ${extra} ];";
    assert_eq!(templates::packages(content), ["ripgrep"]);
    assert!(templates::packages("{ }").is_empty());
    assert!(templates::packages("packages = [ unterminated").is_empty());
}

#[test]
#[ignore = "needs network access to api.github.com"]
fn fetches_nixpkgs_branches_from_github() {
    let branches = github::nixpkgs_branches().unwrap();
    assert_eq!(
        branches.first().map(|b| b.name.as_str()),
        Some("nixpkgs-unstable")
    );
    assert!(branches.iter().any(|b| b.name.starts_with("nixos-")));
}

#[test]
fn the_probe_reads_version_path_and_rev() {
    let json = br#"{"version":"25.11pre-git","path":"/nix/store/abc123-source"}"#;
    let info = system::SystemInfo::from_probe(json, Some("0123456789abcdef".into()));
    assert_eq!(info.nixpkgs_version.as_deref(), Some("25.11pre-git"));
    assert_eq!(
        info.nixpkgs_path.as_deref(),
        Some("/nix/store/abc123-source")
    );
    assert_eq!(info.nixpkgs_rev.as_deref(), Some("0123456789abcdef"));
}

#[test]
fn without_a_nixos_rev_the_rev_comes_from_the_store_path() {
    let path = format!("/nix/store/{}-source", "x".repeat(32));
    let json = format!(r#"{{"version":"25.11","path":"{}"}}"#, path);
    let info = system::SystemInfo::from_probe(json.as_bytes(), Some("short".into()));
    assert_eq!(
        info.nixpkgs_rev,
        Some("x".repeat(32)),
        "a short NixOS rev is ignored"
    );
}

#[test]
fn a_failed_probe_leaves_everything_unknown() {
    let info = system::SystemInfo::from_probe(b"", None);
    assert!(info.nixpkgs_version.is_none());
    assert!(info.nixpkgs_path.is_none());
    assert!(info.nixpkgs_rev.is_none());
}
