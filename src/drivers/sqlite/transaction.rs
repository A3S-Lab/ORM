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
    _guard: OwnedMutexGuard<()>,
}

impl SqliteTransaction {
    pub(crate) fn new(executor: SqliteExecutor, guard: OwnedMutexGuard<()>) -> Self {
        Self {
            executor,
            _guard: guard,
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
    async fn commit(self) -> Result<(), Self::Error> {
        self.executor.execute_control("COMMIT").await
    }

    async fn rollback(self) -> Result<(), Self::Error> {
        self.executor.execute_control("ROLLBACK").await
    }
}
