use thiserror::Error;

#[derive(Error, Debug)]
pub enum LiquiModError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("junction error: {0}")]
    Junction(String),
    #[error("mod not found: {0}")]
    ModNotFound(String),
    #[error("invalid name: {0}")]
    InvalidName(String),
    #[error("unsupported archive format: {0}")]
    UnsupportedArchive(std::path::PathBuf),
    #[error("archive requires a password: {0}")]
    PasswordRequired(std::path::PathBuf),
    #[error("wrong password for archive: {0}")]
    WrongPassword(std::path::PathBuf),
    #[error("archive error in {path}: {source}")]
    Archive {
        path: std::path::PathBuf,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

pub type Result<T> = std::result::Result<T, LiquiModError>;
