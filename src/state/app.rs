use crate::io::{indexer::Indexer, reader::AsyncReader};
use crate::io::filter::spawn_filter_task;
use crate::state::pane::Pane;
use std::collections::HashSet;
use std::path::PathBuf;

pub struct Tab {
    pub name: String,
    pub filepath: PathBuf,
    pub indexer: Indexer,
    pub reader: AsyncReader,
    pub panes: Vec<Pane>,
    pub active_pane: usize,
    pub bookmarks: HashSet<usize>,
}

impl Tab {
    pub fn new(filepath: PathBuf) -> Self {
        let name = filepath.file_name().unwrap_or_default().to_string_lossy().to_string();
        let indexer = Indexer::new(filepath.clone());
        let reader = AsyncReader::new(filepath.clone(), indexer.offsets.clone());

        indexer.start();

        Self {
            name,
            filepath,
            indexer,
            reader,
            panes: vec![Pane::new(false, None)],
            active_pane: 0,
            bookmarks: HashSet::new(),
        }
    }

    pub fn is_pane_collapsed(&self, idx: usize) -> bool {
        let pane = &self.panes[idx];
        if !pane.is_filter { return false; }
        if idx == self.active_pane { return false; }
        if pane.is_pinned { return false; }
        true
    }

    pub fn add_filter(&mut self, query: String, parent_pane: Option<usize>) {
        let mut new_pane = Pane::new(true, Some(query));
        new_pane.parent_pane = parent_pane;
        self.panes.push(new_pane);
        self.update_filter_pane(self.panes.len() - 1);
    }

    pub fn remove_pane(&mut self, idx: usize) {
        if idx == 0 || idx >= self.panes.len() { return; }

        let mut to_remove = vec![idx];
        let mut i = idx + 1;
        while i < self.panes.len() {
            if let Some(parent) = self.panes[i].parent_pane {
                if to_remove.contains(&parent) {
                    to_remove.push(i);
                }
            }
            i += 1;
        }

        to_remove.sort_unstable_by(|a, b| b.cmp(a));
        for r_idx in &to_remove {
            self.panes[*r_idx].task_generation.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.panes.remove(*r_idx);
        }

        for pane in &mut self.panes {
            if let Some(parent) = pane.parent_pane {
                let shift = to_remove.iter().filter(|&&r| r < parent).count();
                pane.parent_pane = Some(parent - shift);
            }
        }

        if self.active_pane >= self.panes.len() {
            self.active_pane = self.panes.len() - 1;
        }
    }

    pub fn retain_pane(&mut self, target_idx: usize) {
        if target_idx == 0 || target_idx >= self.panes.len() {
            let len = self.panes.len();
            for i in (1..len).rev() { self.remove_pane(i); }
            return;
        }

        let mut keepers = vec![0, target_idx];
        let mut curr = target_idx;
        while let Some(parent) = self.panes[curr].parent_pane {
            if !keepers.contains(&parent) { keepers.push(parent); }
            curr = parent;
        }

        let mut to_remove = Vec::new();
        for i in 1..self.panes.len() {
            if !keepers.contains(&i) {
                to_remove.push(i);
            }
        }
        to_remove.sort_unstable_by(|a, b| b.cmp(a));
        for r_idx in &to_remove {
            self.panes[*r_idx].task_generation.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.panes.remove(*r_idx);
        }

        for pane in &mut self.panes {
            if let Some(parent) = pane.parent_pane {
                let shift = to_remove.iter().filter(|&&r| r < parent).count();
                pane.parent_pane = Some(parent - shift);
            }
        }

        let shift = to_remove.iter().filter(|&&r| r < target_idx).count();
        self.active_pane = target_idx - shift;
    }

    pub fn update_filter_pane(&mut self, pane_idx: usize) {
        let pane = &mut self.panes[pane_idx];
        if !pane.is_filter { return; }

        let query = pane.filter_query.clone().unwrap_or_default();
        let is_regex = pane.is_regex;
        let is_negated = pane.is_negated;
        let matched_lines = pane.matched_lines.clone();

        let expected_gen = pane.task_generation.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        let task_generation = pane.task_generation.clone();

        let parent_matched = if let Some(pidx) = pane.parent_pane {
            Some(self.panes[pidx].matched_lines.clone())
        } else {
            None
        };

        let offsets = self.indexer.offsets.clone();
        let filepath = self.filepath.clone();

        spawn_filter_task(
            filepath,
            offsets,
            query,
            is_regex,
            is_negated,
            matched_lines,
            task_generation,
            expected_gen,
            parent_matched,
        );
    }
}

pub struct App {
    pub should_quit: bool,
    pub tabs: Vec<Tab>,
    pub active_tab: usize,
}

impl App {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            tabs: Vec::new(),
            active_tab: 0,
        }
    }

    pub fn add_tab(&mut self, filepath: PathBuf) {
        self.tabs.push(Tab::new(filepath));
        self.active_tab = self.tabs.len() - 1;
    }

    /// Returns a mutable reference to the active tab, or None if no tabs exist.
    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active_tab)
    }

    /// Returns an immutable reference to the active tab, or None if no tabs exist.
    pub fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active_tab)
    }

    pub fn tick(&mut self) {
        // Handle periodic updates if necessary
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
