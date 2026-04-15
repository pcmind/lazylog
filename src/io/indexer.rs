use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, BufReader};
use tokio::sync::RwLock;

// Manages the line-to-byte-offset index for a file
pub struct Indexer {
    pub filepath: PathBuf,
    pub offsets: Arc<RwLock<Vec<u64>>>,
}

impl Indexer {
    pub fn new(filepath: PathBuf) -> Self {
        let offsets = Arc::new(RwLock::new(vec![0])); // File starts at offset 0
        Self { filepath, offsets }
    }

    /// Spawns a background task to index the file asynchronously
    pub fn start(&self) {
        let filepath = self.filepath.clone();
        let offsets = self.offsets.clone();

        tokio::spawn(async move {
            let file = match File::open(&filepath).await {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Error opening file context: {}", e);
                    return;
                }
            };

            let mut reader = BufReader::new(file);
            let mut buffer = [0; 64 * 1024]; // 64 KB chunk
            let mut current_offset: u64 = 0;
            let mut batch = Vec::new();

            loop {
                match reader.read(&mut buffer).await {
                    Ok(0) => {
                        // EOF reached: flush batch and sleep rather than breaking,
                        // so we can continually pick up new appended lines (tail -f)
                        if !batch.is_empty() {
                            let mut lock = offsets.write().await;
                            lock.extend_from_slice(&batch);
                            batch.clear();
                        }
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                    Ok(bytes_read) => {
                        for i in 0..bytes_read {
                            if buffer[i] == b'\n' {
                                batch.push(current_offset + i as u64 + 1);
                            }
                        }
                        current_offset += bytes_read as u64;

                        // Periodically flush the batch to the main RwLock state
                        if batch.len() >= 10000 {
                            let mut lock = offsets.write().await;
                            lock.extend_from_slice(&batch);
                            batch.clear();
                        }
                    }
                    Err(_) => {
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    }
                }
            }
        });
    }
}
