use nui::flake::edit::{self, EditError, Snapshot};
use std::path::PathBuf;

fn write(dir: &tempfile::TempDir, name: &str, content: &str) -> PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, content).unwrap();
    path
}

#[test]
fn restore_puts_back_the_old_content() {
    let dir = tempfile::tempdir().unwrap();
    let path = write(&dir, "flake.nix", "{ a = 1; }");
    let snapshot = edit::apply(&vec![(path.clone(), "{ a = 2; }".into())]).unwrap();
    assert_eq!(snapshot.paths(), [path.as_path()]);
    snapshot.restore().unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ a = 1; }");
}

#[test]
fn restore_deletes_a_file_the_edit_created() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nui/default.nix");
    let snapshot = edit::apply(&vec![(path.clone(), "[ ]".into())]).unwrap();
    snapshot.restore().unwrap();
    assert!(!path.exists());
}

#[test]
fn restore_refuses_a_file_changed_after_the_edit() {
    let dir = tempfile::tempdir().unwrap();
    let a = write(&dir, "a.nix", "{ }");
    let b = write(&dir, "b.nix", "{ }");
    let snapshot = edit::apply(&vec![
        (a.clone(), "{ x = 1; }".into()),
        (b.clone(), "{ y = 1; }".into()),
    ])
    .unwrap();
    std::fs::write(&b, "{ y = 2; }").unwrap();
    assert!(matches!(snapshot.restore(), Err(EditError::ChangedOnDisk(p)) if p == b));
    assert_eq!(std::fs::read_to_string(&a).unwrap(), "{ x = 1; }", "nothing is restored");
}

#[test]
fn apply_refuses_to_write_invalid_nix() {
    let dir = tempfile::tempdir().unwrap();
    let path = write(&dir, "flake.nix", "{ a = 1; }");
    let err = edit::apply(&vec![(path.clone(), "{ a = ; }".into())]).unwrap_err();
    assert!(matches!(err, EditError::InvalidNix(p) if p == path));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ a = 1; }");
}

#[test]
fn merge_keeps_the_first_before_and_the_last_after() {
    let dir = tempfile::tempdir().unwrap();
    let path = write(&dir, "flake.nix", "{ a = 1; }");
    let mut first = edit::apply(&vec![(path.clone(), "{ a = 2; }".into())]).unwrap();
    first.merge(edit::apply(&vec![(path.clone(), "{ a = 3; }".into())]).unwrap());
    first.restore().unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ a = 1; }");
}

#[test]
fn an_empty_snapshot_is_empty() {
    assert!(Snapshot::default().is_empty());
}
