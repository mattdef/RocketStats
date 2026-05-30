use crate::domain::LogEvent;
use crate::error::Result;
use crate::logs::parser::parse_log_line;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader, SeekFrom};
use tokio::sync::mpsc;
use tokio::time::{Duration, sleep};

#[derive(Debug, Clone)]
pub struct LogWatcherConfig {
    pub path: PathBuf,
    pub poll_interval: Duration,
}

impl LogWatcherConfig {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            poll_interval: Duration::from_millis(250),
        }
    }
}

pub async fn watch_log(config: LogWatcherConfig, events: mpsc::Sender<LogEvent>) -> Result<()> {
    let mut offset = 0;

    loop {
        match File::open(&config.path).await {
            Ok(mut file) => {
                let metadata = file.metadata().await?;
                if metadata.len() < offset {
                    offset = 0;
                }

                file.seek(SeekFrom::Start(offset)).await?;
                let mut reader = BufReader::new(file);
                let mut line = String::new();

                loop {
                    line.clear();
                    let bytes = reader.read_line(&mut line).await?;
                    if bytes == 0 {
                        break;
                    }
                    offset += bytes as u64;
                    if let Some(event) = parse_log_line(line.trim_end())
                        && events.send(event).await.is_err()
                    {
                        return Ok(());
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }

        sleep(config.poll_interval).await;
    }
}
