use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use tokio_rusqlite::rusqlite;
use tokio_rusqlite::rusqlite::types::{Value as SqliteValue, ValueRef};

use crate::{
    CompiledQuery, ExecuteResult, Executor, QueryResult, Transaction, TransactionManager, Value,
};

use super::{SqliteError, SqliteRow, SqliteTransaction, SqliteTransactionError};

#[derive(Clone)]
pub struct SqliteExecutor {
    connection: tokio_rusqlite::Connection,
    transaction_lock: Arc<tokio::sync::Mutex<()>>,
}

impl SqliteExecutor {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, SqliteError> {
        Ok(Self {
            connection: tokio_rusqlite::Connection::open(path).await?,
            transaction_lock: Arc::new(tokio::sync::Mutex::new(())),
        })
    }

    pub async fn open_in_memory() -> Result<Self, SqliteError> {
        Ok(Self {
            connection: tokio_rusqlite::Connection::open_in_memory().await?,
            transaction_lock: Arc::new(tokio::sync::Mutex::new(())),
        })
    }

    pub fn connection(&self) -> &tokio_rusqlite::Connection {
        &self.connection
    }

    /// Run an operation inside a transaction and always complete it.
    ///
    /// The operation is committed on success and rolled back on error. If the
    /// calling task is cancelled while the operation is running, dropping the
    /// transaction schedules a rollback while retaining the connection gate.
    pub async fn transaction<T, E, F>(&self, operation: F) -> Result<T, SqliteTransactionError<E>>
    where
        T: Send,
        E: std::error::Error + Send + Sync + 'static,
        F: for<'a> FnOnce(
            &'a SqliteTransaction,
        ) -> Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'a>>,
    {
        let transaction = self.begin().await.map_err(SqliteTransactionError::Begin)?;
        match operation(&transaction).await {
            Ok(value) => {
                transaction
                    .commit()
                    .await
                    .map_err(SqliteTransactionError::Commit)?;
                Ok(value)
            }
            Err(operation) => match transaction.rollback().await {
                Ok(()) => Err(SqliteTransactionError::Operation(operation)),
                Err(rollback) => Err(SqliteTransactionError::OperationAndRollback {
                    operation,
                    rollback,
                }),
            },
        }
    }

    /// Execute trusted schema SQL. Application values should use typed queries.
    pub async fn execute_schema(&self, sql: impl Into<String>) -> Result<(), SqliteError> {
        let _guard = self.transaction_lock.lock().await;
        let sql = sql.into();
        self.connection
            .call(move |connection| connection.execute_batch(&sql))
            .await?;
        Ok(())
    }

    pub(crate) async fn execute_unlocked(
        &self,
        query: &CompiledQuery,
    ) -> Result<ExecuteResult, SqliteError> {
        let sql = query.sql.clone();
        let parameters = sqlite_parameters(&query.parameters)?;
        let rows_affected = self
            .connection
            .call(move |connection| {
                connection.execute(&sql, rusqlite::params_from_iter(parameters))
            })
            .await?;
        Ok(ExecuteResult {
            rows_affected: rows_affected as u64,
        })
    }

    pub(crate) async fn fetch_all_unlocked(
        &self,
        query: &CompiledQuery,
    ) -> Result<QueryResult<SqliteRow>, SqliteError> {
        let sql = query.sql.clone();
        let parameters = sqlite_parameters(&query.parameters)?;
        let rows = self
            .connection
            .call(move |connection| {
                let mut statement = connection.prepare(&sql)?;
                let column_count = statement.column_count();
                let mut cursor = statement.query(rusqlite::params_from_iter(parameters))?;
                let mut rows = Vec::new();
                while let Some(row) = cursor.next()? {
                    let mut values = Vec::with_capacity(column_count);
                    for index in 0..column_count {
                        values.push(value_from_ref(row.get_ref(index)?)?);
                    }
                    rows.push(SqliteRow::new(values));
                }
                Ok(rows)
            })
            .await?;
        Ok(QueryResult { rows })
    }

    pub(crate) async fn execute_control(&self, sql: &'static str) -> Result<(), SqliteError> {
        self.connection
            .call(move |connection| connection.execute_batch(sql))
            .await?;
        Ok(())
    }
}

#[async_trait]
impl Executor for SqliteExecutor {
    type Row = SqliteRow;
    type Error = SqliteError;

    async fn execute(&self, query: &CompiledQuery) -> Result<ExecuteResult, Self::Error> {
        let _guard = self.transaction_lock.lock().await;
        self.execute_unlocked(query).await
    }

    async fn fetch_all(
        &self,
        query: &CompiledQuery,
    ) -> Result<QueryResult<Self::Row>, Self::Error> {
        let _guard = self.transaction_lock.lock().await;
        self.fetch_all_unlocked(query).await
    }
}

#[async_trait]
impl TransactionManager for SqliteExecutor {
    type Transaction = SqliteTransaction;

    async fn begin(&self) -> Result<Self::Transaction, Self::Error> {
        let guard = self.transaction_lock.clone().lock_owned().await;
        self.execute_control("BEGIN IMMEDIATE").await?;
        Ok(SqliteTransaction::new(self.clone(), guard))
    }
}

fn sqlite_parameters(values: &[Value]) -> Result<Vec<SqliteValue>, SqliteError> {
    values.iter().map(value_to_sqlite).collect()
}

fn value_to_sqlite(value: &Value) -> Result<SqliteValue, SqliteError> {
    Ok(match value {
        Value::Null => SqliteValue::Null,
        Value::Bool(value) => SqliteValue::Integer(i64::from(*value)),
        Value::I64(value) => SqliteValue::Integer(*value),
        Value::U64(value) => SqliteValue::Integer(
            i64::try_from(*value).map_err(|_| SqliteError::UnsignedOverflow(*value))?,
        ),
        Value::F64(value) => SqliteValue::Real(*value),
        Value::String(value) => SqliteValue::Text(value.clone()),
        Value::Bytes(value) => SqliteValue::Blob(value.clone()),
    })
}

fn value_from_ref(value: ValueRef<'_>) -> rusqlite::Result<Value> {
    Ok(match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(value) => Value::I64(value),
        ValueRef::Real(value) => Value::F64(value),
        ValueRef::Text(value) => Value::String(String::from_utf8_lossy(value).into_owned()),
        ValueRef::Blob(value) => Value::Bytes(value.to_vec()),
    })
}
