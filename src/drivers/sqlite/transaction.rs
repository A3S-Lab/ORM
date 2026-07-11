use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::OwnedMutexGuard;

use crate::{ExecuteResult, Executor, QueryResult, Transaction};

use super::{SqliteError, SqliteExecutor, SqliteRow, SqliteSavepoint, SqliteSavepointError};

static NEXT_SAVEPOINT_ID: AtomicU64 = AtomicU64::new(1);

/// An exclusive SQLite transaction.
///
/// The transaction owns the connection gate, so queries issued through other
/// clones of the executor wait until this transaction commits or rolls back.
pub struct SqliteTransaction {
    executor: SqliteExecutor,
    guard: Option<OwnedMutexGuard<()>>,
    operation_lock: Arc<tokio::sync::Mutex<()>>,
    completed: bool,
}

impl SqliteTransaction {
    pub(crate) fn new(executor: SqliteExecutor, guard: OwnedMutexGuard<()>) -> Self {
        Self {
            executor,
            guard: Some(guard),
            operation_lock: Arc::new(tokio::sync::Mutex::new(())),
            completed: false,
        }
    }

    /// Run an operation in a nested savepoint.
    ///
    /// Savepoint cleanup owns the transaction operation gate, so cancellation
    /// cannot race with subsequent statements in the outer transaction.
    pub async fn savepoint<T, E, F>(&self, operation: F) -> Result<T, SqliteSavepointError<E>>
    where
        T: Send,
        E: std::error::Error + Send + Sync + 'static,
        F: for<'a> FnOnce(
            &'a SqliteSavepoint,
        ) -> Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'a>>,
    {
        let guard = self.operation_lock.clone().lock_owned().await;
        let id = NEXT_SAVEPOINT_ID.fetch_add(1, Ordering::Relaxed);
        let savepoint = SqliteSavepoint::begin(self.executor.clone(), guard, id)
            .await
            .map_err(SqliteSavepointError::Begin)?;
        match operation(&savepoint).await {
            Ok(value) => {
                savepoint
                    .release()
                    .await
                    .map_err(SqliteSavepointError::Release)?;
                Ok(value)
            }
            Err(operation) => match savepoint.rollback().await {
                Ok(()) => Err(SqliteSavepointError::Operation(operation)),
                Err(cleanup) => {
                    Err(SqliteSavepointError::OperationAndCleanup { operation, cleanup })
                }
            },
        }
    }
}

#[async_trait]
impl Executor for SqliteTransaction {
    type Row = SqliteRow;
    type Error = SqliteError;

    async fn execute(&self, query: &crate::CompiledQuery) -> Result<ExecuteResult, Self::Error> {
        let _operation = self.operation_lock.clone().lock_owned().await;
        self.executor.execute_unlocked(query).await
    }

    async fn fetch_all(
        &self,
        query: &crate::CompiledQuery,
    ) -> Result<QueryResult<Self::Row>, Self::Error> {
        let _operation = self.operation_lock.clone().lock_owned().await;
        self.executor.fetch_all_unlocked(query).await
    }
}

#[async_trait]
impl Transaction for SqliteTransaction {
    async fn commit(mut self) -> Result<(), Self::Error> {
        let _operation = self.operation_lock.lock().await;
        self.executor.execute_control("COMMIT").await?;
        self.completed = true;
        Ok(())
    }

    async fn rollback(mut self) -> Result<(), Self::Error> {
        let _operation = self.operation_lock.lock().await;
        self.executor.execute_control("ROLLBACK").await?;
        self.completed = true;
        Ok(())
    }
}

impl Drop for SqliteTransaction {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        let Some(guard) = self.guard.take() else {
            return;
        };
        let executor = self.executor.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                let _guard = guard;
                let _ = executor.execute_control("ROLLBACK").await;
            });
        }
    }
}
