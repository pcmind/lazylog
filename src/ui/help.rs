use ratatui::{
    layout::{Constraint, Direction, Layout, Alignment},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
    Frame,
};
use crate::commands::KeyRegistry;

pub fn render_help_popup(f: &mut Frame, registry: &KeyRegistry, selected_index: usize) {
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

    let block = Block::default()
        .title(" Keybindings Help ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let items: Vec<ListItem> = registry.all_bindings().iter().enumerate().map(|(i, b)| {
        let key_str = b.display_key();
        let desc = b.description;
        
        let mut style = Style::default().fg(Color::White);
        if i == selected_index {
            style = style.bg(Color::Rgb(60, 60, 60)).add_modifier(Modifier::BOLD);
        }

        let content = Line::from(vec![
            Span::styled(format!("{:>10} ", key_str), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("│ "),
            Span::styled(format!("{:<30}", desc), style),
        ]);
        
        ListItem::new(content).style(style)
    }).collect();

    let mut list_state = ListState::default();
    list_state.select(Some(selected_index));

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(Color::Rgb(60, 60, 60)).add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");

    f.render_stateful_widget(list, popup_area, &mut list_state);
}
