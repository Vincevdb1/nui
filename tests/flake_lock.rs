use nui::flake::Input;
use nui::flake::lock::{RevChange, rev_changes, update_summary};

fn input(name: &str, rev: Option<&str>) -> Input {
    Input {
        name: name.into(),
        rev: rev.map(String::from),
        ..Default::default()
    }
}

#[test]
fn only_changed_revisions_are_reported() {
    let before = [
        input("a", Some("1111111aaaa")),
        input("b", Some("2222222bbbb")),
    ];
    let after = [
        input("a", Some("1111111aaaa")),
        input("b", Some("3333333cccc")),
    ];
    assert_eq!(
        rev_changes(&before, &after),
        [RevChange {
            name: "b".into(),
            old: Some("2222222bbbb".into()),
            new: Some("3333333cccc".into())
        }]
    );
}

#[test]
fn an_input_without_a_lock_entry_has_no_old_revision() {
    let changes = rev_changes(&[input("a", None)], &[input("a", Some("4444444dddd"))]);
    assert_eq!(changes[0].old, None);
    assert_eq!(update_summary(&changes), "a: unlocked → 4444444");
}

#[test]
fn the_summary_uses_short_revisions_one_line_each() {
    let changes = [
        RevChange {
            name: "a".into(),
            old: Some("1111111aaaa".into()),
            new: Some("2222222bbbb".into()),
        },
        RevChange {
            name: "b".into(),
            old: Some("3333333cccc".into()),
            new: Some("4444444dddd".into()),
        },
    ];
    assert_eq!(
        update_summary(&changes),
        "a: 1111111 → 2222222\nb: 3333333 → 4444444"
    );
}

#[test]
fn no_changes_mean_up_to_date() {
    assert_eq!(update_summary(&[]), "All inputs are already up to date");
}
