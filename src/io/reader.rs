use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::RwLock;

// Exposes O(1) async file reading based on byte offsets
pub struct AsyncReader {
    filepath: PathBuf,
    offsets: Arc<RwLock<Vec<u64>>>,
    file: Option<File>,
}

impl AsyncReader {
    pub fn new(filepath: PathBuf, offsets: Arc<RwLock<Vec<u64>>>) -> Self {
        Self {
            filepath,
            offsets,
            file: None,
        }
    }

    /// Reads up to `count` lines starting from `start_line` (0-indexed)
    pub async fn read_lines(&mut self, start_line: usize, count: usize) -> Vec<String> {
        let offsets = self.offsets.read().await;

        let start_offset = match offsets.get(start_line) {
            Some(&offset) => offset,
            None => return vec![], // out of bounds
        };

        // Determine end offset based on how many lines exist ahead
        let end_index = start_line + count;
        let end_offset = match offsets.get(end_index) {
            Some(&offset) => offset,
            None => {
                // If the end_index is out of bounds, maybe we are at the end of the indexed part
                // Try to just read until the last known indexed byte or rely on EOF
                if !offsets.is_empty() {
                    *offsets.last().unwrap()
                } else {
                    return vec![];
                }
            }
        };

        let bytes_to_read = end_offset.saturating_sub(start_offset);
        if bytes_to_read == 0 {
            // Might be at EOF or indexing is paused
            // For now, let's just attempt to read up to buffer limit if it's the last line,
            // but normally the last offset is the char after the last newline.
            return vec![];
        }

        // Lazy initialize the file handle
        if self.file.is_none() {
            if let Ok(f) = File::open(&self.filepath).await {
                self.file = Some(f);
            } else {
                return vec![];
            }
        }

        let file = self.file.as_mut().unwrap();
        if file
            .seek(std::io::SeekFrom::Start(start_offset))
            .await
            .is_err()
        {
            return vec![];
        }

        let mut buffer = vec![0u8; bytes_to_read as usize];
        if file.read_exact(&mut buffer).await.is_ok() {
            let content = String::from_utf8_lossy(&buffer).to_string();
            // Split by newline and remove the last empty element if it ends in newline
            let mut lines: Vec<String> = content.split('\n').map(|s| s.to_string()).collect();
            if lines.last().map(|s| s.is_empty()).unwrap_or(false) {
                lines.pop();
            }
            return lines;
        }

        vec![]
    }

    /// Reads specific lines based on an array of 0-based line indices
    pub async fn read_specific_lines(&mut self, indices: &[usize]) -> Vec<String> {
        let mut results = Vec::new();
        // Since indices might jump around, we can seek and read per index.
        // For very large arrays this might be slow, but for a 50-line viewport, it's fast.

        if self.file.is_none() {
            if let Ok(f) = File::open(&self.filepath).await {
                self.file = Some(f);
            } else {
                return vec![];
            }
        }

        let file = self.file.as_mut().unwrap();
        let offsets = self.offsets.read().await;

        for &idx in indices {
            let start = match offsets.get(idx) {
                Some(&s) => s,
                None => {
                    results.push(String::new());
                    continue;
                }
            };

            let end = match offsets.get(idx + 1) {
                Some(&e) => e,
                None => {
                    if !offsets.is_empty() {
                        *offsets.last().unwrap()
                    } else {
                        start
                    }
                }
            };

            let bytes = end.saturating_sub(start);
            if bytes == 0 {
                results.push(String::new());
                continue;
            }

            if file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
                results.push(String::new());
                continue;
            }

            let mut buf = vec![0u8; bytes as usize];
            if file.read_exact(&mut buf).await.is_ok() {
                let content = String::from_utf8_lossy(&buf).to_string();
                results.push(content.trim_end_matches('\n').to_string());
            } else {
                results.push(String::new());
            }
        }

        results
    }
}
