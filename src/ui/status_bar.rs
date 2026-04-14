use crate::state::action::{Mode, BindingContextWrapper};
use crate::input::handler::CommandHandler;
use crate::ui::render::RenderContext;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

pub struct StatusBar;

impl StatusBar {
    pub fn render(cmd: &CommandHandler, ctx: &RenderContext) -> Paragraph<'static> {
        let mut prefix_str = String::new();
        if !cmd.pending_keys.is_empty() {
            prefix_str = format!(" WAIT [{}] ", cmd.pending_keys.iter().map(|k| k.display_key()).collect::<Vec<_>>().join(" "));
        }

        let mode_str = match cmd.mode {
            Mode::Normal => if ctx.is_filter_pane { " NORMAL (FILTER) " } else { " NORMAL " },
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
        if ctx.is_following {
            spans.push(Span::styled(
                " FOLLOW ",
                Style::default().bg(Color::Green).fg(Color::Black).add_modifier(Modifier::BOLD),
            ));
        }

        // Active search query badge (in Normal mode)
        if matches!(cmd.mode, Mode::Normal) {
            if let Some(q) = &cmd.search_query {
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

        let metrics_str = if ctx.is_filter_pane {
            format!(" | Match {}/{} | Line {} ", ctx.pane_selected_line + 1, ctx.pane_total_lines, ctx.current_line)
        } else if ctx.total_lines > 0 {
            format!(" | Line {}/{} | Size: {} B ", ctx.pane_selected_line + 1, ctx.pane_total_lines, ctx.file_size)
        } else {
            String::new()
        };

        if !metrics_str.is_empty() {
            spans.push(Span::styled(
                metrics_str,
                Style::default().bg(Color::White).fg(Color::Black).add_modifier(Modifier::BOLD),
            ));
        }

        let mut hints_spans = Vec::new();
        hints_spans.push(Span::raw(" "));

        match cmd.mode {
            Mode::Normal | Mode::Visual { .. } => {
                let bind_ctx = if ctx.is_filter_pane { BindingContextWrapper::FilterPane } else { BindingContextWrapper::MainPane };
                let bind_ctx = if matches!(cmd.mode, Mode::Visual { .. }) { BindingContextWrapper::VisualMode } else { bind_ctx };

                let search_active = cmd.search_query.is_some();
                let bindings = cmd.registry.visible_bindings(bind_ctx, &cmd.pending_keys, search_active);
                let mut displayed_keys = std::collections::HashSet::new();
                for b in bindings {
                    if cmd.pending_keys.len() < b.sequence.len() {
                        let next_key = &b.sequence[cmd.pending_keys.len()];
                        let key_str = next_key.display_key();
                        if displayed_keys.insert(key_str.clone()) {
                            let is_prefix = b.sequence.len() > cmd.pending_keys.len() + 1;
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
                hints_spans.push(Span::styled(format!("> {}", cmd.filter_input), Style::default().fg(Color::Yellow)));
            },
            Mode::Search => {
                hints_spans.push(Span::styled("[Esc] ", Style::default().fg(Color::Cyan)));
                hints_spans.push(Span::styled("Cancel  ", Style::default().fg(Color::White)));
                hints_spans.push(Span::styled("[Enter] ", Style::default().fg(Color::Cyan)));
                hints_spans.push(Span::styled("Confirm  ", Style::default().fg(Color::White)));
                hints_spans.push(Span::styled(format!("/ {}", cmd.search_input), Style::default().fg(Color::Yellow)));
            },
            Mode::Help => {
                hints_spans.push(Span::styled("[Esc] ", Style::default().fg(Color::Cyan)));
                hints_spans.push(Span::styled("Close  ", Style::default().fg(Color::White)));
                hints_spans.push(Span::styled("[↑/↓] ", Style::default().fg(Color::Cyan)));
                hints_spans.push(Span::styled("Navigate  ", Style::default().fg(Color::White)));
                hints_spans.push(Span::styled("[Enter] ", Style::default().fg(Color::Cyan)));
                hints_spans.push(Span::styled("Execute  ", Style::default().fg(Color::White)));
            }
        }



        spans.extend(hints_spans);

        let empty_space = Span::styled(
            " ".repeat(200),
            Style::default().bg(Color::DarkGray),
        );
        spans.push(empty_space);

        Paragraph::new(Line::from(spans))
            .style(Style::default().bg(Color::DarkGray).fg(Color::White))
    }
}
