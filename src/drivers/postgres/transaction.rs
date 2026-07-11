use async_trait::async_trait;

use crate::{CompiledQuery, ExecuteResult, Executor, QueryResult, Transaction};

use super::parameters::{encode, references};
use super::{PostgresError, PostgresRow};

pub struct PostgresTransaction {
    client: Option<deadpool_postgres::Client>,
    completed: bool,
}

impl PostgresTransaction {
    pub(crate) fn new(client: deadpool_postgres::Client) -> Self {
        Self {
            client: Some(client),
            completed: false,
        }
    }

    fn client(&self) -> Result<&deadpool_postgres::Client, PostgresError> {
        self.client.as_ref().ok_or(PostgresError::TransactionClosed)
    }
}

#[async_trait]
impl Executor for PostgresTransaction {
    type Row = PostgresRow;
    type Error = PostgresError;

    async fn execute(&self, query: &CompiledQuery) -> Result<ExecuteResult, Self::Error> {
        let statement = self.client()?.prepare_cached(&query.sql).await?;
        let values = encode(&query.parameters, statement.params())?;
        let parameters = references(&values);
        let rows_affected = self.client()?.execute(&statement, &parameters).await?;
        Ok(ExecuteResult { rows_affected })
    }

    async fn fetch_all(
        &self,
        query: &CompiledQuery,
    ) -> Result<QueryResult<Self::Row>, Self::Error> {
        let statement = self.client()?.prepare_cached(&query.sql).await?;
        let values = encode(&query.parameters, statement.params())?;
        let parameters = references(&values);
        let rows = self
            .client()?
            .query(&statement, &parameters)
            .await?
            .into_iter()
            .map(PostgresRow::decode)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(QueryResult { rows })
    }
}

#[async_trait]
impl Transaction for PostgresTransaction {
    async fn commit(mut self) -> Result<(), Self::Error> {
        self.client()?.batch_execute("COMMIT").await?;
        self.completed = true;
        Ok(())
    }

    async fn rollback(mut self) -> Result<(), Self::Error> {
        self.client()?.batch_execute("ROLLBACK").await?;
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
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                let _client = client;
                let _ = _client.batch_execute("ROLLBACK").await;
            });
        }
    }
}
