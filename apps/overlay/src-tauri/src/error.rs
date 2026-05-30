use thiserror::Error;

#[derive(Debug, Error)]
pub enum OverlayError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("notify error: {0}")]
    Notify(#[from] notify::Error),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("tauri error: {0}")]
    Tauri(#[from] tauri::Error),
    #[error("rocket league api error: {0}")]
    RlApi(#[from] rocketstats_rlapi::RlApiError),
    #[error("invalid log line: {0}")]
    InvalidLogLine(String),
    #[error("auth is required")]
    AuthRequired,
}

pub type Result<T> = std::result::Result<T, OverlayError>;
