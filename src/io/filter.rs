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
    is_case_sensitive: bool,
    matched_lines: Arc<RwLock<Vec<usize>>>,
    task_generation: Arc<AtomicUsize>,
    expected_gen: usize,
    parent_matched: Option<Arc<RwLock<Vec<usize>>>>,
) {
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        if task_generation.load(std::sync::atomic::Ordering::Relaxed) != expected_gen {
            return;
        }

        {
            matched_lines.write().await.clear();
        }

        if query.is_empty() {
            return;
        }

        let query_regex = if is_regex {
            regex::bytes::RegexBuilder::new(&query)
                .case_insensitive(!is_case_sensitive)
                .build()
                .ok()
        } else {
            regex::bytes::RegexBuilder::new(&regex::escape(&query))
                .case_insensitive(!is_case_sensitive)
                .build()
                .ok()
        };

        let query_regex = match query_regex {
            Some(r) => r,
            None => return,
        };

        // Open std::fs::File for cross-platform synchronous operations in spawn_blocking
        let std_file = match std::fs::File::open(&filepath) {
            Ok(f) => f,
            Err(_) => return,
        };

        let mut last_processed = 0;

        loop {
            if task_generation.load(std::sync::atomic::Ordering::Relaxed) != expected_gen {
                return;
            }

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

            // Bound processing batch to keep the UI updating
            let batch_end = std::cmp::min(target_len, last_processed + 100_000);

            let query_regex = query_regex.clone();
            let mut std_file_clone = match std_file.try_clone() {
                Ok(f) => f,
                Err(_) => return,
            };

            // Run heavy work in spawn_blocking
            let local_last = last_processed;
            let expected_gen_clone = expected_gen;
            let task_gen_clone = task_generation.clone();

            let batch_matches = tokio::task::spawn_blocking(move || {
                use std::io::{Read, Seek};
                let mut buf = Vec::new();
                let mut new_matches = Vec::new();
                let mut processed = 0;

                if source_indices.is_none() {
                    let first_line = local_last;
                    let last_line = batch_end.saturating_sub(1);
                    if first_line < current_offsets.len() {
                        let chunk_start = current_offsets[first_line];
                        let chunk_end = current_offsets
                            [std::cmp::min(last_line + 1, current_offsets.len() - 1)];
                        let chunk_size = chunk_end - chunk_start;

                        if chunk_size > 0 {
                            buf.resize(chunk_size as usize, 0);
                            if std_file_clone
                                .seek(std::io::SeekFrom::Start(chunk_start))
                                .is_ok()
                                && std_file_clone.read_exact(&mut buf).is_ok()
                            {
                                for i in local_last..batch_end {
                                    if processed > 0 && processed % 1000 == 0
                                        && task_gen_clone.load(std::sync::atomic::Ordering::Relaxed)
                                            != expected_gen_clone
                                        {
                                            break;
                                        }
                                    if i + 1 >= current_offsets.len() {
                                        break;
                                    }

                                    let line_start = (current_offsets[i] - chunk_start) as usize;
                                    let line_end = (current_offsets[i + 1] - chunk_start) as usize;

                                    if line_end <= buf.len() {
                                        let content = &buf[line_start..line_end];
                                        let mut matched = query_regex.is_match(content);
                                        if is_negated {
                                            matched = !matched;
                                        }
                                        if matched {
                                            new_matches.push(i);
                                        }
                                    }
                                    processed += 1;
                                }
                            }
                        }
                    }
                } else {
                    let si = source_indices.as_ref().unwrap();
                    for i in local_last..batch_end {
                        if processed > 0 && processed % 1000 == 0
                            && task_gen_clone.load(std::sync::atomic::Ordering::Relaxed)
                                != expected_gen_clone
                            {
                                break;
                            }

                        let absolute_line = si[i];
                        if absolute_line + 1 >= current_offsets.len() {
                            break;
                        }

                        let start = current_offsets[absolute_line];
                        let end = current_offsets[absolute_line + 1];
                        let bytes = end.saturating_sub(start);

                        if bytes > 0 {
                            buf.resize(bytes as usize, 0);
                            if std_file_clone.seek(std::io::SeekFrom::Start(start)).is_ok()
                                && std_file_clone.read_exact(&mut buf).is_ok() {
                                    let mut matched = query_regex.is_match(&buf);

                                    if is_negated {
                                        matched = !matched;
                                    }

                                    if matched {
                                        new_matches.push(absolute_line);
                                    }
                                }
                        }

                        processed += 1;
                    }
                }
                (new_matches, processed)
            })
            .await;

            if let Ok((new_matches, processed)) = batch_matches {
                if !new_matches.is_empty() {
                    let mut ml = matched_lines.write().await;
                    ml.extend(new_matches);
                }
                last_processed += processed;
                if processed == 0 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                }
            } else {
                break;
            }
        }
    });
}
