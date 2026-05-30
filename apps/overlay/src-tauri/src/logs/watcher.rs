use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct LogWatcherConfig {
    pub path: PathBuf,
}
