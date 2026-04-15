mod events;
mod state;
mod input;
mod io;
mod ui;
mod dispatch;

use color_eyre::Result;
use crossterm::{
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::time::Duration;
use events::{Events, Event};
use ui::render::RenderContext;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    enable_raw_mode()?;
    std::io::stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;

    let mut events = Events::new(Duration::from_millis(250));
    let mut app = state::app::App::new();
    let mut cmd_handler = input::handler::CommandHandler::new();

    // If an argument is provided, load it as a tab
    if let Some(filepath) = std::env::args().nth(1) {
        app.add_tab(std::path::PathBuf::from(filepath));
    }

    loop {
        // --- Pre-render: gather data ---
        let (pane_contents, ctx) = prepare_frame(&mut app, &terminal).await;

        // --- Render ---
        terminal.draw(|f| {
            ui::render::draw(f, &app, &cmd_handler, &pane_contents, &ctx);
        })?;

        // --- Handle events ---
        if let Some(event) = events.next().await {
            match event {
                Event::Key(key) => {
                    cmd_handler.check_timeout();
                    let action = cmd_handler.handle_key(key, ctx.current_line);
                    dispatch::dispatch(action, &mut app, &mut cmd_handler, ctx.total_lines, ctx.current_line).await;
                }
                Event::Tick => {
                    dispatch::tick(&mut app, ctx.total_lines).await;
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

/// Gather all data needed for rendering: pane contents and render context.
async fn prepare_frame(
    app: &mut state::app::App,
    terminal: &Terminal<CrosstermBackend<std::io::Stdout>>,
) -> (Vec<Vec<(usize, bool, String)>>, RenderContext) {
    let mut pane_contents: Vec<Vec<(usize, bool, String)>> = Vec::new();
    let terminal_size = terminal.size().unwrap_or_default();

    let mut total_lines = 0;
    let mut file_size: u64 = 0;
    let mut current_line = 0;
    let mut pane_total_lines = 0;
    let mut active_is_following = false;
    let mut is_filter_pane = false;

    if let Some(tab) = app.active_tab_mut() {
        let num_panes = tab.panes.len();
        let content_height = terminal_size.height.saturating_sub(2) as usize;
        let collapsed_count = tab.panes.iter().enumerate()
            .filter(|(i, _)| tab.is_pane_collapsed(*i))
            .count();
        let expanded_count = num_panes - collapsed_count;
        let usable_height = content_height.saturating_sub(collapsed_count);

        // Gather metrics
        {
            let offsets = tab.indexer.offsets.read().await;
            total_lines = offsets.len().saturating_sub(1);
            file_size = offsets.last().copied().unwrap_or(0);
        }

        current_line = {
            let pane = &tab.panes[tab.active_pane];
            if pane.is_filter {
                let ml = pane.matched_lines.try_read();
                if let Ok(ml_guard) = ml {
                    pane_total_lines = ml_guard.len();
                    if pane.show_bookmarks {
                        pane_total_lines += tab.bookmarks.len();
                    }
                    ml_guard.get(pane.selected_line).copied().unwrap_or(0)
                } else { 0 }
            } else {
                pane_total_lines = total_lines;
                pane.selected_line
            }
        };

        active_is_following = tab.panes[tab.active_pane].is_following;
        is_filter_pane = tab.panes[tab.active_pane].is_filter;

        let mut collapsed_flags = Vec::with_capacity(tab.panes.len());
        for i in 0..tab.panes.len() {
            collapsed_flags.push(tab.is_pane_collapsed(i));
        }

        // Update pane heights and scroll offsets
        for (i, pane) in tab.panes.iter_mut().enumerate() {
            if collapsed_flags[i] {
                pane.height = 0;
            } else {
                if expanded_count == 1 {
                    pane.height = usable_height;
                } else if i == 0 {
                    // Main pane gets 2/3 when a filter is active
                    pane.height = (usable_height * 2) / 3;
                } else {
                    // Active filter pane gets 1/3
                    pane.height = usable_height / 3;
                }

                if pane.height > 2 {
                    pane.height -= 2;
                }
            }
            if pane.height > 0 {
                let padding = 3.min(pane.height.saturating_sub(1) / 2);
                if pane.selected_line < pane.scroll_offset + padding {
                    pane.scroll_offset = pane.selected_line.saturating_sub(padding);
                } else if pane.selected_line >= pane.scroll_offset + pane.height.saturating_sub(padding) {
                    pane.scroll_offset = (pane.selected_line + padding + 1).saturating_sub(pane.height);
                }
            }
        }

        // Fetch filter pane contents + sync main pane cursor
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
                pane_contents.push(Vec::new());
            }
        }

        if let Some(target) = sync_main_line {
            tab.panes[0].selected_line = target;
            let height = tab.panes[0].height;
            if height > 0 {
                let padding = 3.min(height.saturating_sub(1) / 2);
                if tab.panes[0].selected_line < tab.panes[0].scroll_offset + padding {
                    tab.panes[0].scroll_offset = tab.panes[0].selected_line.saturating_sub(padding);
                } else if tab.panes[0].selected_line >= tab.panes[0].scroll_offset + height.saturating_sub(padding) {
                    tab.panes[0].scroll_offset = (tab.panes[0].selected_line + padding + 1).saturating_sub(height);
                }
            }
        }

        // Fetch main pane content
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

    let ctx = RenderContext {
        current_line,
        pane_total_lines,
        total_lines,
        file_size,
        is_following: active_is_following,
        is_filter_pane,
    };

    (pane_contents, ctx)
}
