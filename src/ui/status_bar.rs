use crate::commands::{Mode, KeyRegistry, BindingContextWrapper, KeyCombo};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

pub struct StatusBar;

impl StatusBar {
    pub fn render(
        registry: &KeyRegistry,
        mode: &Mode, 
        current_line: usize, 
        total_lines: usize, 
        file_size: u64, 
        filter_input: &str, 
        is_filter_pane: bool,
        pending_keys: &[KeyCombo],
        is_following: bool,
        search_input: &str,
        search_query: &Option<String>,
    ) -> Paragraph<'static> {
        
        let mut prefix_str = String::new();
        if !pending_keys.is_empty() {
            prefix_str = format!(" WAIT [{}] ", pending_keys.iter().map(|k| k.display_key()).collect::<Vec<_>>().join(" "));
        }

        let mode_str = match mode {
            Mode::Normal => if is_filter_pane { " NORMAL (FILTER) " } else { " NORMAL " },
            Mode::Filter => " INPUT ",
            Mode::Search => " SEARCH ",
            Mode::Help => " HELP ",
            Mode::Visual { .. } => " VISUAL ",
        };

        let mut spans = vec![
            Span::styled(
                mode_str,
                Style::default().bg(Color::Blue).fg(Color::Black).add_modifier(Modifier::BOLD),
            )
        ];

        // Follow mode badge
        if is_following {
            spans.push(Span::styled(
                " FOLLOW ",
                Style::default().bg(Color::Green).fg(Color::Black).add_modifier(Modifier::BOLD),
            ));
        }

        // Active search query badge (in Normal mode)
        if matches!(mode, Mode::Normal) {
            if let Some(q) = search_query {
                spans.push(Span::styled(
                    format!(" /{} ", q),
                    Style::default().bg(Color::Yellow).fg(Color::Black).add_modifier(Modifier::BOLD),
                ));
            }
        }

        if !prefix_str.is_empty() {
            spans.push(Span::styled(
                prefix_str,
                Style::default().bg(Color::LightRed).fg(Color::Black).add_modifier(Modifier::BOLD),
            ));
        }

        let mut hints_spans = Vec::new();
        hints_spans.push(Span::raw(" "));

        match mode {
            Mode::Normal | Mode::Visual { .. } => {
                let ctx = if is_filter_pane { BindingContextWrapper::FilterPane } else { BindingContextWrapper::MainPane };
                let ctx = if matches!(mode, Mode::Visual { .. }) { BindingContextWrapper::VisualMode } else { ctx };
                
                let bindings = registry.visible_bindings(ctx, pending_keys);
                let mut displayed_keys = std::collections::HashSet::new();
                for b in bindings {
                    if pending_keys.len() < b.sequence.len() {
                        let next_key = &b.sequence[pending_keys.len()];
                        let key_str = next_key.display_key();
                        if displayed_keys.insert(key_str.clone()) {
                            let is_prefix = b.sequence.len() > pending_keys.len() + 1;
                            let label = if is_prefix { "Prefix" } else { b.label };
                            hints_spans.push(Span::styled(format!("[{}] ", key_str), Style::default().fg(Color::Cyan)));
                            hints_spans.push(Span::styled(format!("{}  ", label), Style::default().fg(Color::White)));
                        }
                    }
                }
            },
            Mode::Filter => {
                hints_spans.push(Span::styled("[Esc] ", Style::default().fg(Color::Cyan)));
                hints_spans.push(Span::styled("Cancel  ", Style::default().fg(Color::White)));
                hints_spans.push(Span::styled("[Enter] ", Style::default().fg(Color::Cyan)));
                hints_spans.push(Span::styled("Confirm  ", Style::default().fg(Color::White)));
                hints_spans.push(Span::styled(format!("> {}", filter_input), Style::default().fg(Color::Yellow)));
            },
            Mode::Search => {
                hints_spans.push(Span::styled("[Esc] ", Style::default().fg(Color::Cyan)));
                hints_spans.push(Span::styled("Cancel  ", Style::default().fg(Color::White)));
                hints_spans.push(Span::styled("[Enter] ", Style::default().fg(Color::Cyan)));
                hints_spans.push(Span::styled("Confirm  ", Style::default().fg(Color::White)));
                hints_spans.push(Span::styled(format!("/ {}", search_input), Style::default().fg(Color::Yellow)));
            },
            Mode::Help => {
                hints_spans.push(Span::styled("[Esc/q/?] ", Style::default().fg(Color::Cyan)));
                hints_spans.push(Span::styled("Close  ", Style::default().fg(Color::White)));
                hints_spans.push(Span::styled("[j/k] ", Style::default().fg(Color::Cyan)));
                hints_spans.push(Span::styled("Navigate  ", Style::default().fg(Color::White)));
                hints_spans.push(Span::styled("[Enter] ", Style::default().fg(Color::Cyan)));
                hints_spans.push(Span::styled("Execute  ", Style::default().fg(Color::White)));
            }
        }

        let metrics_str = if total_lines > 0 {
            format!("| Line {}/{} | Size: {} B ", current_line, total_lines, file_size)
        } else {
            String::new()
        };

        hints_spans.push(Span::styled(metrics_str, Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)));

        spans.extend(hints_spans);
        
        let empty_space = Span::styled(
            " ".repeat(200), // Quick hack to fill bg
            Style::default().bg(Color::DarkGray),
        );
        spans.push(empty_space);

        Paragraph::new(Line::from(spans))
            .style(Style::default().bg(Color::DarkGray).fg(Color::White))
    }
}
