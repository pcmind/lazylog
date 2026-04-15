use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::{AsyncSeekExt, AsyncReadExt};

#[tokio::main]
async fn main() {
    let mut file = File::open("/tmp/aaa.log").await.unwrap();
    let start = std::time::Instant::now();
    
    // Simulate current lazy logic:
    let mut offsets = Vec::new();
    let mut index_file = File::open("/tmp/aaa.log").await.unwrap();
    let mut buf = Vec::new();
    index_file.read_to_end(&mut buf).await.unwrap();
    let mut current = 0;
    offsets.push(0);
    for b in buf {
        if b == b'\n' { offsets.push(current + 1); }
        current+=1;
    }
    
    println!("Indexed {} lines in {:?}", offsets.len(), start.elapsed());
    
    let filter_start = std::time::Instant::now();
    let query_lower = "usable".to_string();
    let mut matched = 0;
    
    for i in 0..(offsets.len()-1) {
        let start_pos = offsets[i];
        let bytes = offsets[i+1] - start_pos;
        if bytes > 0 {
            file.seek(tokio::io::SeekFrom::Start(start_pos)).await.unwrap();
            let mut buf_run = vec![0u8; bytes as usize];
            file.read_exact(&mut buf_run).await.unwrap();
            let content = String::from_utf8_lossy(&buf_run);
            if content.to_lowercase().contains(&query_lower) {
                matched += 1;
            }
        }
    }
    println!("Filtered old way {} in {:?}", matched, filter_start.elapsed());
}
