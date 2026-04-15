use crate::state::action::{Action, FilterIntent, Mode};
use crate::state::app::{App, Tab};
use crate::input::handler::CommandHandler;

/// Dispatch an Action to mutate app state.
pub async fn dispatch(
    action: Action,
    app: &mut App,
    cmd_handler: &mut CommandHandler,
    total_lines: usize,
    current_line: usize,
) {
    match action {
        Action::Quit => {
            app.quit();
        }
        Action::ScrollDown => {
            if let Some(tab) = app.active_tab_mut() {
                let ap = tab.active_pane;
                let max = get_max_lines(tab, ap, total_lines).await;
                tab.panes[ap].scroll_down(max);
            }
        }
        Action::ScrollUp => {
            if let Some(tab) = app.active_tab_mut() {
                tab.panes[tab.active_pane].scroll_up();
            }
        }
        Action::GotoTop => {
            if let Some(tab) = app.active_tab_mut() {
                tab.panes[tab.active_pane].goto_top();
            }
        }
        Action::GotoBottom => {
            if let Some(tab) = app.active_tab_mut() {
                let ap = tab.active_pane;
                let max = get_max_lines(tab, ap, total_lines).await;
                tab.panes[ap].goto_bottom(max);
            }
        }
        Action::HalfPageDown => {
            if let Some(tab) = app.active_tab_mut() {
                let ap = tab.active_pane;
                let max = get_max_lines(tab, ap, total_lines).await;
                tab.panes[ap].half_page_down(max);
            }
        }
        Action::HalfPageUp => {
            if let Some(tab) = app.active_tab_mut() {
                tab.panes[tab.active_pane].half_page_up();
            }
        }
        Action::PageDown => {
            if let Some(tab) = app.active_tab_mut() {
                let ap = tab.active_pane;
                let max = get_max_lines(tab, ap, total_lines).await;
                tab.panes[ap].page_down(max);
            }
        }
        Action::PageUp => {
            if let Some(tab) = app.active_tab_mut() {
                tab.panes[tab.active_pane].page_up();
            }
        }
        Action::ScrollLeft => {
            if let Some(tab) = app.active_tab_mut() {
                tab.panes[tab.active_pane].scroll_left();
            }
        }
        Action::ScrollRight => {
            if let Some(tab) = app.active_tab_mut() {
                tab.panes[tab.active_pane].scroll_right();
            }
        }
        Action::NextPane => {
            if let Some(tab) = app.active_tab_mut() {
                tab.active_pane = (tab.active_pane + 1) % tab.panes.len();
            }
        }
        Action::PrevPane => {
            if let Some(tab) = app.active_tab_mut() {
                tab.active_pane = tab.active_pane.saturating_sub(1);
            }
        }
        Action::SubmitFilter(query, intent) => {
            if let Some(tab) = app.active_tab_mut() {
                let active_idx = tab.active_pane;
                match intent {

                    FilterIntent::Edit => {
                        if tab.panes[active_idx].is_filter {
                            tab.panes[active_idx].filter_query = Some(query);
                            tab.update_filter_pane(active_idx);
                        }
                    }
                    FilterIntent::New => {
                        tab.add_filter(query, None);
                        tab.active_pane = tab.panes.len() - 1;
                    }
                }
            }
        }
        Action::ToggleRegex => {
            if let Some(tab) = app.active_tab_mut() {
                let ap = tab.active_pane;
                if tab.panes[ap].is_filter {
                    tab.panes[ap].is_regex = !tab.panes[ap].is_regex;
                    tab.update_filter_pane(ap);
                }
            }
        }
        Action::ToggleNegate => {
            if let Some(tab) = app.active_tab_mut() {
                let ap = tab.active_pane;
                if tab.panes[ap].is_filter {
                    tab.panes[ap].is_negated = !tab.panes[ap].is_negated;
                    tab.update_filter_pane(ap);
                }
            }
        }
        Action::ToggleCaseSensitive => {
            if let Some(tab) = app.active_tab_mut() {
                let ap = tab.active_pane;
                if tab.panes[ap].is_filter {
                    tab.panes[ap].is_case_sensitive = !tab.panes[ap].is_case_sensitive;
                    tab.update_filter_pane(ap);
                }
            }
        }
        Action::TogglePinFilter => {
            if let Some(tab) = app.active_tab_mut() {
                let ap = tab.active_pane;
                if tab.panes[ap].is_filter {
                    tab.panes[ap].is_pinned = !tab.panes[ap].is_pinned;
                }
            }
        }
        Action::ToggleInterleave => {
            if let Some(tab) = app.active_tab_mut() {
                let ap = tab.active_pane;
                if tab.panes[ap].is_filter {
                    tab.panes[ap].show_bookmarks = !tab.panes[ap].show_bookmarks;
                }
            }
        }
        Action::EditFilter => {
            if let Some(tab) = app.active_tab() {
                let ap = tab.active_pane;
                if tab.panes[ap].is_filter {
                    cmd_handler.mode = Mode::Filter;
                    cmd_handler.filter_input = tab.panes[ap].filter_query.clone().unwrap_or_default();
                }
            }
        }
        Action::FocusPane(idx) => {
            if let Some(tab) = app.active_tab_mut() {
                if idx < tab.panes.len() {
                    tab.active_pane = idx;
                }
            }
        }
        Action::ClosePane => {
            if let Some(tab) = app.active_tab_mut() {
                let idx = tab.active_pane;
                tab.remove_pane(idx);
            }
        }
        Action::CloseOtherPanes => {
            if let Some(tab) = app.active_tab_mut() {
                let idx = tab.active_pane;
                tab.retain_pane(idx);
            }
        }
        Action::ToggleBookmark => {
            if let Some(tab) = app.active_tab_mut() {
                let ap = tab.active_pane;
                let target_line = if tab.panes[ap].is_filter {
                    let matched = tab.panes[ap].matched_lines.try_read();
                    if let Ok(ml) = matched {
                        ml.get(tab.panes[ap].selected_line).copied().unwrap_or(0)
                    } else {
                        0
                    }
                } else {
                    tab.panes[ap].selected_line
                };

                if tab.bookmarks.contains(&target_line) {
                    tab.bookmarks.remove(&target_line);
                } else {
                    tab.bookmarks.insert(target_line);
                }
            }
        }
        Action::Yank(anchor) => {
            if let Some(tab) = app.active_tab_mut() {
                let start = anchor.min(current_line);
                let end = anchor.max(current_line);
                let count = end.saturating_sub(start) + 1;

                let lines = tab.reader.read_lines(start, count).await;
                let text = lines.join("\n");

                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(text);
                }
            }
        }
        // Search
        Action::SubmitSearch(_query) => {
            if let Some(tab) = app.active_tab_mut() {
                let ap = tab.active_pane;
                let max = get_max_lines(tab, ap, total_lines).await;
                jump_to_search_match(tab, ap, max, &cmd_handler.search_query, current_line, true).await;
            }
        }
        Action::NextSearchResult => {
            if cmd_handler.search_query.is_some() {
                if let Some(tab) = app.active_tab_mut() {
                    let ap = tab.active_pane;
                    let max = get_max_lines(tab, ap, total_lines).await;
                    jump_to_search_match(tab, ap, max, &cmd_handler.search_query, current_line, true).await;
                }
            }
        }
        Action::PrevSearchResult => {
            if cmd_handler.search_query.is_some() {
                if let Some(tab) = app.active_tab_mut() {
                    let ap = tab.active_pane;
                    let max = get_max_lines(tab, ap, total_lines).await;
                    jump_to_search_match(tab, ap, max, &cmd_handler.search_query, current_line, false).await;
                }
            }
        }
        // Follow mode
        Action::ToggleFollow => {
            if let Some(tab) = app.active_tab_mut() {
                let ap = tab.active_pane;
                tab.panes[ap].is_following = !tab.panes[ap].is_following;
                if tab.panes[ap].is_following {
                    let max = get_max_lines(tab, ap, total_lines).await;
                    tab.panes[ap].goto_bottom(max);
                    tab.panes[ap].is_following = true;
                }
            }
        }
        Action::BeginSearch
        | Action::ClearSearch
        | Action::EnterVisual
        | Action::ShowHelp
        | Action::None => {}
    }
}

