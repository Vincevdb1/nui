mod common;

use common::fixture;
use nui::backend::{Error, nxv};

#[test]
fn search_keeps_the_newest_entry_per_attribute() {
    let packages = nxv::parse_search(fixture("nxv_search.json").as_bytes()).unwrap();
    let attrs: Vec<_> = packages.iter().map(|p| p.attribute.as_str()).collect();
    assert_eq!(attrs, ["ripgrep", "ripgrep-all", "steam"]);

    let rg = &packages[0];
    assert_eq!(rg.version.as_deref(), Some("15.2.0"));
    assert_eq!(rg.hash.as_deref(), Some("c8007378"));
    assert_eq!(rg.description.as_deref(), Some("Fast grep"));
    assert_eq!(
        rg.platforms.as_deref(),
        Some(&["x86_64-linux".to_string(), "aarch64-darwin".to_string()][..])
    );
    assert_eq!(
        rg.licenses.as_deref(),
        Some(&["Unlicense".to_string(), "MIT".to_string()][..])
    );

    let rga = &packages[1];
    assert_eq!(rga.description, None);
    assert_eq!(rga.platforms, None, "missing fields stay empty");
}

#[test]
fn search_accepts_legacy_stringified_arrays() {
    let packages = nxv::parse_search(fixture("nxv_legacy.json").as_bytes()).unwrap();
    assert_eq!(
        packages[0].platforms.as_deref(),
        Some(&["x86_64-linux".to_string()][..])
    );
    assert_eq!(
        packages[0].licenses.as_deref(),
        Some(&["unfreeRedistributable".to_string()][..])
    );
}

#[test]
fn versions_trim_dates_and_detect_unfree_licenses() {
    let versions = nxv::parse_versions(fixture("nxv_versions.json").as_bytes()).unwrap();
    let summary: Vec<_> = versions
        .iter()
        .map(|v| (v.version.as_str(), v.date.as_str()))
        .collect();
    assert_eq!(
        summary,
        [
            ("15.2.0", "2026-09-21"),
            ("14.1.1", "2026-07-15"),
            ("13.0.0", "2023-01-10"),
        ]
    );
    assert!(versions.iter().all(|v| !v.is_unfree && !v.is_system));
    assert_eq!(versions[0].hash, "c8007378a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6");

    let legacy = nxv::parse_versions(fixture("nxv_legacy.json").as_bytes()).unwrap();
    assert!(legacy[0].is_unfree);
}

#[test]
fn malformed_output_is_a_parse_error() {
    assert!(matches!(
        nxv::parse_search(b"not json"),
        Err(Error::Parse("nxv", _))
    ));
    assert!(matches!(
        nxv::parse_versions(b"{}"),
        Err(Error::Parse("nxv", _))
    ));
    assert!(nxv::parse_search(b"[]").unwrap().is_empty());
}

#[test]
fn unfree_detection_is_case_insensitive() {
    assert!(nxv::is_unfree(&["Unfree redistributable"]));
    assert!(nxv::is_unfree(&["MIT", "unfreeRedistributable"]));
    assert!(!nxv::is_unfree(&["MIT", "GPL-3.0"]));
    assert!(!nxv::is_unfree::<&str>(&[]));
}

#[test]
fn strip_ansi_removes_escape_sequences() {
    assert_eq!(nxv::strip_ansi("\x1b[32mgreen\x1b[0m text"), "green text");
    assert_eq!(nxv::strip_ansi("\x1b[2K\x1b[1Gline"), "line");
    assert_eq!(nxv::strip_ansi("plain"), "plain");
}

#[test]
fn compact_progress_redraws_the_bar_in_ten_cells() {
    assert_eq!(
        nxv::compact_progress("Downloading [=====-----] 50%"),
        "[█████     ] 50%"
    );
    assert_eq!(
        nxv::compact_progress("[####################] done"),
        "[██████████] done"
    );
    assert_eq!(
        nxv::compact_progress("[          ] 0/10"),
        "[          ] 0/10"
    );
    assert_eq!(nxv::compact_progress("no bar here"), "no bar here");
    assert_eq!(nxv::compact_progress("cut [===="), "[====");
}
