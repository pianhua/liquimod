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
}

pub type Result<T> = std::result::Result<T, LiquiModError>;
