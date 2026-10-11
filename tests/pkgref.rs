use nui::backend::system::SystemInfo;
use nui::pkgref::{PackageRef, Source};

const REV: &str = "15c6719d8c604779cf59e03c245ea61d3d7ab69b";

fn system(path: Option<&str>, version: Option<&str>) -> SystemInfo {
    SystemInfo {
        nixpkgs_version: version.map(str::to_string),
        nixpkgs_path: path.map(str::to_string),
        nixpkgs_rev: Some(REV.to_string()),
    }
}

#[test]
fn system_packages_prefer_the_local_nixpkgs_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_str().unwrap();
    let r = PackageRef::new(Source::System(REV.into()), "hello", None);
    assert_eq!(
        r.installable(&system(Some(path), Some("25.11.1"))),
        format!("path:{path}#hello")
    );
}

#[test]
fn system_packages_fall_back_to_the_release_channel() {
    let r = PackageRef::new(Source::System(REV.into()), "hello", None);
    let missing_path = Some("/definitely/not/here");
    let cases = [
        ("25.11.20260630.b6018f8", "nixpkgs/nixos-25.11#hello"),
        ("26.05pre123456.abcdef", "nixpkgs/nixpkgs-unstable#hello"),
        ("unstable", "nixpkgs/nixpkgs-unstable#hello"),
        ("garbage", "nixpkgs/nixpkgs-unstable#hello"),
    ];
    for (version, expected) in cases {
        assert_eq!(
            r.installable(&system(missing_path, Some(version))),
            expected,
            "version {version}"
        );
    }
}

#[test]
fn system_packages_fall_back_to_the_revision() {
    let r = PackageRef::new(Source::System(REV.into()), "hello", None);
    assert_eq!(
        r.installable(&SystemInfo::default()),
        format!("github:NixOS/nixpkgs/{REV}#hello")
    );
}

#[test]
fn nixpkgs_and_registry_installables() {
    let info = system(None, Some("25.11"));
    assert_eq!(
        PackageRef::new(Source::Nixpkgs("abc".into()), "jq", None).installable(&info),
        "github:NixOS/nixpkgs/abc#jq"
    );
    assert_eq!(
        PackageRef::new(Source::Registry, "jq", None).installable(&info),
        "nixpkgs#jq"
    );
}

#[test]
fn rev_is_known_except_for_the_registry() {
    assert_eq!(
        PackageRef::new(Source::System("a".into()), "x", None).rev(),
        Some("a")
    );
    assert_eq!(
        PackageRef::new(Source::Nixpkgs("b".into()), "x", None).rev(),
        Some("b")
    );
    assert_eq!(PackageRef::new(Source::Registry, "x", None).rev(), None);
}

#[test]
fn short_name_abbreviates_revisions() {
    let v = Some("1.0".to_string());
    assert_eq!(
        PackageRef::new(Source::System(REV.into()), "hello", v.clone()).short_name(),
        "15c6719#hello@1.0"
    );
    assert_eq!(
        PackageRef::new(Source::Nixpkgs(REV.into()), "hello", None).short_name(),
        "nixpkgs/15c6719#hello"
    );
    assert_eq!(
        PackageRef::new(Source::Registry, "hello", v).short_name(),
        "hello@1.0"
    );
}

#[test]
fn display_shows_the_full_reference() {
    let v = Some("2.12".to_string());
    assert_eq!(
        PackageRef::new(Source::System("abc".into()), "hello", v.clone()).to_string(),
        "system/abc#hello@2.12"
    );
    assert_eq!(
        PackageRef::new(Source::Nixpkgs("abc".into()), "hello", None).to_string(),
        "nixpkgs/abc#hello"
    );
    assert_eq!(
        PackageRef::new(Source::Registry, "hello", v).to_string(),
        "hello@2.12"
    );
}

#[test]
fn references_are_equal_by_source_attr_and_version() {
    let a = PackageRef::new(Source::Registry, "hello", None);
    assert_eq!(a, PackageRef::new(Source::Registry, "hello", None));
    assert_ne!(
        a,
        PackageRef::new(Source::Registry, "hello", Some("1".into()))
    );
    assert_ne!(
        a,
        PackageRef::new(Source::Nixpkgs("x".into()), "hello", None)
    );
}

fn flake_source() -> Source {
    Source::Flake {
        name: "nixvim".into(),
        url: "github:nix-community/nixvim".into(),
    }
}

#[test]
fn flake_packages_install_from_their_flake() {
    let r = PackageRef::new(flake_source(), "default", Some("0.1".into()));
    assert_eq!(
        r.installable(&system(None, Some("25.11.1"))),
        "github:nix-community/nixvim#default"
    );
    assert_eq!(r.rev(), None);
}

#[test]
fn flake_packages_are_named_after_their_flake() {
    let r = PackageRef::new(flake_source(), "default", Some("0.1".into()));
    assert_eq!(r.short_name(), "nixvim#default@0.1");
    assert_eq!(r.to_string(), "github:nix-community/nixvim#default@0.1");
}
