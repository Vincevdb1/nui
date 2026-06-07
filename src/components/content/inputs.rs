use crate::nix::Input;
use ratatui::{prelude::*, widgets::*};

pub struct InputsProps<'a> {
    pub inputs: &'a [Input],
    pub input_table_state: &'a mut TableState,
}

pub fn render(props: &mut InputsProps, frame: &mut Frame, area: Rect) {
    let mut rows = Vec::new();

    rows.push(Row::new(vec![
        Cell::from("─".repeat(100)),
        Cell::from("┼"),
        Cell::from("─".repeat(100)),
    ]));

    for input in props.inputs {
        rows.push(Row::new(vec![
            Cell::from(format!(" {}", input.name)),
            Cell::from("│"),
            Cell::from(format!(" {}", input.url)),
        ]));
    }

    let widths = [
        Constraint::Percentage(20),
        Constraint::Length(1),
        Constraint::Min(10),
    ];

    let table = Table::new(rows, widths)
        .header(
            Row::new(vec![
                Cell::from(" Name"),
                Cell::from("│"),
                Cell::from(" URL"),
            ])
            .style(Style::default().add_modifier(Modifier::BOLD)),
        )
        .column_spacing(0)
        .row_highlight_style(Style::default().bg(Color::Cyan).fg(Color::Black));

    frame.render_stateful_widget(table, area, props.input_table_state);
}
