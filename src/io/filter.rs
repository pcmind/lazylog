use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use tokio::sync::RwLock;

/// Spawns a background task that scans lines for matches and populates `matched_lines`.
pub fn spawn_filter_task(
    filepath: PathBuf,
    offsets: Arc<RwLock<Vec<u64>>>,
    query: String,
    is_regex: bool,
    is_negated: bool,
    matched_lines: Arc<RwLock<Vec<usize>>>,
    task_generation: Arc<AtomicUsize>,
    expected_gen: usize,
    parent_matched: Option<Arc<RwLock<Vec<usize>>>>,
) {
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        if task_generation.load(std::sync::atomic::Ordering::Relaxed) != expected_gen { return; }

        { matched_lines.write().await.clear(); }

        let regex = if is_regex { regex::Regex::new(&query).ok() } else { None };
        if query.is_empty() {
            return;
        }
        let query_lower = query.to_lowercase();

        let mut file = match tokio::fs::File::open(&filepath).await {
            Ok(f) => f,
            Err(_) => return,
        };

        let mut last_processed = 0;

        loop {
            if task_generation.load(std::sync::atomic::Ordering::Relaxed) != expected_gen { return; }

            let current_offsets = { offsets.read().await.clone() };

            let source_indices = if let Some(ref arc) = parent_matched {
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

            let mut processed = 0;
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
                
                if bytes > 0 {
                    if file.seek(std::io::SeekFrom::Start(start)).await.is_ok() {
                        let mut buf = vec![0u8; bytes as usize];
                        if file.read_exact(&mut buf).await.is_ok() {
                            let content = String::from_utf8_lossy(&buf);

                            let mut matched = if let Some(ref r) = regex {
                                r.is_match(&content)
                            } else {
                                content.to_lowercase().contains(&query_lower)
                            };

                            if is_negated {
                                matched = !matched;
                            }

                            if matched {
                                let mut ml = matched_lines.write().await;
                                ml.push(absolute_line);
                            }
                        }
                    }
                }
                
                processed += 1;
            }

            last_processed += processed;
        }
    });
}
