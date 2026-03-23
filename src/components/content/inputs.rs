use ratatui::{prelude::*, widgets::*};
use crate::nix::Input;

pub fn render(inputs: &[Input], frame: &mut Frame, area: Rect) {
    let header = Row::new(vec!["Name", "URL"])
        .style(Style::default().add_modifier(Modifier::BOLD))
        .bottom_margin(1);

    let rows: Vec<Row> = inputs
        .iter()
        .map(|input| {
            Row::new(vec![
                Cell::from(input.name.clone()),
                Cell::from(input.url.clone()),
            ])
        })
        .collect();

    let widths = [Constraint::Percentage(20), Constraint::Percentage(80)];

    let table = Table::new(rows, widths)
        .header(header)
        .column_spacing(2)
        .style(Style::default());

    frame.render_widget(table, area);
}
