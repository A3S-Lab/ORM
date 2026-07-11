use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;

use async_trait::async_trait;
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use tokio_postgres::NoTls;

use crate::{CompiledQuery, ExecuteResult, Executor, QueryResult, Transaction, TransactionManager};

use super::parameters::{encode, references};
use super::{PostgresError, PostgresRow, PostgresTransaction, PostgresTransactionError};

#[derive(Clone)]
pub struct PostgresExecutor {
    pool: Pool,
}

impl PostgresExecutor {
    pub const fn from_pool(pool: Pool) -> Self {
        Self { pool }
    }

    /// Build a non-TLS pool. Production deployments should construct a pool
    /// with their TLS connector and pass it to `from_pool`.
    pub fn connect_no_tls(url: &str, max_size: usize) -> Result<Self, PostgresError> {
        let config = tokio_postgres::Config::from_str(url).map_err(PostgresError::Configuration)?;
        let manager = Manager::from_config(
            config,
            NoTls,
            ManagerConfig {
                recycling_method: RecyclingMethod::Verified,
            },
        );
        let pool = Pool::builder(manager)
            .max_size(max_size)
            .build()
            .map_err(PostgresError::PoolBuild)?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &Pool {
        &self.pool
    }

    pub async fn transaction<T, E, F>(&self, operation: F) -> Result<T, PostgresTransactionError<E>>
    where
        T: Send,
        E: std::error::Error + Send + Sync + 'static,
        F: for<'a> FnOnce(
            &'a PostgresTransaction,
        ) -> Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'a>>,
    {
        let transaction = self
            .begin()
            .await
            .map_err(PostgresTransactionError::Begin)?;
        match operation(&transaction).await {
            Ok(value) => {
                transaction
                    .commit()
                    .await
                    .map_err(PostgresTransactionError::Commit)?;
                Ok(value)
            }
            Err(operation) => match transaction.rollback().await {
                Ok(()) => Err(PostgresTransactionError::Operation(operation)),
                Err(rollback) => Err(PostgresTransactionError::OperationAndRollback {
                    operation,
                    rollback,
                }),
            },
        }
    }
}

#[async_trait]
impl TransactionManager for PostgresExecutor {
    type Transaction = PostgresTransaction;

    async fn begin(&self) -> Result<Self::Transaction, Self::Error> {
        let client = self.pool.get().await?;
        client.batch_execute("BEGIN").await?;
        Ok(PostgresTransaction::new(client))
    }
}

#[async_trait]
impl Executor for PostgresExecutor {
    type Row = PostgresRow;
    type Error = PostgresError;

    async fn execute(&self, query: &CompiledQuery) -> Result<ExecuteResult, Self::Error> {
        let client = self.pool.get().await?;
        let statement = client.prepare_cached(&query.sql).await?;
        let values = encode(&query.parameters, statement.params())?;
        let parameters = references(&values);
        let rows_affected = client.execute(&statement, &parameters).await?;
        Ok(ExecuteResult { rows_affected })
    }

    async fn fetch_all(
        &self,
        query: &CompiledQuery,
    ) -> Result<QueryResult<Self::Row>, Self::Error> {
        let client = self.pool.get().await?;
        let statement = client.prepare_cached(&query.sql).await?;
        let values = encode(&query.parameters, statement.params())?;
        let parameters = references(&values);
        let rows = client
            .query(&statement, &parameters)
            .await?
            .into_iter()
            .map(PostgresRow::decode)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(QueryResult { rows })
    }
}
