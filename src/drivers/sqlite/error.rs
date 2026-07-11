#[derive(Debug, thiserror::Error)]
pub enum SqliteError {
    #[error("SQLite operation failed: {0}")]
    Database(#[from] tokio_rusqlite::rusqlite::Error),
    #[error("SQLite task failed: {0}")]
    Async(#[from] tokio_rusqlite::Error),
    #[error("unsigned integer {0} is too large for SQLite")]
    UnsignedOverflow(u64),
}

#[derive(Debug, thiserror::Error)]
pub enum SqliteTransactionError<E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    #[error("could not start SQLite transaction: {0}")]
    Begin(#[source] SqliteError),
    #[error("transaction operation failed: {0}")]
    Operation(#[source] E),
    #[error("could not commit SQLite transaction: {0}")]
    Commit(#[source] SqliteError),
    #[error("transaction operation failed ({operation}) and rollback failed ({rollback})")]
    OperationAndRollback { operation: E, rollback: SqliteError },
}
