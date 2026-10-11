mod common;

use common::flake_src;
use nui::flake::parse;

fn shell_content(fixture: &str, path: &str) -> String {
    parse::outputs(&flake_src(fixture))
        .into_iter()
        .find(|o| o.path == path)
        .and_then(|o| o.content)
        .unwrap_or_else(|| panic!("{fixture} has no output {path}"))
}

#[test]
fn reads_items_of_a_with_pkgs_list() {
    let content = shell_content("devshells", "x86_64-linux.default");
    assert_eq!(parse::package_attributes(&content), ["python312", "isort"]);
}

#[test]
fn reads_every_package_list_attribute() {
    let content = shell_content("flat-paths", "x86_64-linux.native");
    assert_eq!(
        parse::package_attributes(&content),
        ["pkgs.pkg-config", "pkgs.openssl"]
    );
}

#[test]
fn keeps_input_references_verbatim() {
    let content = shell_content("pinned", "x86_64-linux.default");
    assert_eq!(
        parse::package_attributes(&content),
        [
            "pkgs.git",
            "inputs.nixpkgs-hello.legacyPackages.${system}.hello"
        ]
    );
}

#[test]
fn a_bare_list_is_read_when_there_is_no_package_list_attribute() {
    assert_eq!(
        parse::package_attributes("[ pkgs.git pkgs.vim ]"),
        ["pkgs.git", "pkgs.vim"]
    );
}

#[test]
fn a_shell_without_lists_has_no_packages() {
    let content = shell_content("flat-paths", "x86_64-linux.empty");
    assert!(parse::package_attributes(&content).is_empty());
}

#[test]
fn ref_source_splits_input_and_attribute() {
    let cases = [
        ("hello", (None, "hello")),
        ("pkgs.hello", (Some("pkgs"), "hello")),
        (
            "inputs.foo.legacyPackages.${system}.hello",
            (Some("foo"), "hello"),
        ),
        ("foo.packages.x86_64-linux.bar", (Some("foo"), "bar")),
        ("self.packages.${system}.tool", (Some("self"), "tool")),
        (
            "pkgs.python3Packages.numpy",
            (Some("pkgs"), "python3Packages.numpy"),
        ),
    ];
    for (attr, expected) in cases {
        assert_eq!(parse::ref_source(attr), expected, "attr {attr}");
    }
}
