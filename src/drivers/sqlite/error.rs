#[derive(Debug, thiserror::Error)]
pub enum SqliteError {
    #[error("SQLite operation failed: {0}")]
    Database(#[from] tokio_rusqlite::rusqlite::Error),
    #[error("SQLite task failed: {0}")]
    Async(#[from] tokio_rusqlite::Error),
    #[error("unsigned integer {0} is too large for SQLite")]
    UnsignedOverflow(u64),
}
