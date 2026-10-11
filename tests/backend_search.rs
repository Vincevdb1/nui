use nui::backend::search::{
    ChannelVersion, Request, Scope, SearchPackage, SearchResult, merge_flake, merge_shell, sorted,
};
use nui::flake::Input;
use std::collections::HashMap;

fn pkg(attr: &str, version: &str) -> SearchPackage {
    SearchPackage {
        attribute: attr.to_string(),
        version: Some(version.to_string()),
        description: None,
        platforms: None,
        licenses: None,
        hash: None,
    }
}

fn input(name: &str, url: &str) -> Input {
    Input {
        name: name.to_string(),
        url: url.to_string(),
        ..Default::default()
    }
}

fn channels(result: &SearchResult) -> Vec<(&str, &str)> {
    result
        .versions
        .iter()
        .map(|v| (v.channel.as_str(), v.version.as_str()))
        .collect()
}

#[test]
fn shell_requests_search_nxv_and_the_system_channel() {
    let request = Request::new("rg".into(), true, &[], Some("25.11.123.abc"));
    assert!(
        matches!(&request.scope, Scope::Shell { system_channel } if system_channel == "25.11.123.abc")
    );
    assert_eq!(request.channels(), ["25.11.123.abc", "nxv"]);

    let unknown = Request::new("rg".into(), true, &[], None);
    assert_eq!(unknown.channels(), ["nixos-unstable", "nxv"]);
}

#[test]
fn flake_requests_search_each_nixpkgs_channel_once() {
    let inputs = [
        input("nixpkgs", "github:nixos/nixpkgs/nixos-25.11"),
        input("stable", "github:nixos/nixpkgs/nixos-25.11"),
        input("unstable", "github:nixos/nixpkgs/nixpkgs-unstable"),
        input("hm", "github:nix-community/home-manager"),
    ];
    let request = Request::new("rg".into(), false, &inputs, Some("ignored"));
    assert_eq!(request.channels(), ["nixos-25.11", "nixpkgs-unstable"]);
    match &request.scope {
        Scope::Flake { inputs, .. } => {
            let names: Vec<_> = inputs.iter().map(|i| i.name.as_str()).collect();
            assert_eq!(names, ["hm"], "non-nixpkgs inputs are searched as flakes");
        }
        Scope::Shell { .. } => panic!("expected a flake scope"),
    }
}

#[test]
fn flake_requests_without_nixpkgs_use_unstable() {
    let request = Request::new("rg".into(), false, &[], None);
    assert_eq!(request.channels(), ["nixos-unstable"]);
}

#[test]
fn shell_merge_keeps_one_version_per_channel() {
    let mut results = HashMap::new();
    merge_shell(&mut results, "nxv", pkg("ripgrep", "15.2.0"));
    merge_shell(&mut results, "nxv", pkg("ripgrep", "14.1.1"));
    merge_shell(&mut results, "25.11", pkg("ripgrep", "15.1.0"));
    merge_shell(&mut results, "unstable", pkg("ripgrep", "15.2.0"));

    let rg = &results["ripgrep"];
    assert_eq!(
        channels(rg),
        [
            ("nxv", "15.2.0"),
            ("25.11", "15.1.0"),
            ("nixos-unstable", "15.2.0")
        ],
        "the first hit per channel wins; bare channel names count as unstable"
    );
}

#[test]
fn shell_merge_fills_in_missing_details() {
    let mut results = HashMap::new();
    merge_shell(&mut results, "nxv", pkg("steam", "1.0"));
    merge_shell(
        &mut results,
        "25.11",
        SearchPackage {
            description: Some("Game store".into()),
            platforms: Some(vec!["x86_64-linux".into()]),
            licenses: Some(vec!["Unfree redistributable".into()]),
            hash: Some("abc".into()),
            ..pkg("steam", "1.0")
        },
    );
    let steam = &results["steam"];
    assert_eq!(steam.description, "Game store");
    assert_eq!(steam.platforms, ["x86_64-linux"]);
    assert!(steam.is_unfree);
    assert_eq!(steam.hash.as_deref(), Some("abc"));
    assert_eq!(steam.source_input, None);
}

