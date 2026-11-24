mod common;

use common::{fixture, flake_path};
use nui::backend::nix;
use nui::flake::{Output, OutputKind};

fn output(kind: OutputKind, path: &str) -> Output {
    Output {
        kind,
        path: path.to_string(),
        name: None,
        content: None,
        edit_error: None,
    }
}

#[test]
fn eval_targets_point_at_each_output_package_list() {
    let (target, extract) =
        nix::eval_target(&output(OutputKind::DevShell, "x86_64-linux.default")).unwrap();
    assert_eq!(target, r#"devShells."x86_64-linux"."default""#);
    assert!(extract.contains("buildInputs") && extract.contains("nativeBuildInputs"));

    let (target, _) = nix::eval_target(&output(OutputKind::Nixos, "test-host")).unwrap();
    assert_eq!(
        target,
        r#"nixosConfigurations."test-host".config.environment.systemPackages"#
    );

    let (target, _) = nix::eval_target(&output(OutputKind::Darwin, "mac")).unwrap();
    assert_eq!(
        target,
        r#"darwinConfigurations."mac".config.environment.systemPackages"#
    );

    let (target, extract) = nix::eval_target(&output(OutputKind::Home, "tester")).unwrap();
    assert_eq!(target, r#"homeConfigurations."tester""#);
    assert!(extract.contains("home.packages"));
}

#[test]
fn package_outputs_are_not_evaluated() {
    assert!(nix::eval_target(&output(OutputKind::Packages, "x86_64-linux.default")).is_none());
    assert!(nix::eval_target(&output(OutputKind::LegacyPackages, "x86_64-linux.hello")).is_none());
}

#[test]
fn parses_nix_env_metadata() {
    let meta = nix::parse_metadata(fixture("nix_env_metadata.json").as_bytes()).unwrap();
    assert_eq!(meta.version, "2.12.2");
    assert_eq!(
        meta.description,
        "Program that produces a familiar, friendly greeting"
    );
}

#[test]
fn metadata_tolerates_missing_fields() {
    let meta = nix::parse_metadata(br#"{ "nixpkgs.x": {} }"#).unwrap();
    assert_eq!(meta.version, "Unknown");
    assert_eq!(meta.description, "");
    assert!(nix::parse_metadata(b"{}").is_none());
    assert!(nix::parse_metadata(b"not json").is_none());
}

#[test]
fn parses_nix_search_on_a_flake() {
    let packages = nix::parse_flake_search(fixture("nix_search_flake.json").as_bytes()).unwrap();
    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0].attribute, "packages.x86_64-linux.default");
    assert_eq!(packages[0].version.as_deref(), Some("2.12.2"));
    assert!(nix::parse_flake_search(b"[]").is_err());
}

// The tests below evaluate the fixture flakes with a real `nix` and need network access.

#[test]
#[ignore = "needs nix and network"]
fn flake_show_reads_devshells() {
    let outputs = nix::flake_show(&flake_path("devshells")).unwrap();
    let paths: Vec<_> = outputs.iter().map(|o| o.path.as_str()).collect();
    assert_eq!(paths, ["x86_64-linux.default", "x86_64-linux.another"]);
    assert_eq!(outputs[0].name.as_deref(), Some("python shell"));
}

#[test]
#[ignore = "needs nix and network"]
fn flake_show_reads_system_configurations() {
    let nixos = nix::flake_show(&flake_path("nixos")).unwrap();
    assert_eq!(nixos.len(), 1);
    assert_eq!(
        (nixos[0].kind, nixos[0].path.as_str()),
        (OutputKind::Nixos, "test-host")
    );

    let home = nix::flake_show(&flake_path("home-manager")).unwrap();
    assert_eq!(
        (home[0].kind, home[0].path.as_str()),
        (OutputKind::Home, "tester")
    );
}

#[test]
#[ignore = "needs nix and network"]
fn flake_show_matches_the_captured_fixture() {
    let live = nix::flake_show(&flake_path("mixed")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&fixture("flake_show_v2.json")).unwrap();
    let source = common::flake_src("mixed");
    let mut expected = nui::flake::parse::outputs_from_flake_show(&json, &source);
    nix::attach_shell_sources(&nui::flake::locate::Fs, &flake_path("mixed"), &mut expected);
    assert_eq!(live, expected);
}

#[test]
#[ignore = "needs nix and network"]
fn output_packages_evaluates_a_devshell() {
    let dir = common::root().join("test_flakes/devshells");
    let flake_ref = nui::flake::files::flake_ref(&dir);
    let mut packages = nix::output_packages(
        &flake_ref,
        &output(OutputKind::DevShell, "x86_64-linux.another"),
    )
    .unwrap();
    packages.sort_by(|a, b| a.name.cmp(&b.name));
    let names: Vec<_> = packages.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["hello", "testssl.sh"]);
    assert!(
        packages
            .iter()
            .all(|p| !p.version.is_empty() && !p.is_unfree)
    );
}

#[test]
#[ignore = "needs nix and network"]
fn output_packages_evaluates_a_nixos_configuration() {
    let flake_ref = nui::flake::files::flake_ref(&common::root().join("test_flakes/nixos"));
    let packages =
        nix::output_packages(&flake_ref, &output(OutputKind::Nixos, "test-host")).unwrap();
    let names: Vec<_> = packages.iter().map(|p| p.name.as_str()).collect();
    for expected in ["curl", "git", "vim"] {
        assert!(
            names.contains(&expected),
            "{expected} missing from {names:?}"
        );
    }
}

#[test]
fn versions_expr_looks_up_each_safe_attribute_in_the_given_set() {
    let attrs = ["hello", "python3Packages.requests", "bad\"; attr"].map(String::from);
    let expr = nix::versions_expr("import <nixpkgs> {}", &attrs);
    assert!(expr.starts_with("let pkgs = import <nixpkgs> {};"));
    assert!(expr.contains(r#""hello" = getV "hello";"#));
    assert!(expr.contains(r#""python3Packages.requests" = getV "python3Packages.requests";"#));
    assert!(
        !expr.contains("bad"),
        "attributes with other characters are left out"
    );
    assert!(expr.ends_with('}'));
}

#[test]
fn versions_at_rejects_revisions_that_are_not_plain_hashes() {
    let attrs = ["hello".to_string()];
    assert!(nix::versions_at("abc\"; evil", &attrs, None).is_empty());
}

#[test]
fn update_command_names_the_inputs_and_the_flake() {
    let cmd = nix::update_command("./sub", &["nixpkgs".into(), "home-manager".into()]);
    let args: Vec<_> = cmd
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert_eq!(cmd.get_program(), "nix");
    assert_eq!(
        args,
        [
            "flake",
            "update",
            "nixpkgs",
            "home-manager",
            "--flake",
            "./sub"
        ]
    );
}

#[test]
fn update_command_without_inputs_updates_all() {
    let cmd = nix::update_command(".", &[]);
    let args: Vec<_> = cmd
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert_eq!(args, ["flake", "update", "--flake", "."]);
}

#[test]
fn stderr_tail_keeps_the_last_lines() {
    assert_eq!(nix::stderr_tail(b"one\ntwo\nthree\n", 2), "two\nthree");
}

#[test]
#[ignore] // needs nix and network
fn update_inputs_locks_a_copied_flake() {
    let dir = common::copy_flake("devshells");
    let flake_ref = dir.path().to_string_lossy().into_owned();
    nix::update_inputs(&flake_ref, &["nixpkgs-stable".into()]).unwrap();
    assert!(dir.path().join("flake.lock").exists());
}
