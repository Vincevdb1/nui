use nui::backend::nxv::VersionInfo;
use nui::backend::search::ChannelVersion;
use nui::features::search::versions::{VersionItem, VersionPicker, compare_versions};
use nui::flake::Input;
use std::cmp::Ordering;

#[test]
fn versions_compare_numerically_per_component() {
    let cases = [
        ("1.10", "1.9", Ordering::Greater),
        ("2.0", "10.0", Ordering::Less),
        ("1.2.3", "1.2.3", Ordering::Equal),
        ("1.2", "1.2.1", Ordering::Less),
        ("1.0-rc1", "1.0-rc2", Ordering::Less),
        ("1.0b", "1.0a", Ordering::Greater),
        ("2024-01-02", "2024-01-10", Ordering::Less),
    ];
    for (a, b, expected) in cases {
        assert_eq!(compare_versions(a, b), expected, "{a} vs {b}");
    }
}

fn info(version: &str, hash: &str) -> VersionInfo {
    VersionInfo {
        version: version.to_string(),
        hash: hash.to_string(),
        date: "2026-01-01".to_string(),
        is_unfree: false,
        is_system: false,
    }
}

fn input(name: &str, branch: &str, rev: Option<&str>) -> Input {
    Input {
        name: name.to_string(),
        url: format!("github:nixos/nixpkgs/{branch}"),
        branch: Some(branch.to_string()),
        rev: rev.map(str::to_string),
    }
}

fn cv(channel: &str, version: &str, locked: Option<&str>) -> ChannelVersion {
    ChannelVersion {
        version: version.to_string(),
        channel: channel.to_string(),
        locked_version: locked.map(str::to_string),
    }
}

fn picker() -> VersionPicker {
    VersionPicker::new("ripgrep".into(), "Fast grep".into(), false, None)
}

fn listed(p: &VersionPicker) -> Vec<String> {
    p.items
        .iter()
        .map(|item| match item {
            VersionItem::Input { input, version, .. } => format!("{input}:{version}"),
            VersionItem::Version(v) => v.version.clone(),
        })
        .collect()
}

#[test]
fn a_new_picker_is_loading_and_empty() {
    let p = picker();
    assert!(p.loading);
    assert!(p.items.is_empty());
    assert!(p.selected().is_none());
}

#[test]
fn fetched_history_is_sorted_newest_first_and_selected() {
    let mut p = picker();
    p.set_fetched(
        vec![
            info("13.0.0", "c"),
            info("15.2.0", "a"),
            info("14.1.1", "b"),
        ],
        &[],
        &[],
    );
    assert!(!p.loading);
    assert_eq!(listed(&p), ["15.2.0", "14.1.1", "13.0.0"]);
    assert_eq!(p.selected().map(VersionItem::version), Some("15.2.0"));
}

#[test]
fn inputs_carrying_the_package_are_listed_with_their_locked_version() {
    let inputs = [
        input("nixpkgs", "nixos-25.11", Some("abc")),
        input("unstable", "nixos-unstable", None),
        input("old", "nixos-23.11", None),
    ];
    let channels = [
        cv("nixos-25.11", "15.1.0", Some("15.0.0")),
        cv("nixos-unstable", "15.2.0", None),
    ];
    let mut p = picker();
    p.set_fetched(vec![info("14.1.1", "b")], &inputs, &channels);
    assert_eq!(
        listed(&p),
        ["unstable:15.2.0", "nixpkgs:15.0.0", "14.1.1"],
        "the locked version wins and inputs without the package are left out"
    );
    match &p.items[1] {
        VersionItem::Input { rev, channel, .. } => {
            assert_eq!(rev, "abc");
            assert_eq!(channel, "nixos-25.11");
        }
        VersionItem::Version(_) => panic!("expected an input item"),
    }
    match &p.items[0] {
        VersionItem::Input { rev, .. } => assert_eq!(rev, "-------", "unlocked input"),
        VersionItem::Version(_) => panic!("expected an input item"),
    }
}

#[test]
fn rebuild_refreshes_input_versions_without_refetching() {
    let inputs = [input("nixpkgs", "nixos-25.11", Some("abc"))];
    let mut p = picker();
    p.set_fetched(
        vec![info("1.0", "x")],
        &inputs,
        &[cv("nixos-25.11", "2.0", None)],
    );
    assert_eq!(listed(&p), ["nixpkgs:2.0", "1.0"]);

    p.rebuild(&inputs, &[cv("nixos-25.11", "2.0", Some("1.5"))]);
    assert_eq!(listed(&p), ["nixpkgs:1.5", "1.0"]);
}

#[test]
fn stepping_wraps_through_the_items() {
    let mut p = picker();
    p.set_fetched(vec![info("2", "a"), info("1", "b")], &[], &[]);
    p.step(true);
    assert_eq!(p.selected().map(VersionItem::version), Some("1"));
    p.step(true);
    assert_eq!(p.selected().map(VersionItem::version), Some("2"));
    p.step(false);
    assert_eq!(p.selected().map(VersionItem::version), Some("1"));
}
