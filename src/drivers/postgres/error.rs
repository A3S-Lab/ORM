#[derive(Debug, thiserror::Error)]
pub enum PostgresError {
    #[error("could not get PostgreSQL connection from pool: {0}")]
    Pool(#[from] deadpool_postgres::PoolError),
    #[error("PostgreSQL operation failed: {0}")]
    Database(#[from] tokio_postgres::Error),
    #[error("invalid PostgreSQL connection string: {0}")]
    Configuration(#[source] tokio_postgres::Error),
    #[error("could not build PostgreSQL connection pool: {0}")]
    PoolBuild(#[source] deadpool_postgres::BuildError),
    #[error("parameter value {value} is outside the range of PostgreSQL {target}")]
    IntegerOverflow { value: String, target: &'static str },
    #[error("query has {values} values but PostgreSQL inferred {parameters} parameters")]
    ParameterCount { values: usize, parameters: usize },
    #[error("PostgreSQL returned unsupported column type {0}")]
    UnsupportedType(String),
    #[error("PostgreSQL transaction no longer owns its pooled connection")]
    TransactionClosed,
}

#[derive(Debug, thiserror::Error)]
pub enum PostgresTransactionError<E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    #[error("could not start PostgreSQL transaction: {0}")]
    Begin(#[source] PostgresError),
    #[error("transaction operation failed: {0}")]
    Operation(#[source] E),
    #[error("could not commit PostgreSQL transaction: {0}")]
    Commit(#[source] PostgresError),
    #[error("transaction operation failed ({operation}) and rollback failed ({rollback})")]
    OperationAndRollback {
        operation: E,
        rollback: PostgresError,
    },
}
