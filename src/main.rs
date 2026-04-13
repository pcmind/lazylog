mod events;
mod app;
mod commands;
mod io;
mod ui;

use color_eyre::Result;
use crossterm::{
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::time::Duration;
use events::{Events, Event};

/// Split a line's text into styled spans, highlighting all occurrences of `query`
/// (case-insensitive). Matched substrings get `highlight_style`, the rest get `base_style`.
fn build_search_spans(
    text: &str,
    query: &str,
    base_style: ratatui::style::Style,
    highlight_style: ratatui::style::Style,
) -> Vec<ratatui::text::Span<'static>> {
    if query.is_empty() {
        return vec![ratatui::text::Span::styled(text.to_string(), base_style)];
    }

    let text_lower = text.to_lowercase();
    let query_lower = query.to_lowercase();
    let mut spans = Vec::new();
    let mut last_end = 0;

    for (start, _) in text_lower.match_indices(&query_lower) {
        if start > last_end {
            spans.push(ratatui::text::Span::styled(
                text[last_end..start].to_string(),
                base_style,
            ));
        }
        spans.push(ratatui::text::Span::styled(
            text[start..start + query.len()].to_string(),
            highlight_style,
        ));
        last_end = start + query.len();
    }

    if last_end < text.len() {
        spans.push(ratatui::text::Span::styled(
            text[last_end..].to_string(),
            base_style,
        ));
    }

    if spans.is_empty() {
        spans.push(ratatui::text::Span::styled(text.to_string(), base_style));
    }

    spans
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    enable_raw_mode()?;
    std::io::stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;

    let mut events = Events::new(Duration::from_millis(250));
    let mut app = app::App::new();
    let mut cmd_handler = commands::CommandHandler::new();

    // If an argument is provided, load it as a tab
    if let Some(filepath) = std::env::args().nth(1) {
        app.add_tab(std::path::PathBuf::from(filepath));
    }

    loop {
        // Fetch visible lines for all panes in the active tab before drawing
        let mut pane_contents: Vec<Vec<(usize, bool, String)>> = Vec::new();
        
        let terminal_size = terminal.size().unwrap_or_default();
        
        let mut total_lines = 0;
        let mut file_size: u64 = 0;
        let mut current_line = 0;
        let mut active_is_following = false;

        if !app.tabs.is_empty() {
            let tab = &mut app.tabs[app.active_tab];
            let num_panes = tab.panes.len();
            let content_height = terminal_size.height.saturating_sub(2) as usize; // Tab bar + Status bar 
            let collapsed_count = tab.panes.iter().enumerate()
                .filter(|(i, p)| p.is_filter && *i != tab.active_pane)
                .count();
            let expanded_count = num_panes - collapsed_count;
            let individual_height = content_height.saturating_sub(collapsed_count) / expanded_count.max(1);

            current_line = if !tab.panes.is_empty() {
                let pane = &tab.panes[tab.active_pane];
                if pane.is_filter {
                    let ml = pane.matched_lines.try_read();
                    if let Ok(ml_guard) = ml {
                        ml_guard.get(pane.selected_line).copied().unwrap_or(0)
                    } else { 0 }
                } else {
                    pane.selected_line
                }
            } else { 0 };

            active_is_following = tab.panes[tab.active_pane].is_following;

            // Gather metrics safely
            {
                let offsets = tab.indexer.offsets.read().await;
                total_lines = offsets.len().saturating_sub(1);
                file_size = offsets.last().copied().unwrap_or(0);
            }

            // Update bounds
            for (i, pane) in tab.panes.iter_mut().enumerate() {
                if pane.is_filter && i != tab.active_pane {
                    pane.height = 0; // collapsed
                } else {
                    pane.height = individual_height;
                    if pane.height > 2 { 
                        pane.height -= 2; 
                    } 
                }
                if pane.height > 0 {
                    if pane.selected_line < pane.scroll_offset {
                        pane.scroll_offset = pane.selected_line;
                    } else if pane.selected_line >= pane.scroll_offset + pane.height {
                        pane.scroll_offset = pane.selected_line + 1 - pane.height;
                    }
                }
            }

            let mut sync_main_line: Option<usize> = None;

            for p_idx in 1..tab.panes.len() {
                let pane = &tab.panes[p_idx];
                if pane.is_filter {
                    let matched_lines = pane.matched_lines.read().await;
                    let visible_indices: Vec<usize>;
                    
                    if pane.show_bookmarks {
                        let mut book_vec: Vec<usize> = tab.bookmarks.iter().copied().collect();
                        book_vec.sort_unstable();
                        
                        let mut union = Vec::with_capacity(matched_lines.len() + book_vec.len());
                        let mut m_it = matched_lines.iter().peekable();
                        let mut b_it = book_vec.iter().peekable();
                        
                        loop {
                            match (m_it.peek(), b_it.peek()) {
                                (Some(&&m), Some(&&b)) => {
                                    if m < b { union.push(m); m_it.next(); } 
                                    else if b < m { union.push(b); b_it.next(); } 
                                    else { union.push(m); m_it.next(); b_it.next(); }
                                }
                                (Some(&&m), None) => { union.push(m); m_it.next(); }
                                (None, Some(&&b)) => { union.push(b); b_it.next(); }
                                (None, None) => break,
                            }
                        }
                        
                        visible_indices = union.into_iter().skip(pane.scroll_offset).take(pane.height).collect();
                    } else {
                        visible_indices = matched_lines.iter().skip(pane.scroll_offset).take(pane.height).copied().collect();
                    }

                    let lines = tab.reader.read_specific_lines(&visible_indices).await;
                    let lines_with_info: Vec<(usize, bool, String)> = lines.into_iter().enumerate().map(|(i, l)| {
                        let absolute_line = visible_indices[i];
                        let is_selected = (pane.scroll_offset + i) == pane.selected_line && tab.active_pane == p_idx;
                        if is_selected && tab.active_pane != 0 {
                            sync_main_line = Some(absolute_line);
                        }
                        (absolute_line, is_selected, l)
                    }).collect();
                    pane_contents.push(lines_with_info);
                } else {
                    pane_contents.push(Vec::new()); // Safe pad (won't be triggered, handles index shift)
                }
            }

            if let Some(target) = sync_main_line {
                tab.panes[0].selected_line = target;
                let height = tab.panes[0].height;
                if tab.panes[0].selected_line < tab.panes[0].scroll_offset {
                    tab.panes[0].scroll_offset = tab.panes[0].selected_line;
                } else if tab.panes[0].selected_line >= tab.panes[0].scroll_offset + height {
                    tab.panes[0].scroll_offset = tab.panes[0].selected_line + 1 - height;
                }
            }

            let mut main_pane_info = Vec::new();
            if !tab.panes.is_empty() {
                let p0 = &tab.panes[0];
                let lines = tab.reader.read_lines(p0.scroll_offset, p0.height).await;
                main_pane_info = lines.into_iter().enumerate().map(|(i, l)| {
                    let absolute_line = p0.scroll_offset + i;
                    let is_selected = absolute_line == p0.selected_line;
                    (absolute_line, is_selected, l)
                }).collect();
            }
            pane_contents.insert(0, main_pane_info);
        }

        // Capture state for rendering closure
        let search_query_for_render = cmd_handler.search_query.clone();

        terminal.draw(|f| {
            let (tabs_area, main_area, status_area) = ui::layout::LayoutTree::split_main(f.size());
            
            use ratatui::widgets::{Block, Borders, Paragraph};
            use ratatui::layout::{Layout, Direction, Constraint};
            use ratatui::text::{Span, Line};
            use ratatui::style::{Color, Modifier, Style};

            let search_highlight_style = Style::default().bg(Color::Yellow).fg(Color::Black).add_modifier(Modifier::BOLD);

            // 1. Draw Tabs
            let tab_info = if app.tabs.is_empty() {
                " [No File] ".to_string()
            } else {
                format!(" [{}] ", app.tabs[app.active_tab].name)
            };
            f.render_widget(Paragraph::new(tab_info).block(Block::default().borders(Borders::BOTTOM)), tabs_area);

            // 2. Draw Main Content (Panes)
            if !app.tabs.is_empty() {
                let tab = &app.tabs[app.active_tab];
                let expanded_panes = tab.panes.iter().enumerate()
                    .filter(|(i, p)| !(p.is_filter && *i != tab.active_pane))
                    .count() as u32;
                let constraints: Vec<Constraint> = tab.panes.iter().enumerate().map(|(i, pane)| {
                    if pane.is_filter && i != tab.active_pane {
                        Constraint::Length(1)
                    } else {
                        Constraint::Ratio(1, expanded_panes)
                    }
                }).collect();
                let pane_rects = Layout::default().direction(Direction::Vertical).constraints(constraints).split(main_area);
                
                for (i, pane) in tab.panes.iter().enumerate() {
                    let h_offset = pane.horizontal_offset;
                    let mut text_lines = Vec::new();
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
                        } else if let commands::Mode::Visual { anchor_line } = cmd_handler.mode {
                            if i == tab.active_pane {
                                let start = anchor_line.min(current_line);
                                let end = anchor_line.max(current_line);
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
                        let content_spans = if let Some(ref sq) = search_query_for_render {
                            build_search_spans(display_text, sq, style, search_highlight_style)
                        } else {
                            vec![Span::styled(display_text.to_string(), style)]
                        };

                        let mut line_spans = vec![span_prefix];
                        line_spans.extend(content_spans);
                        text_lines.push(Line::from(line_spans));
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
            let is_filter_pane = if !app.tabs.is_empty() {
                app.tabs[app.active_tab].panes[app.tabs[app.active_tab].active_pane].is_filter
            } else {
                false
            };
            
            f.render_widget(ui::status_bar::StatusBar::render(
                &cmd_handler.registry,
                &cmd_handler.mode,
                current_line,
                total_lines,
                file_size,
                &cmd_handler.filter_input,
                is_filter_pane,
                &cmd_handler.pending_keys,
                active_is_following,
                &cmd_handler.search_input,
                &cmd_handler.search_query,
            ), status_area);

            if cmd_handler.mode == commands::Mode::Help {
                ui::help::render_help_popup(f, &cmd_handler.registry, cmd_handler.help_selected);
            }

        })?;

        if let Some(event) = events.next().await {
            match event {
                Event::Key(key) => {
                    cmd_handler.check_timeout();
                    let action = cmd_handler.handle_key(key, current_line);
                    match action {
                        commands::Action::Quit => {
                            app.quit();
                        }
                        commands::Action::ScrollDown => {
                            if !app.tabs.is_empty() {
                                let tab = &mut app.tabs[app.active_tab];
                                let max = get_max_lines(tab, tab.active_pane, total_lines).await;
                                tab.panes[tab.active_pane].scroll_down(max);
                            }
                        }
                        commands::Action::ScrollUp => {
                            if !app.tabs.is_empty() {
                                let tab = &mut app.tabs[app.active_tab];
                                tab.panes[tab.active_pane].scroll_up();
                            }
                        }
                        commands::Action::GotoTop => {
                            if !app.tabs.is_empty() {
                                let tab = &mut app.tabs[app.active_tab];
                                tab.panes[tab.active_pane].goto_top();
                            }
                        }
                        commands::Action::GotoBottom => {
                            if !app.tabs.is_empty() {
                                let tab = &mut app.tabs[app.active_tab];
                                let max = get_max_lines(tab, tab.active_pane, total_lines).await;
                                tab.panes[tab.active_pane].goto_bottom(max);
                            }
                        }
                        commands::Action::HalfPageDown => {
                            if !app.tabs.is_empty() {
                                let tab = &mut app.tabs[app.active_tab];
                                let max = get_max_lines(tab, tab.active_pane, total_lines).await;
                                tab.panes[tab.active_pane].half_page_down(max);
                            }
                        }
                        commands::Action::HalfPageUp => {
                            if !app.tabs.is_empty() {
                                let tab = &mut app.tabs[app.active_tab];
                                tab.panes[tab.active_pane].half_page_up();
                            }
                        }
                        commands::Action::PageDown => {
                            if !app.tabs.is_empty() {
                                let tab = &mut app.tabs[app.active_tab];
                                let max = get_max_lines(tab, tab.active_pane, total_lines).await;
                                tab.panes[tab.active_pane].page_down(max);
                            }
                        }
                        commands::Action::PageUp => {
                            if !app.tabs.is_empty() {
                                let tab = &mut app.tabs[app.active_tab];
                                tab.panes[tab.active_pane].page_up();
                            }
                        }
                        commands::Action::ScrollLeft => {
                            if !app.tabs.is_empty() {
                                let tab = &mut app.tabs[app.active_tab];
                                tab.panes[tab.active_pane].scroll_left();
                            }
                        }
                        commands::Action::ScrollRight => {
                            if !app.tabs.is_empty() {
                                let tab = &mut app.tabs[app.active_tab];
                                tab.panes[tab.active_pane].scroll_right();
                            }
                        }
                        // Handle Filter additions and pane navigation
                        commands::Action::NextPane => {
                            if !app.tabs.is_empty() {
                                let tab = &mut app.tabs[app.active_tab];
                                tab.active_pane = (tab.active_pane + 1) % tab.panes.len();
                            }
                        }
                        commands::Action::PrevPane => {
                            if !app.tabs.is_empty() {
                                let tab = &mut app.tabs[app.active_tab];
                                tab.active_pane = tab.active_pane.saturating_sub(1);
                            }
                        }
                        commands::Action::SubmitFilter(query, intent) => {
                            if !app.tabs.is_empty() {
                                let tab = &mut app.tabs[app.active_tab];
                                let active_idx = tab.active_pane;
                                
                                match intent {
                                    commands::FilterIntent::Stack => {
                                        tab.add_filter(query, Some(active_idx));
                                        tab.active_pane = tab.panes.len() - 1;
                                    }
                                    commands::FilterIntent::Edit => {
                                        if tab.panes[active_idx].is_filter {
                                            tab.panes[active_idx].filter_query = Some(query);
                                            tab.update_filter_pane(active_idx);
                                        }
                                    }
                                    commands::FilterIntent::New => {
                                        tab.add_filter(query, None);
                                        tab.active_pane = tab.panes.len() - 1;
                                    }
                                }
                            }
                        }
                        commands::Action::ToggleRegex => {
                            if !app.tabs.is_empty() {
                                let tab = &mut app.tabs[app.active_tab];
                                let active_pane = tab.active_pane;
                                if tab.panes[active_pane].is_filter {
                                    tab.panes[active_pane].is_regex = !tab.panes[active_pane].is_regex;
                                    tab.update_filter_pane(active_pane);
                                }
                            }
                        }
                        commands::Action::ToggleNegate => {
                            if !app.tabs.is_empty() {
                                let tab = &mut app.tabs[app.active_tab];
                                let active_pane = tab.active_pane;
                                if tab.panes[active_pane].is_filter {
                                    tab.panes[active_pane].is_negated = !tab.panes[active_pane].is_negated;
                                    tab.update_filter_pane(active_pane);
                                }
                            }
                        }
                        commands::Action::ToggleInterleave => {
                            if !app.tabs.is_empty() {
                                let tab = &mut app.tabs[app.active_tab];
                                let active_pane = tab.active_pane;
                                if tab.panes[active_pane].is_filter {
                                    tab.panes[active_pane].show_bookmarks = !tab.panes[active_pane].show_bookmarks;
                                }
                            }
                        }
                        commands::Action::EditFilter => {
                            if !app.tabs.is_empty() {
                                let tab = &app.tabs[app.active_tab];
                                let active_pane = tab.active_pane;
                                if tab.panes[active_pane].is_filter {
                                    cmd_handler.mode = commands::Mode::Filter;
                                    cmd_handler.filter_input = tab.panes[active_pane].filter_query.clone().unwrap_or_default();
                                }
                            }
                        }
                        commands::Action::FocusPane(idx) => {
                            if !app.tabs.is_empty() {
                                let tab = &mut app.tabs[app.active_tab];
                                if idx < tab.panes.len() {
                                    tab.active_pane = idx;
                                }
                            }
                        }
                        commands::Action::ClosePane => {
                            if !app.tabs.is_empty() {
                                let tab = &mut app.tabs[app.active_tab];
                                let idx = tab.active_pane;
                                tab.remove_pane(idx);
                            }
                        }
                        commands::Action::CloseOtherPanes => {
                            if !app.tabs.is_empty() {
                                let tab = &mut app.tabs[app.active_tab];
                                let idx = tab.active_pane;
                                tab.retain_pane(idx);
                            }
                        }
                        commands::Action::ToggleBookmark => {
                            if !app.tabs.is_empty() {
                                let tab = &mut app.tabs[app.active_tab];
                                let active_pane = &tab.panes[tab.active_pane];
                                let target_line = if active_pane.is_filter {
                                    // Map from matched to original
                                    let matched = active_pane.matched_lines.try_read();
                                    if let Ok(ml) = matched {
                                        ml.get(active_pane.selected_line).copied().unwrap_or(0)
                                    } else {
                                        0
                                    }
                                } else {
                                    active_pane.selected_line
                                };

                                if tab.bookmarks.contains(&target_line) {
                                    tab.bookmarks.remove(&target_line);
                                } else {
                                    tab.bookmarks.insert(target_line);
                                }
                            }
                        }
                        commands::Action::Yank(anchor) => {
                            if !app.tabs.is_empty() {
                                let start = anchor.min(current_line);
                                let end = anchor.max(current_line);
                                let count = end.saturating_sub(start) + 1;
                                
                                let lines = app.tabs[app.active_tab].reader.read_lines(start, count).await;
                                let text = lines.join("\n");
                                
                                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                                    let _ = clipboard.set_text(text);
                                }
                            }
                        }
                        // Search
                        commands::Action::SubmitSearch(_query) => {
                            // search_query is already stored in cmd_handler by handle_search
                            // Jump to first match after current line
                            if !app.tabs.is_empty() {
                                let tab = &mut app.tabs[app.active_tab];
                                let ap = tab.active_pane;
                                let max = get_max_lines(tab, ap, total_lines).await;
                                jump_to_search_match(tab, ap, max, &cmd_handler.search_query, current_line, true).await;
                            }
                        }
                        commands::Action::NextSearchResult => {
                            if cmd_handler.search_query.is_some() && !app.tabs.is_empty() {
                                let tab = &mut app.tabs[app.active_tab];
                                let ap = tab.active_pane;
                                let max = get_max_lines(tab, ap, total_lines).await;
                                jump_to_search_match(tab, ap, max, &cmd_handler.search_query, current_line, true).await;
                            }
                        }
                        commands::Action::PrevSearchResult => {
                            if cmd_handler.search_query.is_some() && !app.tabs.is_empty() {
                                let tab = &mut app.tabs[app.active_tab];
                                let ap = tab.active_pane;
                                let max = get_max_lines(tab, ap, total_lines).await;
                                jump_to_search_match(tab, ap, max, &cmd_handler.search_query, current_line, false).await;
                            }
                        }
                        // Follow mode
                        commands::Action::ToggleFollow => {
                            if !app.tabs.is_empty() {
                                let tab = &mut app.tabs[app.active_tab];
                                let ap = tab.active_pane;
                                tab.panes[ap].is_following = !tab.panes[ap].is_following;
                                if tab.panes[ap].is_following {
                                    let max = get_max_lines(tab, ap, total_lines).await;
                                    tab.panes[ap].goto_bottom(max);
                                    // Re-enable following (goto_bottom doesn't disable it)
                                    tab.panes[ap].is_following = true;
                                }
                            }
                        }
                        commands::Action::BeginSearch
                        | commands::Action::ClearSearch
                        | commands::Action::EnterVisual
                        | commands::Action::ShowHelp
                        | commands::Action::None => {}
                    }
                }

                Event::Tick => {
                    app.tick();
                    // Follow mode: auto-scroll to bottom on tick
                    if !app.tabs.is_empty() {
                        let tab = &mut app.tabs[app.active_tab];
                        let ap = tab.active_pane;
                        if tab.panes[ap].is_following {
                            let max = get_max_lines(tab, ap, total_lines).await;
                            if max > 0 {
                                tab.panes[ap].selected_line = max.saturating_sub(1);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if app.should_quit {
            break;
        }
    }

    disable_raw_mode()?;
    std::io::stdout().execute(LeaveAlternateScreen)?;

    Ok(())
}

/// Get the maximum number of lines for a pane (total_lines for main, matched_lines count for filters)
async fn get_max_lines(tab: &app::Tab, pane_idx: usize, total_lines: usize) -> usize {
    let pane = &tab.panes[pane_idx];
    if pane.is_filter {
        let ml = pane.matched_lines.read().await;
        if pane.show_bookmarks {
            // Union of matched + bookmarks
            let book_count = tab.bookmarks.len();
            // Approximate: this is an upper bound; exact would require dedup
            ml.len() + book_count
        } else {
            ml.len()
        }
    } else {
        total_lines
    }
}

/// Jump to the next (or previous) line matching the search query
async fn jump_to_search_match(
    tab: &mut app::Tab,
    pane_idx: usize,
    max_lines: usize,
    search_query: &Option<String>,
    current_line: usize,
    forward: bool,
) {
    let query = match search_query {
        Some(q) => q.to_lowercase(),
        None => return,
    };

    let pane = &tab.panes[pane_idx];

    if pane.is_filter {
        // Search within matched lines
        let matched = pane.matched_lines.read().await;
        if matched.is_empty() { return; }

        let current_idx = pane.selected_line;
        let len = matched.len();

        // Read lines around current position to find matches
        let range: Box<dyn Iterator<Item = usize>> = if forward {
            if current_idx + 1 < len {
                Box::new((current_idx + 1..len).chain(0..=current_idx))
            } else {
                Box::new((0..len).into_iter())
            }
        } else {
            if current_idx > 0 {
                Box::new((0..current_idx).rev().chain((current_idx..len).rev()))
            } else {
                Box::new((0..len).rev())
            }
        };

        for idx in range {
            let abs_line = matched[idx];
            let lines = tab.reader.read_specific_lines(&[abs_line]).await;
            if let Some(line_text) = lines.first() {
                if line_text.to_lowercase().contains(&query) {
                    drop(matched);
                    tab.panes[pane_idx].selected_line = idx;
                    return;
                }
            }
        }
    } else {
        // Main pane: scan forward/backward from current line
        let scan_count = max_lines.min(5000); // Cap to avoid scanning millions of lines
        
        let range: Vec<usize> = if forward {
            let start = current_line + 1;
            (0..scan_count).map(|i| (start + i) % max_lines).collect()
        } else {
            let start = if current_line == 0 { max_lines.saturating_sub(1) } else { current_line - 1 };
            (0..scan_count).map(|i| {
                if i <= start { start - i } else { max_lines - 1 - (i - start - 1) }
            }).collect()
        };

        // Read in batches for efficiency
        let batch_size = 100;
        for chunk in range.chunks(batch_size) {
            let lines = tab.reader.read_specific_lines(chunk).await;
            for (i, line_text) in lines.iter().enumerate() {
                if line_text.to_lowercase().contains(&query) {
                    tab.panes[pane_idx].selected_line = chunk[i];
                    return;
                }
            }
        }
    }
}
