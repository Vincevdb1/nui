use nui::ui::widgets::{Selectable, centered_rect, channel_color, truncate, wrap};
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::{ListState, TableState};

#[test]
fn wrap_moves_with_wrap_around() {
    let cases = [
        // (current, len, forward, expected)
        (None, 3, true, Some(0)),
        (None, 3, false, Some(0)),
        (Some(0), 3, true, Some(1)),
        (Some(2), 3, true, Some(0)),
        (Some(0), 3, false, Some(2)),
        (Some(2), 3, false, Some(1)),
        (Some(7), 3, true, Some(0)),
        (Some(7), 3, false, Some(2)),
        (Some(0), 0, true, None),
        (None, 0, false, None),
    ];
    for (current, len, forward, expected) in cases {
        assert_eq!(
            wrap(current, len, forward),
            expected,
            "wrap({current:?}, {len}, {forward})"
        );
    }
}

#[test]
fn step_updates_list_and_table_selection() {
    let mut list = ListState::default();
    list.step(2, true);
    assert_eq!(list.selected(), Some(0));
    list.step(2, true);
    assert_eq!(list.selected(), Some(1));
    list.step(2, true);
    assert_eq!(list.selected(), Some(0));

    let mut table = TableState::default();
    table.select(Some(0));
    table.step(4, false);
    assert_eq!(table.selected(), Some(3));
    table.step(0, false);
    assert_eq!(table.selected(), None, "an empty list clears the selection");
}

#[test]
fn truncate_adds_an_ellipsis_only_when_needed() {
    assert_eq!(truncate("short", 10), "short");
    assert_eq!(truncate("exactly10!", 10), "exactly10!");
    assert_eq!(truncate("a long description", 10), "a long ...");
    assert_eq!(
        truncate("ééééééé", 6),
        "ééé...",
        "counts characters, not bytes"
    );
}

#[test]
fn channel_colors_are_stable() {
    assert_eq!(channel_color("system"), Color::Cyan);
    assert_eq!(channel_color("nixos-25.11"), channel_color("nixos-25.11"));
    let distinct: std::collections::HashSet<_> = ["nxv", "nixos-25.11", "nixos-unstable", "a", "b"]
        .iter()
        .map(|c| format!("{:?}", channel_color(c)))
        .collect();
    assert!(distinct.len() > 1, "channels don't all share one colour");
}

#[test]
fn centered_rect_stays_inside_and_centred() {
    let area = Rect::new(0, 0, 100, 50);
    let popup = centered_rect(60, 40, area);
    assert_eq!((popup.width, popup.height), (60, 20));
    assert_eq!((popup.x, popup.y), (20, 15));

    let offset = Rect::new(10, 5, 80, 40);
    let inner = centered_rect(50, 50, offset);
    assert!(inner.x >= offset.x && inner.right() <= offset.right());
    assert!(inner.y >= offset.y && inner.bottom() <= offset.bottom());
}
