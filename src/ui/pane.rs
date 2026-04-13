use std::sync::Arc;
use tokio::sync::RwLock;
use std::sync::atomic::AtomicUsize;

// Individual Pane logic (content buffer, filtering, bookmarks)
pub struct Pane {
    pub is_filter: bool,
    pub filter_query: Option<String>,
    pub is_regex: bool,
    pub is_negated: bool,
    pub show_bookmarks: bool,
    pub scroll_offset: usize,
    pub selected_line: usize,
    pub height: usize,
    pub horizontal_offset: usize,
    pub is_following: bool,
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
            is_negated: false,
            show_bookmarks: false,
            scroll_offset: 0,
            selected_line: 0,
            height: 0,
            horizontal_offset: 0,
            is_following: false,
            matched_lines: Arc::new(RwLock::new(Vec::new())),
            task_generation: Arc::new(AtomicUsize::new(0)),
            parent_pane: None,
        }
    }

    pub fn scroll_down(&mut self, max_lines: usize) {
        let limit = max_lines.saturating_sub(1);
        if self.selected_line < limit {
            self.selected_line += 1;
        }
    }

    pub fn scroll_up(&mut self) {
        self.is_following = false;
        self.selected_line = self.selected_line.saturating_sub(1);
    }

    pub fn page_down(&mut self, max_lines: usize) {
        let limit = max_lines.saturating_sub(1);
        self.selected_line = (self.selected_line + self.height).min(limit);
    }

    pub fn page_up(&mut self) {
        self.is_following = false;
        self.selected_line = self.selected_line.saturating_sub(self.height);
    }

    pub fn half_page_down(&mut self, max_lines: usize) {
        let half = self.height / 2;
        let limit = max_lines.saturating_sub(1);
        self.selected_line = (self.selected_line + half).min(limit);
    }

    pub fn half_page_up(&mut self) {
        self.is_following = false;
        let half = self.height / 2;
        self.selected_line = self.selected_line.saturating_sub(half);
    }

    pub fn goto_top(&mut self) {
        self.is_following = false;
        self.selected_line = 0;
    }

    pub fn goto_bottom(&mut self, max_lines: usize) {
        self.selected_line = max_lines.saturating_sub(1);
    }

    pub fn scroll_left(&mut self) {
        self.horizontal_offset = self.horizontal_offset.saturating_sub(4);
    }

    pub fn scroll_right(&mut self) {
        self.horizontal_offset += 4;
    }
}
