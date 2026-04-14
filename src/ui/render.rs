use ratatui::{
    layout::{Layout, Direction, Constraint},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::state::action::Mode;
use crate::state::app::App;
use crate::input::handler::CommandHandler;
use crate::ui::layout::LayoutTree;
use crate::ui::status_bar::StatusBar;
use crate::ui::help::render_help_popup;

/// Bundles render-time state that doesn't belong to App or CommandHandler.
pub struct RenderContext {
    pub current_line: usize,
    pub total_lines: usize,
    pub file_size: u64,
    pub is_following: bool,
    pub is_filter_pane: bool,
}

/// Split a line's text into styled spans, highlighting all occurrences of `query`
/// (case-insensitive).
pub fn build_search_spans(
    text: &str,
    query: &str,
    base_style: Style,
    highlight_style: Style,
) -> Vec<Span<'static>> {
    if query.is_empty() {
        return vec![Span::styled(text.to_string(), base_style)];
    }

    let text_lower = text.to_lowercase();
    let query_lower = query.to_lowercase();
    let mut spans = Vec::new();
    let mut last_end = 0;

    for (start, _) in text_lower.match_indices(&query_lower) {
        if start > last_end {
            spans.push(Span::styled(text[last_end..start].to_string(), base_style));
        }
        spans.push(Span::styled(text[start..start + query.len()].to_string(), highlight_style));
        last_end = start + query.len();
    }

    if last_end < text.len() {
        spans.push(Span::styled(text[last_end..].to_string(), base_style));
    }

    if spans.is_empty() {
        spans.push(Span::styled(text.to_string(), base_style));
    }

    spans
}

/// Main draw function: renders tabs, panes, status bar, and help overlay.
pub fn draw(
    f: &mut Frame,
    app: &App,
    cmd_handler: &CommandHandler,
    pane_contents: &[Vec<(usize, bool, String)>],
    ctx: &RenderContext,
) {
    let (tabs_area, main_area, status_area) = LayoutTree::split_main(f.size());

    let search_highlight_style = Style::default().bg(Color::Yellow).fg(Color::Black).add_modifier(Modifier::BOLD);

    // 1. Draw Tabs
    let tab_info = if app.tabs.is_empty() {
        " [No File] ".to_string()
    } else {
        format!(" [{}] ", app.tabs[app.active_tab].name)
    };
    f.render_widget(Paragraph::new(tab_info).block(Block::default().borders(Borders::BOTTOM)), tabs_area);

    // 2. Draw Main Content (Panes)
    if let Some(tab) = app.active_tab() {
        let expanded_panes = tab.panes.iter().enumerate()
            .filter(|(i, p)| !(p.is_filter && *i != tab.active_pane))
            .count() as u32;
        let constraints: Vec<Constraint> = tab.panes.iter().enumerate().map(|(i, pane)| {
            if pane.is_filter && i != tab.active_pane {
                Constraint::Length(1)
            } else if expanded_panes == 1 {
                Constraint::Percentage(100)
            } else if i == 0 {
                // Main pane gets 2/3 when a filter is active
                Constraint::Ratio(2, 3)
            } else {
                // Active filter pane gets 1/3
                Constraint::Ratio(1, 3)
            }
        }).collect();
        let pane_rects = Layout::default().direction(Direction::Vertical).constraints(constraints).split(main_area);

        for (i, pane) in tab.panes.iter().enumerate() {
            let h_offset = pane.horizontal_offset;
            let mut text_lines = Vec::new();

            if i < pane_contents.len() {
                for (absolute_line, is_selected, line_text) in &pane_contents[i] {
                    let is_marked = tab.bookmarks.contains(absolute_line);
                    let mark_icon = if is_marked { "★ " } else { "  " };
                    let prefix = format!("{}{:>5} │ ", mark_icon, absolute_line);

                    let mut style = Style::default().fg(Color::White);
                    if *is_selected {
                        if i == tab.active_pane {
                            style = style.bg(Color::Rgb(60, 60, 60)).add_modifier(Modifier::BOLD);
                        } else {
                            style = style.bg(Color::Rgb(40, 40, 40));
                        }
                    } else if let Mode::Visual { anchor_line } = cmd_handler.mode {
                        if i == tab.active_pane {
                            let start = anchor_line.min(ctx.current_line);
                            let end = anchor_line.max(ctx.current_line);
                            if *absolute_line >= start && *absolute_line <= end {
                                style = style.bg(Color::Rgb(20, 20, 80));
                            }
                        }
                    }

                    let span_prefix = Span::styled(prefix, style.fg(if is_marked { Color::Red } else { Color::Yellow }));

                    // Apply horizontal offset
                    let display_text = if h_offset < line_text.len() {
                        &line_text[h_offset..]
                    } else if h_offset > 0 && !line_text.is_empty() {
                        ""
                    } else {
                        line_text.as_str()
                    };

                    // Build content spans with search highlighting
                    let content_spans = if let Some(ref sq) = cmd_handler.search_query {
                        build_search_spans(display_text, sq, style, search_highlight_style)
                    } else {
                        vec![Span::styled(display_text.to_string(), style)]
                    };

                    let mut line_spans = vec![span_prefix];
                    line_spans.extend(content_spans);
                    text_lines.push(Line::from(line_spans));
                }
            }

            let is_collapsed = pane.is_filter && i != tab.active_pane;
            let title = if pane.is_filter {
                let r_flag = if pane.is_regex { "R" } else { "S" };
                let n_flag = if pane.is_negated { "N" } else { "-" };
                let b_flag = if pane.show_bookmarks { "B" } else { "-" };
                let indicator = if is_collapsed { "▶" } else { "▼" };
                format!(" {} [{}] Filter: {} [{}/{}/{}] ", indicator, i, pane.filter_query.as_deref().unwrap_or("*"), r_flag, n_flag, b_flag)
            } else {
                let follow_mark = if pane.is_following { " ⟳" } else { "" };
                format!(" [{}] {}{} ", i, tab.name, follow_mark)
            };

            let block = if is_collapsed {
                Block::default()
                    .borders(Borders::TOP)
                    .title(title)
                    .border_style(Style::default().fg(Color::DarkGray))
            } else {
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(if i == tab.active_pane { Style::default().fg(Color::Yellow) } else { Style::default() })
            };

            f.render_widget(Paragraph::new(text_lines).block(block), pane_rects[i]);
        }
    } else {
        let main_block = Block::default().title("Lazylog").borders(Borders::ALL);
        f.render_widget(Paragraph::new("No file loaded.\n\nUsage: lazylog <file>").block(main_block), main_area);
    }

    // 3. Draw Status bar
    f.render_widget(StatusBar::render(cmd_handler, ctx), status_area);

    // 4. Help overlay
    if cmd_handler.mode == Mode::Help {
        render_help_popup(f, &cmd_handler.registry, cmd_handler.help_selected, &cmd_handler.help_filter);
    }
}
