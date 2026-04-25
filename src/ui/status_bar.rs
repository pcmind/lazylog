use crate::input::handler::CommandHandler;
use crate::state::action::{ActionId, BindingContextWrapper, Mode};
use crate::ui::render::RenderContext;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

pub struct StatusBar;

pub fn compact_num(n: usize) -> String {
    if n < 1000 {
        n.to_string()
    } else if n < 1_000_000 {
        format!("{:.1}K", (n as f64) / 1000.0)
    } else {
        format!("{:.1}M", (n as f64) / 1_000_000.0)
    }
}

pub fn compact_size(n: u64) -> String {
    if n < 1024 {
        format!("{}B", n)
    } else if n < 1024 * 1024 {
        format!("{:.1}KB", (n as f64) / 1024.0)
    } else if n < 1024 * 1024 * 1024 {
        format!("{:.1}MB", (n as f64) / 1024.0 / 1024.0)
    } else {
        format!("{:.1}GB", (n as f64) / 1024.0 / 1024.0 / 1024.0)
    }
}

impl StatusBar {
    pub fn render(cmd: &CommandHandler, ctx: &RenderContext) -> Paragraph<'static> {
        let mut prefix_str = String::new();
        if !cmd.pending_keys.is_empty() {
            prefix_str = format!(
                " WAIT [{}] ",
                cmd.pending_keys
                    .iter()
                    .map(|k| k.display_key())
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }

        let mode_str = match cmd.mode {
            Mode::Normal => {
                if ctx.is_filter_pane {
                    " NORMAL (FILTER) "
                } else {
                    " NORMAL "
                }
            }
            Mode::Filter => " INPUT ",
            Mode::Search => " SEARCH ",
            Mode::Help => " HELP ",
            Mode::Visual { .. } => " VISUAL ",
            Mode::LineDetail => " DETAIL ",
        };

        let mut spans = vec![Span::styled(
            mode_str,
            Style::default()
                .bg(Color::Blue)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )];

        // Follow mode badge
        if ctx.is_following {
            spans.push(Span::styled(
                " FOLLOW ",
                Style::default()
                    .bg(Color::Green)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        // Boolean mode badge
        if ctx.is_boolean {
            spans.push(Span::styled(
                " BOOLEAN ",
                Style::default()
                    .bg(Color::Magenta)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        // Active search query badge (in Normal mode)
        if matches!(cmd.mode, Mode::Normal)
            && let Some(q) = &cmd.search_query
        {
            spans.push(Span::styled(
                format!(" /{} ", q),
                Style::default()
                    .bg(Color::Yellow)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        if !prefix_str.is_empty() {
            spans.push(Span::styled(
                prefix_str,
                Style::default()
                    .bg(Color::LightRed)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        let mut hints_spans = Vec::new();
        hints_spans.push(Span::raw(" "));

        match cmd.mode {
            Mode::Normal | Mode::Visual { .. } => {
                let bind_ctx = if ctx.is_filter_pane {
                    BindingContextWrapper::FilterPane
                } else {
                    BindingContextWrapper::MainPane
                };
                let bind_ctx = if matches!(cmd.mode, Mode::Visual { .. }) {
                    BindingContextWrapper::VisualMode
                } else {
                    bind_ctx
                };

                let search_active = cmd.search_query.is_some();
                let bindings =
                    cmd.registry
                        .visible_bindings(bind_ctx, &cmd.pending_keys, search_active);
                let mut displayed_keys = std::collections::HashSet::new();
                for b in bindings {
                    if cmd.pending_keys.len() < b.sequence.len() {
                        let next_key = &b.sequence[cmd.pending_keys.len()];
                        let key_str = next_key.display_key();
                        if displayed_keys.insert(key_str.clone()) {
                            let is_prefix = b.sequence.len() > cmd.pending_keys.len() + 1;
                            let label = if is_prefix {
                                match key_str.as_str() {
                                    "e" => "Edit Filter",
                                    "g" => "Goto",
                                    _ => "Prefix",
                                }
                            } else {
                                b.label
                            };

                            let is_active = match b.action {
                                ActionId::ToggleFollow => ctx.is_following,
                                ActionId::TogglePinFilter => ctx.is_pinned,
                                ActionId::ToggleRegex => ctx.is_regex,
                                ActionId::ToggleNegate => ctx.is_negated,
                                ActionId::ToggleCaseSensitive => ctx.is_case_sensitive,
                                ActionId::ToggleInterleave => ctx.show_bookmarks,
                                ActionId::ToggleBoolean => ctx.is_boolean,
                                _ => false,
                            };

                            let key_color = if is_active { Color::Green } else { Color::Cyan };
                            let label_color = if is_active {
                                Color::Green
                            } else {
                                Color::White
                            };

                            hints_spans.push(Span::styled(key_str, Style::default().fg(key_color)));
                            hints_spans.push(Span::styled(
                                format!(":{} ", label),
                                Style::default().fg(label_color),
                            ));
                        }
                    }
                }
            }
            Mode::Filter => {
                hints_spans.push(Span::styled("[Esc] ", Style::default().fg(Color::Cyan)));
                hints_spans.push(Span::styled("Cancel  ", Style::default().fg(Color::White)));
                hints_spans.push(Span::styled("[Enter] ", Style::default().fg(Color::Cyan)));
                hints_spans.push(Span::styled("Confirm  ", Style::default().fg(Color::White)));
                hints_spans.push(Span::styled("> ", Style::default().fg(Color::Yellow)));
                render_input_with_cursor(&cmd.filter_input, cmd.filter_cursor, &mut hints_spans);
            }
            Mode::Search => {
                hints_spans.push(Span::styled("[Esc] ", Style::default().fg(Color::Cyan)));
                hints_spans.push(Span::styled("Cancel  ", Style::default().fg(Color::White)));
                hints_spans.push(Span::styled("[Enter] ", Style::default().fg(Color::Cyan)));
                hints_spans.push(Span::styled("Confirm  ", Style::default().fg(Color::White)));
                hints_spans.push(Span::styled("/ ", Style::default().fg(Color::Yellow)));
                render_input_with_cursor(&cmd.search_input, cmd.search_cursor, &mut hints_spans);
            }
            Mode::Help => {
                hints_spans.push(Span::styled("[Esc] ", Style::default().fg(Color::Cyan)));
                hints_spans.push(Span::styled("Close  ", Style::default().fg(Color::White)));
                hints_spans.push(Span::styled("[↑/↓] ", Style::default().fg(Color::Cyan)));
                hints_spans.push(Span::styled(
                    "Navigate  ",
                    Style::default().fg(Color::White),
                ));
                hints_spans.push(Span::styled("[Enter] ", Style::default().fg(Color::Cyan)));
                hints_spans.push(Span::styled("Execute  ", Style::default().fg(Color::White)));
            }
            Mode::LineDetail => {
                hints_spans.push(Span::styled("[Esc/q] ", Style::default().fg(Color::Cyan)));
                hints_spans.push(Span::styled("Close  ", Style::default().fg(Color::White)));
            }
        }

        spans.extend(hints_spans);

        let empty_space = Span::styled(" ".repeat(200), Style::default().bg(Color::DarkGray));
        spans.push(empty_space);

        Paragraph::new(Line::from(spans))
            .style(Style::default().bg(Color::DarkGray).fg(Color::White))
    }
}

fn render_input_with_cursor(input: &str, cursor: usize, spans: &mut Vec<Span<'static>>) {
    let chars: Vec<char> = input.chars().collect();
    if cursor >= chars.len() {
        spans.push(Span::styled(input.to_string(), Style::default().fg(Color::Yellow)));
        spans.push(Span::styled(
            " ",
            Style::default().bg(Color::White).fg(Color::Black),
        ));
    } else {
        let before: String = chars[..cursor].iter().collect();
        let current: String = chars[cursor].to_string();
        let after: String = chars[cursor + 1..].iter().collect();
 
        spans.push(Span::styled(before, Style::default().fg(Color::Yellow)));
        spans.push(Span::styled(
            current,
            Style::default().bg(Color::White).fg(Color::Black),
        ));
        spans.push(Span::styled(after, Style::default().fg(Color::Yellow)));
    }
}
