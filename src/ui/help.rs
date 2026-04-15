use crate::input::keys::KeyRegistry;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

pub fn render_help_popup(
    f: &mut Frame,
    registry: &KeyRegistry,
    selected_index: usize,
    filter: &str,
) {
    let size = f.size();

    // Create a 80x80% area centered
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(10),
            Constraint::Percentage(80),
            Constraint::Percentage(10),
        ])
        .split(size);

    let popup_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(10),
            Constraint::Percentage(80),
            Constraint::Percentage(10),
        ])
        .split(popup_layout[1])[1];

    f.render_widget(Clear, popup_area);

    // Split popup area into search bar + list
    let inner_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Filter input area
            Constraint::Min(1),    // Bindings list
        ])
        .split(popup_area);

    // Render filter input
    let filter_display = if filter.is_empty() {
        " Type to filter... ".to_string()
    } else {
        format!(" > {} ", filter)
    };
    let filter_style = if filter.is_empty() {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::Yellow)
    };
    let filter_block = Block::default()
        .title(" Keybindings Help ")
        .title_alignment(Alignment::Center)
        .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
        .border_style(Style::default().fg(Color::Yellow));
    f.render_widget(
        Paragraph::new(Span::styled(filter_display, filter_style)).block(filter_block),
        inner_layout[0],
    );

    // Filter bindings
    let filter_lower = filter.to_lowercase();
    let filtered_bindings: Vec<_> = registry
        .all_bindings()
        .iter()
        .enumerate()
        .filter(|(_, b)| {
            if filter.is_empty() {
                return true;
            }
            b.description.to_lowercase().contains(&filter_lower)
                || b.label.to_lowercase().contains(&filter_lower)
                || b.display_key().to_lowercase().contains(&filter_lower)
        })
        .collect();

    let items: Vec<ListItem> = filtered_bindings
        .iter()
        .enumerate()
        .map(|(filtered_idx, (_, b))| {
            let key_str = b.display_key();
            let desc = b.description;

            let mut style = Style::default().fg(Color::White);
            if filtered_idx == selected_index {
                style = style
                    .bg(Color::Rgb(60, 60, 60))
                    .add_modifier(Modifier::BOLD);
            }

            let content = Line::from(vec![
                Span::styled(
                    format!("{:>10} ", key_str),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("│ "),
                Span::styled(format!("{:<30}", desc), style),
            ]);

            ListItem::new(content).style(style)
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(selected_index));

    let list_block = Block::default()
        .borders(Borders::BOTTOM | Borders::LEFT | Borders::RIGHT)
        .border_style(Style::default().fg(Color::Yellow));

    let list = List::new(items)
        .block(list_block)
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(60, 60, 60))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    f.render_stateful_widget(list, inner_layout[1], &mut list_state);
}
