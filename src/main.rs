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
use crossterm::event::{KeyCode, KeyModifiers};

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

        if !app.tabs.is_empty() {
            let tab = &mut app.tabs[app.active_tab];
            let num_panes = tab.panes.len();
            let content_height = terminal_size.height.saturating_sub(2) as usize; // Tab bar + Status bar 
            let individual_height = content_height / num_panes.max(1);

            current_line = tab.panes[tab.active_pane].selected_line;

            // Gather metrics safely
            {
                let offsets = tab.indexer.offsets.read().await;
                total_lines = offsets.len().saturating_sub(1);
                file_size = offsets.last().copied().unwrap_or(0);
            }

            // Update bounds
            for pane in &mut tab.panes {
                pane.height = individual_height;
                if pane.height > 2 { 
                    // account for borders realistically
                    pane.height -= 2; 
                } 
                if pane.selected_line < pane.scroll_offset {
                    pane.scroll_offset = pane.selected_line;
                } else if pane.selected_line >= pane.scroll_offset + pane.height {
                    pane.scroll_offset = pane.selected_line + 1 - pane.height;
                }
            }

            let mut sync_main_line: Option<usize> = None;

            for p_idx in 1..tab.panes.len() {
                let pane = &tab.panes[p_idx];
                if pane.is_filter {
                    let matched_lines = pane.matched_lines.read().await;
                    let mut visible_indices: Vec<usize>;
                    
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

        terminal.draw(|f| {
            let (tabs_area, main_area, status_area) = ui::layout::LayoutTree::split_main(f.size());
            
            use ratatui::widgets::{Block, Borders, Paragraph};
            use ratatui::layout::{Layout, Direction, Constraint};
            use ratatui::text::{Span, Line};
            use ratatui::style::{Color, Modifier, Style};

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
                let num_panes = tab.panes.len() as u16;
                let constraints: Vec<Constraint> = (0..num_panes).map(|_| Constraint::Ratio(1, num_panes as u32)).collect();
                let pane_rects = Layout::default().direction(Direction::Vertical).constraints(constraints).split(main_area);
                
                for (i, pane) in tab.panes.iter().enumerate() {
                    let mut text_lines = Vec::new();
                    for (absolute_line, is_selected, line_text) in &pane_contents[i] {
                        let is_marked = tab.bookmarks.contains(absolute_line);
                        let mark_icon = if is_marked { "★ " } else { "  " };
                        let prefix = format!("{}{:>5} │ ", mark_icon, absolute_line);
                        
                        let mut style = Style::default();
                        if *is_selected {
                            if i == tab.active_pane {
                                style = style.bg(Color::Rgb(60, 60, 60)).add_modifier(Modifier::BOLD);
                            } else {
                                style = style.bg(Color::Rgb(40, 40, 40));
                            }
                        }
                        
                        let span_prefix = Span::styled(prefix, style.clone().fg(if is_marked { Color::Red } else { Color::Yellow }));
                        let span_content = Span::styled(line_text.clone(), style);
                        text_lines.push(Line::from(vec![span_prefix, span_content]));
                    }

                    let title = if pane.is_filter {
                        let r_flag = if pane.is_regex { "R" } else { "S" };
                        let b_flag = if pane.show_bookmarks { "B" } else { "-" };
                        format!(" [{}] Filter: {} [{}/{}] ", i, pane.filter_query.as_deref().unwrap_or("*"), r_flag, b_flag)
                    } else {
                        format!(" [{}] {} ", i, tab.name)
                    };

                    let block = Block::default()
                        .borders(Borders::ALL)
                        .title(title)
                        .border_style(if i == tab.active_pane { ratatui::style::Style::default().fg(ratatui::style::Color::Yellow) } else { ratatui::style::Style::default() });
                        
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
            
            f.render_widget(ui::status_bar::StatusBar::render(&cmd_handler.mode, current_line, total_lines, file_size, &cmd_handler.filter_input, is_filter_pane), status_area);

        })?;

        if let Some(event) = events.next().await {
            match event {
                Event::Key(key) => {
                    let action = cmd_handler.handle_key(key);
                    match action {
                        commands::Action::Quit => {
                            app.quit();
                        }
                        commands::Action::ScrollDown => {
                            if !app.tabs.is_empty() {
                                let tab = &mut app.tabs[app.active_tab];
                                tab.panes[tab.active_pane].scroll_down();
                            }
                        }
                        commands::Action::ScrollUp => {
                            if !app.tabs.is_empty() {
                                let tab = &mut app.tabs[app.active_tab];
                                tab.panes[tab.active_pane].scroll_up();
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
                        commands::Action::SubmitFilter(query, is_stacking) => {
                            if !app.tabs.is_empty() {
                                let tab = &mut app.tabs[app.active_tab];
                                let active_idx = tab.active_pane;
                                
                                if is_stacking {
                                    tab.add_filter(query, Some(active_idx));
                                    tab.active_pane = tab.panes.len() - 1;
                                } else if tab.panes[active_idx].is_filter {
                                    tab.panes[active_idx].filter_query = Some(query);
                                    tab.update_filter_pane(active_idx);
                                } else {
                                    tab.add_filter(query, None);
                                    tab.active_pane = tab.panes.len() - 1;
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
                        commands::Action::NextMode(_) | commands::Action::None => {}
                    }
                }

                Event::Tick => {
                    app.tick();
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