/// Handle tick: auto-scroll if following.
pub async fn tick(app: &mut App, total_lines: usize) {
    app.tick();
    if let Some(tab) = app.active_tab_mut() {
        let ap = tab.active_pane;
        if tab.panes[ap].is_following {
            let max = get_max_lines(tab, ap, total_lines).await;
            if max > 0 {
                tab.panes[ap].selected_line = max.saturating_sub(1);
            }
        }
    }
}

/// Get the maximum number of lines for a pane.
pub async fn get_max_lines(tab: &Tab, pane_idx: usize, total_lines: usize) -> usize {
    let pane = &tab.panes[pane_idx];
    if pane.is_filter {
        let ml = pane.matched_lines.read().await;
        if pane.show_bookmarks {
            let book_count = tab.bookmarks.len();
            ml.len() + book_count
        } else {
            ml.len()
        }
    } else {
        total_lines
    }
}

/// Jump to the next (or previous) line matching the search query.
async fn jump_to_search_match(
    tab: &mut Tab,
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
        let matched = pane.matched_lines.read().await;
        if matched.is_empty() { return; }

        let current_idx = pane.selected_line;
        let len = matched.len();

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
        let scan_count = max_lines.min(5000);

        let range: Vec<usize> = if forward {
            let start = current_line + 1;
            (0..scan_count).map(|i| (start + i) % max_lines).collect()
        } else {
            let start = if current_line == 0 { max_lines.saturating_sub(1) } else { current_line - 1 };
            (0..scan_count).map(|i| {
                if i <= start { start - i } else { max_lines - 1 - (i - start - 1) }
            }).collect()
        };

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
