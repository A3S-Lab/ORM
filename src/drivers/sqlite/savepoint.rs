use async_trait::async_trait;
use tokio::sync::OwnedMutexGuard;

use crate::{ExecuteResult, Executor, QueryResult};

use super::{SqliteError, SqliteExecutor, SqliteRow};

pub struct SqliteSavepoint {
    executor: SqliteExecutor,
    name: String,
    guard: Option<OwnedMutexGuard<()>>,
    completed: bool,
}

impl SqliteSavepoint {
    pub(crate) async fn begin(
        executor: SqliteExecutor,
        guard: OwnedMutexGuard<()>,
        id: u64,
    ) -> Result<Self, SqliteError> {
        let name = format!("a3s_sp_{id}");
        executor
            .execute_control(format!("SAVEPOINT \"{name}\""))
            .await?;
        Ok(Self {
            executor,
            name,
            guard: Some(guard),
            completed: false,
        })
    }

    pub(crate) async fn release(mut self) -> Result<(), SqliteError> {
        self.executor
            .execute_control(format!("RELEASE SAVEPOINT \"{}\"", self.name))
            .await?;
        self.completed = true;
        Ok(())
    }

    pub(crate) async fn rollback(mut self) -> Result<(), SqliteError> {
        self.executor
            .execute_control(cleanup_sql(&self.name))
            .await?;
        self.completed = true;
        Ok(())
    }
}

#[async_trait]
impl Executor for SqliteSavepoint {
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

impl Drop for SqliteSavepoint {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        let Some(guard) = self.guard.take() else {
            return;
        };
        let executor = self.executor.clone();
        let sql = cleanup_sql(&self.name);
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                let _guard = guard;
                let _ = executor.execute_control(sql).await;
            });
        }
    }
}

fn cleanup_sql(name: &str) -> String {
    format!("ROLLBACK TO SAVEPOINT \"{name}\"; RELEASE SAVEPOINT \"{name}\"")
}
