use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use tokio::sync::RwLock;

pub struct FilterParams {
    pub filepath: PathBuf,
    pub offsets: Arc<RwLock<Vec<u64>>>,
    pub query: String,
    pub is_regex: bool,
    pub is_negated: bool,
    pub is_case_sensitive: bool,
    pub matched_lines: Arc<RwLock<Vec<usize>>>,
    pub task_generation: Arc<AtomicUsize>,
    pub expected_gen: usize,
    pub parent_matched: Option<Arc<RwLock<Vec<usize>>>>,
    pub is_boolean: bool,
}

/// Spawns a background task that scans lines for matches and populates `matched_lines`.
pub fn spawn_filter_task(params: FilterParams) {
    let FilterParams {
        filepath,
        offsets,
        query,
        is_regex,
        is_negated,
        is_case_sensitive,
        matched_lines,
        task_generation,
        expected_gen,
        parent_matched,
        is_boolean,
    } = params;
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

        enum Matcher {
            Regex(regex::bytes::Regex),
            Boolean(crate::io::query::CompiledExpr),
        }

        impl Matcher {
            fn is_match(&self, content: &[u8]) -> bool {
                match self {
                    Matcher::Regex(r) => r.is_match(content),
                    Matcher::Boolean(e) => e.matches(content),
                }
            }
        }

        let matcher = if is_boolean {
            let q = crate::io::query::QueryExpr::parse(&query);
            let compiled = q.and_then(|expr| expr.compile(is_regex, is_case_sensitive));
            match compiled {
                Some(c) => Matcher::Boolean(c),
                None => return,
            }
        } else {
            let query_regex = if is_regex {
                regex::bytes::RegexBuilder::new(&query)
                    .case_insensitive(!is_case_sensitive)
                    .crlf(true)
                    .build()
                    .ok()
            } else {
                regex::bytes::RegexBuilder::new(&regex::escape(&query))
                    .case_insensitive(!is_case_sensitive)
                    .crlf(true)
                    .build()
                    .ok()
            };
            match query_regex {
                Some(r) => Matcher::Regex(r),
                None => return,
            }
        };

        let matcher = Arc::new(matcher);

        // Open std::fs::File for cross-platform synchronous operations in spawn_blocking
        let std_file = match std::fs::File::open(&filepath) {
            Ok(f) => f,
            Err(_) => return,
        };

        let mut last_processed = 0;
        let mut reusable_buf = Vec::new();

        loop {
            if task_generation.load(std::sync::atomic::Ordering::Relaxed) != expected_gen {
                return;
            }

            let local_last = last_processed;

            enum ChunkData {
                ParentFiltered(Vec<(usize, u64, u64)>),
                Sequential(Vec<u64>),
            }

            let (target_len, chunk_data) = {
                if let Some(ref arc) = parent_matched {
                    let p_guard = arc.read().await;
                    let t_len = p_guard.len();
                    if t_len <= local_last {
                        (t_len, ChunkData::ParentFiltered(Vec::new()))
                    } else {
                        let b_end = std::cmp::min(t_len, local_last + 100_000);
                        let p_chunk = &p_guard[local_last..b_end];
                        let mut ranges = Vec::with_capacity(p_chunk.len());
                        let o_guard = offsets.read().await;
                        for &line in p_chunk {
                            let start = o_guard.get(line).copied().unwrap_or(0);
                            let end = o_guard.get(line + 1).copied().unwrap_or(start);
                            ranges.push((line, start, end));
                        }
                        (t_len, ChunkData::ParentFiltered(ranges))
                    }
                } else {
                    let o_guard = offsets.read().await;
                    let t_len = o_guard.len().saturating_sub(1);
                    if t_len <= local_last {
                        (t_len, ChunkData::Sequential(Vec::new()))
                    } else {
                        let b_end = std::cmp::min(t_len, local_last + 100_000);
                        let needed_len = std::cmp::min(o_guard.len(), b_end + 1);
                        let slice = o_guard[local_last..needed_len].to_vec();
                        (t_len, ChunkData::Sequential(slice))
                    }
                }
            };

            if target_len <= last_processed {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                continue;
            }

            // Bound processing batch to keep the UI updating
            let batch_end = std::cmp::min(target_len, last_processed + 100_000);

            let matcher = matcher.clone();
            let mut std_file_clone = match std_file.try_clone() {
                Ok(f) => f,
                Err(_) => return,
            };

            // Run heavy work in spawn_blocking
            let expected_gen_clone = expected_gen;
            let task_gen_clone = task_generation.clone();

            let mut buf = reusable_buf;

            let batch_matches = tokio::task::spawn_blocking(move || {
                use std::io::{Read, Seek};
                let mut new_matches = Vec::new();
                let mut processed = 0;

                match chunk_data {
                    ChunkData::ParentFiltered(ranges) => {
                        for (absolute_line, start, end) in ranges {
                            if processed > 0
                                && processed % 1000 == 0
                                && task_gen_clone.load(std::sync::atomic::Ordering::Relaxed)
                                    != expected_gen_clone
                            {
                                break;
                            }

                            let bytes = end.saturating_sub(start) as usize;

                            if bytes > 0 {
                                buf.clear();
                                if std_file_clone.seek(std::io::SeekFrom::Start(start)).is_ok()
                                    && std_file_clone
                                        .by_ref()
                                        .take(bytes as u64)
                                        .read_to_end(&mut buf)
                                        .is_ok()
                                {
                                    let mut matched = matcher.is_match(&buf);

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
                    ChunkData::Sequential(chunk_offsets) => {
                        if chunk_offsets.len() < 2 {
                            return (new_matches, processed, buf);
                        }

                        let chunk_start = chunk_offsets[0];
                        let chunk_end = chunk_offsets[chunk_offsets.len() - 1];
                        let chunk_size = (chunk_end - chunk_start) as usize;

                        if chunk_size > 0 {
                            buf.clear();
                            if std_file_clone
                                .seek(std::io::SeekFrom::Start(chunk_start))
                                .is_ok()
                                && std_file_clone
                                    .by_ref()
                                    .take(chunk_size as u64)
                                    .read_to_end(&mut buf)
                                    .is_ok()
                            {
                                match &*matcher {
                                    Matcher::Regex(re) => {
                                        let mut match_it = re.find_iter(&buf).peekable();
                                        for i in local_last..batch_end {
                                            if processed > 0
                                                && processed % 1000 == 0
                                                && task_gen_clone
                                                    .load(std::sync::atomic::Ordering::Relaxed)
                                                    != expected_gen_clone
                                            {
                                                break;
                                            }
                                            let idx = i - local_last;
                                            if idx + 1 >= chunk_offsets.len() {
                                                break;
                                            }

                                            let line_start =
                                                (chunk_offsets[idx] - chunk_start) as usize;
                                            let line_end =
                                                (chunk_offsets[idx + 1] - chunk_start) as usize;

                                            let mut has_match = false;
                                            while let Some(m) = match_it.peek() {
                                                if m.end() <= line_start {
                                                    match_it.next();
                                                } else if m.start() < line_end {
                                                    has_match = true;
                                                    break;
                                                } else {
                                                    break;
                                                }
                                            }

                                            if is_negated {
                                                has_match = !has_match;
                                            }
                                            if has_match {
                                                new_matches.push(i);
                                            }
                                            processed += 1;
                                        }
                                    }
                                    Matcher::Boolean(_) => {
                                        for i in local_last..batch_end {
                                            if processed > 0
                                                && processed % 1000 == 0
                                                && task_gen_clone
                                                    .load(std::sync::atomic::Ordering::Relaxed)
                                                    != expected_gen_clone
                                            {
                                                break;
                                            }
                                            let idx = i - local_last;
                                            if idx + 1 >= chunk_offsets.len() {
                                                break;
                                            }

                                            let line_start =
                                                (chunk_offsets[idx] - chunk_start) as usize;
                                            let line_end =
                                                (chunk_offsets[idx + 1] - chunk_start) as usize;

                                            if line_end <= buf.len() {
                                                let content = &buf[line_start..line_end];
                                                let mut matched = matcher.is_match(content);
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
                        }
                    }
                }
                (new_matches, processed, buf)
            })
            .await;

            if let Ok((new_matches, processed, returned_buf)) = batch_matches {
                reusable_buf = returned_buf;
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
