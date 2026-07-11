use std::sync::Mutex;

use a3s_orm::{
    orm_table, select_from, CompiledQuery, Database, ExecuteResult, Executor, PostgresDialect,
    QueryResult,
};
use async_trait::async_trait;

orm_table! {
    pub struct Person => "person" {
        id: i64 => "id",
    }
}

#[derive(Debug, thiserror::Error)]
#[error("mock executor error")]
struct MockError;

#[derive(Default)]
struct MockExecutor {
    queries: Mutex<Vec<CompiledQuery>>,
}

#[async_trait]
impl Executor for MockExecutor {
    type Row = i64;
    type Error = MockError;

    async fn execute(
        &self,
        query: &CompiledQuery,
    ) -> std::result::Result<ExecuteResult, Self::Error> {
        self.queries.lock().unwrap().push(query.clone());
        Ok(ExecuteResult { rows_affected: 1 })
    }

    async fn fetch_all(
        &self,
        query: &CompiledQuery,
    ) -> std::result::Result<QueryResult<Self::Row>, Self::Error> {
        self.queries.lock().unwrap().push(query.clone());
        Ok(QueryResult { rows: vec![1] })
    }
}

#[tokio::test]
async fn database_separates_building_from_execution() {
    let database = Database::new(PostgresDialect, MockExecutor::default());
    let result = database
        .fetch_all(
            select_from::<Person>()
                .select(Person::id())
                .filter(Person::id().eq(1)),
        )
        .await
        .unwrap();
    assert_eq!(result.rows, vec![1]);
    let queries = database.executor().queries.lock().unwrap();
    assert_eq!(queries.len(), 1);
    assert_eq!(
        queries[0].sql,
        "select \"person\".\"id\" from \"person\" where (\"person\".\"id\" = $1)"
    );
}
