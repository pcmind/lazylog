mod config;
mod dispatch;
mod events;
mod input;
mod io;
mod state;
mod ui;

use color_eyre::Result;
use crossterm::{
    ExecutableCommand,
    event::{DisableMouseCapture, EnableMouseCapture, MouseEvent, MouseEventKind},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use events::{Event, Events};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
};
use std::time::Duration;
use ui::render::RenderContext;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    enable_raw_mode()?;
    std::io::stdout().execute(EnterAlternateScreen)?;
    std::io::stdout().execute(EnableMouseCapture)?;
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
        let (pane_contents, ctx, pane_rects) = prepare_frame(&mut app, &terminal).await;

        // --- Render ---
        terminal.draw(|f| {
            ui::render::draw(f, &app, &cmd_handler, &pane_contents, &ctx, &pane_rects);
        })?;

        // --- Handle events ---
        if let Some(event) = events.next().await {
            match event {
                Event::Key(key) => {
                    let action = cmd_handler.handle_key(key, ctx.current_line);
                    dispatch::dispatch(
                        action,
                        &mut app,
                        &mut cmd_handler,
                        ctx.total_lines,
                        ctx.current_line,
                    )
                    .await;
                }
                Event::Mouse(mouse) => {
                    handle_mouse_event(
                        mouse,
                        &mut app,
                        &mut cmd_handler,
                        &pane_rects,
                        ctx.total_lines,
                        ctx.current_line,
                    )
                    .await;
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
    std::io::stdout().execute(DisableMouseCapture)?;
    std::io::stdout().execute(LeaveAlternateScreen)?;

    Ok(())
}

async fn handle_mouse_event(
    mouse: MouseEvent,
    app: &mut state::app::App,
    cmd_handler: &mut input::handler::CommandHandler,
    pane_rects: &[Rect],
    total_lines: usize,
    current_line: usize,
) {
    let x = mouse.column;
    let y = mouse.row;

    if let MouseEventKind::Up(_) = mouse.kind {
        cmd_handler.dragging_pane = None;
    }

    if let Some(tab) = app.active_tab_mut() {
        if let Some(i) = cmd_handler.dragging_pane {
            if let MouseEventKind::Drag(_) = mouse.kind {
                if i > 0 && i < pane_rects.len() {
                    let prev_i = i - 1;
                    let total_h = pane_rects[prev_i].height + pane_rects[i].height;
                    let total_p = tab.panes[prev_i].height_percent + tab.panes[i].height_percent;

                    let new_prev_h = (y as i16 - pane_rects[prev_i].y as i16).max(1) as u16;
                    let new_prev_p =
                        (new_prev_h as u32 * total_p as u32 / total_h.max(1) as u32) as u16;
                    let new_prev_p = new_prev_p.clamp(5, total_p.saturating_sub(5));

                    tab.panes[prev_i].height_percent = new_prev_p;
                    tab.panes[i].height_percent = total_p.saturating_sub(new_prev_p);
                }
                return;
            }
        }

        for (i, rect) in pane_rects.iter().enumerate() {
            if x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height {
                // Click to focus or start drag
                if let MouseEventKind::Down(_) = mouse.kind {
                    if i > 0 && y == rect.y {
                        cmd_handler.dragging_pane = Some(i);
                    } else {
                        tab.active_pane = i;
                    }
                }

                // Scroll
                let action = match mouse.kind {
                    MouseEventKind::ScrollUp => Some(state::action::Action::ScrollUp),
                    MouseEventKind::ScrollDown => Some(state::action::Action::ScrollDown),
                    _ => None,
                };

                if let Some(act) = action {
                    tab.active_pane = i;
                    dispatch::dispatch(act, app, cmd_handler, total_lines, current_line).await;
                }

                break;
            }
        }
    }
}

/// Gather all data needed for rendering: pane contents and render context.
async fn prepare_frame(
    app: &mut state::app::App,
    terminal: &Terminal<CrosstermBackend<std::io::Stdout>>,
) -> (Vec<Vec<(usize, bool, String)>>, RenderContext, Vec<Rect>) {
    let mut pane_contents: Vec<Vec<(usize, bool, String)>> = Vec::new();
    let terminal_size = terminal.size().unwrap_or_default();
    let mut pane_rects = Vec::new();

    let mut total_lines = 0;
    let mut file_size: u64 = 0;
    let mut current_line = 0;
    let mut active_is_following = false;
    let mut is_filter_pane = false;

    if let Some(tab) = app.active_tab_mut() {
        let (main_area, _) = ui::layout::LayoutTree::split_main(terminal_size);

        let mut collapsed_flags = Vec::with_capacity(tab.panes.len());
        for i in 0..tab.panes.len() {
            collapsed_flags.push(tab.is_pane_collapsed(i));
        }

        // Calculate pane_rects
        let total_weight: u16 = tab
            .panes
            .iter()
            .enumerate()
            .filter(|(i, _)| !collapsed_flags[*i])
            .map(|(_, p)| p.height_percent)
            .sum();

        let constraints: Vec<Constraint> = tab
            .panes
            .iter()
            .enumerate()
            .map(|(i, pane)| {
                if collapsed_flags[i] {
                    Constraint::Length(1)
                } else {
                    Constraint::Ratio(pane.height_percent as u32, total_weight.max(1) as u32)
                }
            })
            .collect();

        pane_rects = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(main_area)
            .to_vec();

        // Gather metrics
        {
            let offsets = tab.indexer.offsets.read().await;
            total_lines = offsets.len().saturating_sub(1);
            file_size = offsets.last().copied().unwrap_or(0);
        }

        current_line = tab.absolute_line_sync(tab.active_pane).unwrap_or(0);

        active_is_following = tab.panes[tab.active_pane].is_following;
        is_filter_pane = tab.panes[tab.active_pane].is_filter;

        // Sync all inactive panes to the active pane's absolute line
        let target = current_line;
        for i in 0..tab.panes.len() {
            if i == tab.active_pane {
                continue;
            }
            let pane = &mut tab.panes[i];
            if !pane.is_filter {
                pane.selected_line = target;
                continue;
            }
            let matched = pane.matched_lines.read().await;
            let mut best_idx = 0;
            if matched.is_empty() && (!pane.show_bookmarks || tab.bookmarks.is_empty()) {
                pane.selected_line = 0;
                continue;
            }
            if pane.show_bookmarks {
                let mut book_vec: Vec<usize> = tab.bookmarks.iter().copied().collect();
                book_vec.sort_unstable();
                let mut m_it = matched.iter().peekable();
                let mut b_it = book_vec.iter().peekable();
                let mut current_idx = 0;
                let mut min_diff = usize::MAX;
                loop {
                    let value = match (m_it.peek(), b_it.peek()) {
                        (Some(&&m), Some(&&b)) => {
                            if m < b {
                                m_it.next();
                                m
                            } else if b < m {
                                b_it.next();
                                b
                            } else {
                                m_it.next();
                                b_it.next();
                                m
                            }
                        }
                        (Some(&&m), None) => {
                            m_it.next();
                            m
                        }
                        (None, Some(&&b)) => {
                            b_it.next();
                            b
                        }
                        (None, None) => break,
                    };
                    let diff = value.abs_diff(target);
                    if diff < min_diff {
                        min_diff = diff;
                        best_idx = current_idx;
                    } else if diff > min_diff {
                        break;
                    }
                    current_idx += 1;
                }
            } else {
                let idx_res = matched.binary_search(&target);
                best_idx = match idx_res {
                    Ok(idx) => idx,
                    Err(idx) => {
                        if idx == 0 {
                            0
                        } else if idx == matched.len() {
                            matched.len().saturating_sub(1)
                        } else {
                            let d_next = matched[idx] - target;
                            let d_prev = target - matched[idx - 1];
                            if d_prev <= d_next { idx - 1 } else { idx }
                        }
                    }
                };
            }
            pane.selected_line = best_idx;
        }

        // Update pane heights and scroll offsets
        for (i, pane) in tab.panes.iter_mut().enumerate() {
            let area = pane_rects[i];
            if collapsed_flags[i] {
                pane.height = 0;
            } else {
                pane.height = area.height as usize;
                if pane.height > 2 {
                    pane.height -= 2;
                }
            }
            if pane.height > 0 {
                let padding = 3.min(pane.height.saturating_sub(1) / 2);
                if pane.selected_line < pane.scroll_offset + padding {
                    pane.scroll_offset = pane.selected_line.saturating_sub(padding);
                } else if pane.selected_line
                    >= pane.scroll_offset + pane.height.saturating_sub(padding)
                {
                    pane.scroll_offset =
                        (pane.selected_line + padding + 1).saturating_sub(pane.height);
                }
            }
        }

        // Fetch filter pane contents

        for p_idx in 1..tab.panes.len() {
            let pane = &tab.panes[p_idx];
            if pane.is_filter {
                let matched_lines = pane.matched_lines.read().await;
                let visible_indices: Vec<usize> = if pane.show_bookmarks {
                    let mut book_vec: Vec<usize> = tab.bookmarks.iter().copied().collect();
                    book_vec.sort_unstable();

                    let mut visible = Vec::with_capacity(pane.height);
                    let mut m_it = matched_lines.iter().peekable();
                    let mut b_it = book_vec.iter().peekable();
                    let mut current_idx = 0;

                    while visible.len() < pane.height {
                        let value = match (m_it.peek(), b_it.peek()) {
                            (Some(&&m), Some(&&b)) => {
                                if m < b {
                                    m_it.next();
                                    m
                                } else if b < m {
                                    b_it.next();
                                    b
                                } else {
                                    m_it.next();
                                    b_it.next();
                                    m
                                }
                            }
                            (Some(&&m), None) => {
                                m_it.next();
                                m
                            }
                            (None, Some(&&b)) => {
                                b_it.next();
                                b
                            }
                            (None, None) => break,
                        };

                        if current_idx >= pane.scroll_offset {
                            visible.push(value);
                        }
                        current_idx += 1;
                    }

                    visible
                } else {
                    matched_lines
                        .iter()
                        .skip(pane.scroll_offset)
                        .take(pane.height)
                        .copied()
                        .collect()
                };

                let lines = tab.reader.read_specific_lines(&visible_indices).await;
                let lines_with_info: Vec<(usize, bool, String)> = lines
                    .into_iter()
                    .enumerate()
                    .map(|(i, l)| {
                        let absolute_line = visible_indices[i];
                        let is_selected = (pane.scroll_offset + i) == pane.selected_line;
                        (absolute_line, is_selected, l)
                    })
                    .collect();
                pane_contents.push(lines_with_info);
            } else {
                pane_contents.push(Vec::new());
            }
        }

        // Fetch main pane content
        let mut main_pane_info = Vec::new();
        if !tab.panes.is_empty() {
            let p0 = &tab.panes[0];
            let lines = tab.reader.read_lines(p0.scroll_offset, p0.height).await;
            main_pane_info = lines
                .into_iter()
                .enumerate()
                .map(|(i, l)| {
                    let absolute_line = p0.scroll_offset + i;
                    let is_selected = absolute_line == p0.selected_line;
                    (absolute_line, is_selected, l)
                })
                .collect();
        }
        pane_contents.insert(0, main_pane_info);
    }

    let ctx = if let Some(tab) = app.active_tab() {
        let active_pane = &tab.panes[tab.active_pane];
        RenderContext {
            current_line,
            total_lines,
            file_size,
            is_following: active_is_following,
            is_filter_pane,
            is_regex: active_pane.is_regex,
            is_negated: active_pane.is_negated,
            is_case_sensitive: active_pane.is_case_sensitive,
            is_pinned: active_pane.is_pinned,
            show_bookmarks: active_pane.show_bookmarks,
            is_boolean: active_pane.is_boolean,
        }
    } else {
        RenderContext {
            current_line: 0,
            total_lines: 0,
            file_size: 0,
            is_following: false,
            is_filter_pane: false,
            is_regex: false,
            is_negated: false,
            is_case_sensitive: false,
            is_pinned: false,
            show_bookmarks: false,
            is_boolean: false,
        }
    };

    (pane_contents, ctx, pane_rects)
}
