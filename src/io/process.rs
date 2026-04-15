use tokio::process::Command;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use crate::config::Config;

/// Applies external transformers to a log line if it matches any configured patterns.
pub async fn apply_transformers(line: String, config: &Config) -> String {
    for t in &config.transformers {
        let matches = if let Some(ref re) = t.regex {
            re.is_match(&line)
        } else if let Some(ref sub) = t.substring {
            line.contains(sub)
        } else {
            false
        };

        if matches {
            match run_command(&t.command, &line).await {
                Ok(output) if !output.trim().is_empty() => return output,
                _ => continue, // Try next transformer or fall back to original
            }
        }
    }
    line
}

async fn run_command(cmd_str: &str, input: &str) -> tokio::io::Result<String> {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(cmd_str)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().ok_or_else(|| {
        tokio::io::Error::new(tokio::io::ErrorKind::Other, "Failed to open stdin")
    })?;

    let input_bytes = input.as_bytes().to_vec();
    
    // Write input in a separate task to avoid deadlock if pipe is full
    tokio::spawn(async move {
        let _ = stdin.write_all(&input_bytes).await;
        let _ = stdin.flush().await;
        drop(stdin);
    });

    let output = child.wait_with_output().await?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(tokio::io::Error::new(
            tokio::io::ErrorKind::Other,
            format!("Command failed with status: {}", output.status),
        ))
    }
}
