mod common;

use nui::backend::nix_search;

#[test]
fn channels_map_to_nix_search_releases() {
    let cases = [
        ("nixos-25.11", "25.11"),
        ("nixpkgs-24.05-darwin", "24.05"),
        ("release-23.11", "23.11"),
        ("25.11", "25.11"),
        ("nixos-unstable", "unstable"),
        ("nixpkgs-unstable", "unstable"),
        ("master", "unstable"),
        ("nixos-1.2", "unstable"),
        // A locked revision is not a release.
        ("15c6719d8c604779cf59e03c245ea61d3d7ab69b", "unstable"),
    ];
    for (channel, expected) in cases {
        assert_eq!(
            nix_search::cli_channel(channel),
            expected,
            "channel {channel}"
        );
    }
}

#[test]
fn parses_one_package_per_line_skipping_noise() {
    let packages = nix_search::parse_output(common::fixture("nix_search.jsonl").as_bytes());
    let attrs: Vec<_> = packages.iter().map(|p| p.attribute.as_str()).collect();
    assert_eq!(attrs, ["ripgrep", "ripgrep-all"]);

    let rg = &packages[0];
    assert_eq!(rg.version.as_deref(), Some("15.1.0"));
    assert_eq!(
        rg.licenses.as_deref(),
        Some(&["The Unlicense".to_string(), "MIT License".to_string()][..])
    );
    assert!(
        rg.platforms
            .as_ref()
            .unwrap()
            .contains(&"x86_64-linux".to_string())
    );
    assert_eq!(rg.hash, None);
}

#[test]
fn empty_output_has_no_packages() {
    assert!(nix_search::parse_output(b"").is_empty());
    assert!(nix_search::parse_output(b"\n\n").is_empty());
}
