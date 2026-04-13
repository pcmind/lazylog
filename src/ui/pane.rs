use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::sync::atomic::AtomicUsize;

// Individual Pane logic (content buffer, filtering, bookmarks)
pub struct Pane {
    pub is_filter: bool,
    pub filter_query: Option<String>,
    pub is_regex: bool,
    pub show_bookmarks: bool,
    pub scroll_offset: usize,
    pub selected_line: usize,
    pub height: usize,
    // Holds the indices of the original file lines that matched the query
    pub matched_lines: Arc<RwLock<Vec<usize>>>,
    pub task_generation: Arc<AtomicUsize>,
    pub parent_pane: Option<usize>,
}

impl Pane {
    pub fn new(is_filter: bool, filter_query: Option<String>) -> Self {
        Self {
            is_filter,
            filter_query,
            is_regex: false, // Default substring
            show_bookmarks: false,
            scroll_offset: 0,
            selected_line: 0,
            height: 0,
            matched_lines: Arc::new(RwLock::new(Vec::new())),
            task_generation: Arc::new(AtomicUsize::new(0)),
            parent_pane: None,
        }
    }


    pub fn scroll_down(&mut self) {
        self.selected_line += 1;
    }

    pub fn scroll_up(&mut self) {
        self.selected_line = self.selected_line.saturating_sub(1);
    }
}
