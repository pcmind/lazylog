use crate::io::{indexer::Indexer, reader::AsyncReader};
use crate::ui::pane::Pane;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

pub struct Tab {
    pub name: String,
    pub filepath: PathBuf,
    pub indexer: Indexer,
    pub reader: AsyncReader,
    pub panes: Vec<Pane>,
    pub active_pane: usize,
    pub bookmarks: HashSet<usize>, // Original file line indices
}

impl Tab {
    pub fn new(filepath: PathBuf) -> Self {
        let name = filepath.file_name().unwrap_or_default().to_string_lossy().to_string();
        let indexer = Indexer::new(filepath.clone());
        let reader = AsyncReader::new(filepath.clone(), indexer.offsets.clone());
        
        indexer.start(); // Kick off background indexing
        
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
        let matched_lines = pane.matched_lines.clone();
        
        let expected_gen = pane.task_generation.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        let task_generation = pane.task_generation.clone();

        let parent_pane_idx = pane.parent_pane;
        let parent_matched_arc = if let Some(pidx) = parent_pane_idx {
            Some(self.panes[pidx].matched_lines.clone())
        } else {
            None
        };

        let offsets = self.indexer.offsets.clone();
        let filepath = self.filepath.clone();

        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            if task_generation.load(std::sync::atomic::Ordering::Relaxed) != expected_gen { return; }

            { matched_lines.write().await.clear(); }

            let regex = if is_regex { regex::Regex::new(&query).ok() } else { None };
            let query_lower = query.to_lowercase(); 

            let mut file = match tokio::fs::File::open(&filepath).await {
                Ok(f) => f,
                Err(_) => return,
            };

            let mut last_processed = 0;

            loop {
                if task_generation.load(std::sync::atomic::Ordering::Relaxed) != expected_gen { return; }

                let current_offsets = { offsets.read().await.clone() };
                
                let source_indices = if let Some(ref arc) = parent_matched_arc {
                    Some(arc.read().await.clone())
                } else {
                    None
                };

                let target_len = if let Some(ref si) = source_indices {
                    si.len()
                } else {
                    current_offsets.len().saturating_sub(1)
                };

                if target_len <= last_processed {
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    continue;
                }

                use tokio::io::{AsyncSeekExt, AsyncReadExt};

                for i in last_processed..target_len {
                    if task_generation.load(std::sync::atomic::Ordering::Relaxed) != expected_gen { return; }

                    let absolute_line = if let Some(ref si) = source_indices {
                        si[i]
                    } else {
                        i
                    };
                    
                    if absolute_line + 1 >= current_offsets.len() {
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        break;
                    }

                    let start = current_offsets[absolute_line];
                    let end = current_offsets[absolute_line + 1];
                    let bytes = end.saturating_sub(start);
                    if bytes == 0 { continue; }

                    if file.seek(std::io::SeekFrom::Start(start)).await.is_ok() {
                        let mut buf = vec![0u8; bytes as usize];
                        if file.read_exact(&mut buf).await.is_ok() {
                            let content = String::from_utf8_lossy(&buf);
                            
                            let matched = if let Some(ref r) = regex {
                                r.is_match(&content)
                            } else {
                                content.to_lowercase().contains(&query_lower)
                            };

                            if matched {
                                let mut ml = matched_lines.write().await;
                                ml.push(absolute_line);
                            }
                        }
                    }
                }

                last_processed = target_len;
            }
        });
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
