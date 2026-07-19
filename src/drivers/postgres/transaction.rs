use std::sync::Arc;

use async_trait::async_trait;

use crate::{CompiledQuery, ExecuteResult, Executor, QueryResult, Transaction};

use super::metrics::PostgresPoolMetrics;
use super::parameters::{encode, references};
use super::{PostgresError, PostgresRow};

pub struct PostgresTransaction {
    client: Option<deadpool_postgres::Client>,
    metrics: Arc<PostgresPoolMetrics>,
    completed: bool,
}

impl PostgresTransaction {
    pub(crate) fn new(
        client: deadpool_postgres::Client,
        metrics: Arc<PostgresPoolMetrics>,
    ) -> Self {
        Self {
            client: Some(client),
            metrics,
            completed: false,
        }
    }

    fn client(&self) -> Result<&deadpool_postgres::Client, PostgresError> {
        self.client.as_ref().ok_or(PostgresError::TransactionClosed)
    }

    fn database_error(&self, source: tokio_postgres::Error) -> PostgresError {
        let error = PostgresError::Database(source);
        self.metrics.record_error(error.retry_class());
        error
    }

    fn driver_error(&self, error: PostgresError) -> PostgresError {
        self.metrics.record_error(error.retry_class());
        error
    }
}

#[async_trait]
impl Executor for PostgresTransaction {
    type Row = PostgresRow;
    type Error = PostgresError;

    async fn execute(&self, query: &CompiledQuery) -> Result<ExecuteResult, Self::Error> {
        let statement = self
            .client()?
            .prepare_cached(&query.sql)
            .await
            .map_err(|source| self.database_error(source))?;
        let values = encode(&query.parameters, statement.params())
            .map_err(|error| self.driver_error(error))?;
        let parameters = references(&values);
        let rows_affected = self
            .client()?
            .execute(&statement, &parameters)
            .await
            .map_err(|source| self.database_error(source))?;
        Ok(ExecuteResult { rows_affected })
    }

    async fn fetch_all(
        &self,
        query: &CompiledQuery,
    ) -> Result<QueryResult<Self::Row>, Self::Error> {
        let statement = self
            .client()?
            .prepare_cached(&query.sql)
            .await
            .map_err(|source| self.database_error(source))?;
        let values = encode(&query.parameters, statement.params())
            .map_err(|error| self.driver_error(error))?;
        let parameters = references(&values);
        let rows = self
            .client()?
            .query(&statement, &parameters)
            .await
            .map_err(|source| self.database_error(source))?
            .into_iter()
            .map(PostgresRow::decode)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| self.driver_error(error))?;
        Ok(QueryResult { rows })
    }
}

#[async_trait]
impl Transaction for PostgresTransaction {
    async fn commit(mut self) -> Result<(), Self::Error> {
        self.client()?
            .batch_execute("COMMIT")
            .await
            .map_err(|source| self.database_error(source))?;
        self.completed = true;
        Ok(())
    }

    async fn rollback(mut self) -> Result<(), Self::Error> {
        self.client()?
            .batch_execute("ROLLBACK")
            .await
            .map_err(|source| self.database_error(source))?;
        self.completed = true;
        Ok(())
    }
}

impl Drop for PostgresTransaction {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        let Some(client) = self.client.take() else {
            return;
        };
        // An abandoned transaction must never return an open session to the
        // pool. Detach first so cancellation or the absence of a current Tokio
        // runtime closes the connection instead of making it reusable.
        let client = deadpool_postgres::Client::take(client);
        let metrics = Arc::clone(&self.metrics);
        match tokio::runtime::Handle::try_current() {
            Ok(runtime) => {
                runtime.spawn(async move {
                    if let Err(source) = client.batch_execute("ROLLBACK").await {
                        let error = PostgresError::Database(source);
                        metrics.record_error(error.retry_class());
                    }
                });
            }
            Err(_) => {
                drop(client);
            }
        }
    }
}