#[test]
fn flake_merge_keys_input_packages_by_input() {
    let mut results = HashMap::new();
    merge_flake(&mut results, "nixos-25.11", false, pkg("hello", "2.12"));
    merge_flake(
        &mut results,
        "nixpkgs-unstable",
        false,
        pkg("hello", "2.13"),
    );
    merge_flake(&mut results, "tools", true, pkg("hello", "0.1"));
    merge_flake(
        &mut results,
        "self",
        true,
        pkg("packages.x86_64-linux.default", "2.12.2"),
    );

    let mut keys: Vec<_> = results.keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(
        keys,
        ["hello", "self.packages.x86_64-linux.default", "tools.hello"]
    );
    assert_eq!(
        channels(&results["hello"]),
        [("nixos-25.11", "2.12"), ("nixpkgs-unstable", "2.13")]
    );
    assert_eq!(
        results["tools.hello"].source_input.as_deref(),
        Some("tools")
    );
    assert_eq!(results["hello"].source_input, None);
}

#[test]
fn flake_merge_prefers_unstable_and_longer_descriptions() {
    let with_desc = |desc: &str| SearchPackage {
        description: Some(desc.into()),
        ..pkg("hello", "1")
    };
    let mut results = HashMap::new();
    merge_flake(
        &mut results,
        "nixos-25.11",
        false,
        with_desc("A greeting program, stable"),
    );
    merge_flake(&mut results, "nixos-unstable", false, with_desc("Greeter"));
    assert_eq!(
        results["hello"].description, "Greeter",
        "first unstable hit wins"
    );

    merge_flake(
        &mut results,
        "nixos-24.05",
        false,
        with_desc("A much longer description"),
    );
    assert_eq!(results["hello"].description, "A much longer description");
}

#[test]
fn flake_merge_unions_platforms() {
    let with_platforms = |p: &[&str]| SearchPackage {
        platforms: Some(p.iter().map(|s| s.to_string()).collect()),
        ..pkg("hello", "1")
    };
    let mut results = HashMap::new();
    merge_flake(
        &mut results,
        "a",
        false,
        with_platforms(&["x86_64-linux", "aarch64-linux"]),
    );
    merge_flake(
        &mut results,
        "b",
        false,
        with_platforms(&["aarch64-darwin", "x86_64-linux"]),
    );
    assert_eq!(
        results["hello"].platforms,
        ["aarch64-darwin", "aarch64-linux", "x86_64-linux"]
    );
}

fn result(name: &str, description: &str) -> SearchResult {
    let mut results = HashMap::new();
    merge_flake(
        &mut results,
        "nixos-unstable",
        false,
        SearchPackage {
            description: Some(description.into()),
            ..pkg(name, "1")
        },
    );
    results.remove(name).unwrap()
}

#[test]
fn sorting_ranks_exact_then_prefix_then_substring_then_description() {
    let results: HashMap<_, _> = [
        result("zgrep", "mentions rg in passing"),
        result("ripgrep", "fast grep"),
        result("rg", "exact"),
        result("rga", "prefix match"),
        result("unrelated", "nothing"),
        result("sorg", "substring"),
    ]
    .into_iter()
    .map(|r| (r.name.clone(), r))
    .collect();

    let names: Vec<_> = sorted(results, "RG").into_iter().map(|r| r.name).collect();
    assert_eq!(
        names,
        ["rg", "rga", "sorg", "zgrep", "ripgrep", "unrelated"]
    );
}

fn with_versions(versions: &[(&str, &str)]) -> SearchResult {
    SearchResult {
        versions: versions
            .iter()
            .map(|(channel, version)| ChannelVersion {
                version: version.to_string(),
                channel: channel.to_string(),
                locked_version: None,
            })
            .collect(),
        ..result("x", "")
    }
}

#[test]
fn shell_version_prefers_system_then_the_system_channel() {
    let r = with_versions(&[("nxv", "3"), ("25.11", "2"), ("system", "1")]);
    assert_eq!(r.shell_version(Some("25.11.1")), "1");

    let r = with_versions(&[("nxv", "3"), ("25.11", "2")]);
    assert_eq!(r.shell_version(Some("25.11.1")), "2");
    assert_eq!(r.shell_version(None), "3");

    assert_eq!(with_versions(&[]).shell_version(None), "Unknown");
}
