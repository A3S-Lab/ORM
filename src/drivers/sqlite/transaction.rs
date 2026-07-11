use async_trait::async_trait;
use tokio::sync::OwnedMutexGuard;

use crate::{ExecuteResult, Executor, QueryResult, Transaction};

use super::{SqliteError, SqliteExecutor, SqliteRow};

/// An exclusive SQLite transaction.
///
/// The transaction owns the connection gate, so queries issued through other
/// clones of the executor wait until this transaction commits or rolls back.
pub struct SqliteTransaction {
    executor: SqliteExecutor,
    guard: Option<OwnedMutexGuard<()>>,
    completed: bool,
}

impl SqliteTransaction {
    pub(crate) fn new(executor: SqliteExecutor, guard: OwnedMutexGuard<()>) -> Self {
        Self {
            executor,
            guard: Some(guard),
            completed: false,
        }
    }
}

#[async_trait]
impl Executor for SqliteTransaction {
    type Row = SqliteRow;
    type Error = SqliteError;

    async fn execute(&self, query: &crate::CompiledQuery) -> Result<ExecuteResult, Self::Error> {
        self.executor.execute_unlocked(query).await
    }

    async fn fetch_all(
        &self,
        query: &crate::CompiledQuery,
    ) -> Result<QueryResult<Self::Row>, Self::Error> {
        self.executor.fetch_all_unlocked(query).await
    }
}

#[async_trait]
impl Transaction for SqliteTransaction {
    async fn commit(mut self) -> Result<(), Self::Error> {
        self.executor.execute_control("COMMIT").await?;
        self.completed = true;
        Ok(())
    }

    async fn rollback(mut self) -> Result<(), Self::Error> {
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
