mod common;

use common::{fixture, flake_src};
use nui::flake::{Output, OutputKind, parse};

fn summary(outputs: &[Output]) -> Vec<(OutputKind, &str, Option<&str>)> {
    outputs
        .iter()
        .map(|o| (o.kind, o.path.as_str(), o.name.as_deref()))
        .collect()
}

#[test]
fn devshells_under_a_let_bound_system_are_resolved() {
    let outputs = parse::outputs(&flake_src("devshells"));
    assert_eq!(
        summary(&outputs),
        [
            (
                OutputKind::DevShell,
                "x86_64-linux.default",
                Some("python shell")
            ),
            (
                OutputKind::DevShell,
                "x86_64-linux.another",
                Some("another shell")
            ),
        ]
    );
    let content = outputs[0].content.as_deref().unwrap();
    assert!(content.starts_with("pkgs.mkShell"), "{content}");
    assert!(content.contains("python312"));
}

#[test]
fn dotted_paths_with_a_literal_system() {
    let outputs = parse::outputs(&flake_src("flat-paths"));
    assert_eq!(
        summary(&outputs),
        [
            (
                OutputKind::DevShell,
                "x86_64-linux.default",
                Some("flat default")
            ),
            (
                OutputKind::DevShell,
                "x86_64-linux.native",
                Some("native inputs")
            ),
            (OutputKind::DevShell, "x86_64-linux.empty", None),
        ]
    );
}

#[test]
fn system_configurations_sit_one_level_deep() {
    let nixos = parse::outputs(&flake_src("nixos"));
    assert_eq!(summary(&nixos), [(OutputKind::Nixos, "test-host", None)]);

    let home = parse::outputs(&flake_src("home-manager"));
    assert_eq!(home.len(), 1);
    assert_eq!(home[0].kind, OutputKind::Home);
    assert!(home[0].content.as_deref().unwrap().contains("ripgrep"));

    let darwin = parse::outputs(&flake_src("darwin"));
    assert_eq!(darwin.len(), 1);
    assert_eq!(darwin[0].kind, OutputKind::Darwin);
}

#[test]
fn mixed_outputs_include_packages_from_source() {
    let kinds: Vec<_> = parse::outputs(&flake_src("mixed"))
        .iter()
        .map(|o| (o.kind, o.path.clone()))
        .collect();
    assert_eq!(
        kinds,
        [
            (OutputKind::Packages, "x86_64-linux.default".to_string()),
            (OutputKind::Nixos, "server".to_string()),
            (OutputKind::DevShell, "x86_64-linux.tools".to_string()),
            (OutputKind::DevShell, "x86_64-linux.default".to_string()),
        ]
    );
}

#[test]
fn source_without_outputs_gives_nothing() {
    assert!(parse::outputs("{ description = \"x\"; }").is_empty());
    assert!(parse::outputs("").is_empty());
    assert!(parse::outputs("{ outputs = 42; }").is_empty());
}

fn from_show(file: &str) -> Vec<Output> {
    let json: serde_json::Value = serde_json::from_str(&fixture(file)).unwrap();
    parse::outputs_from_flake_show(&json, &flake_src("mixed"))
}

#[test]
fn flake_show_v2_lists_editable_outputs_default_shell_first() {
    let outputs = from_show("flake_show_v2.json");
    assert_eq!(
        summary(&outputs),
        [
            (
                OutputKind::DevShell,
                "x86_64-linux.default",
                Some("mixed default")
            ),
            (OutputKind::DevShell, "x86_64-linux.tools", Some("tools")),
            (OutputKind::Nixos, "server", None),
        ],
        "`packages` is not shown and the default devShell comes first"
    );
    assert!(
        outputs[0]
            .content
            .as_deref()
            .unwrap()
            .contains("pkgs.hello"),
        "source text from flake.nix is attached"
    );
}

#[test]
fn flake_show_v1_and_v2_agree() {
    assert_eq!(
        from_show("flake_show_v1.json"),
        from_show("flake_show_v2.json")
    );
}

#[test]
fn outputs_missing_from_source_have_no_content() {
    let json: serde_json::Value = serde_json::from_str(&fixture("flake_show_v2.json")).unwrap();
    let outputs = parse::outputs_from_flake_show(&json, "{ }");
    assert_eq!(outputs.len(), 3);
    assert!(
        outputs
            .iter()
            .all(|o| o.content.is_none() && o.name.is_none())
    );
}

#[test]
fn empty_flake_show_gives_nothing() {
    let empty = serde_json::json!({});
    assert!(parse::outputs_from_flake_show(&empty, "").is_empty());
    let v2 = serde_json::json!({ "version": 2, "inventory": {} });
    assert!(parse::outputs_from_flake_show(&v2, "").is_empty());
}
