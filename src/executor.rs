use std::marker::PhantomData;

use async_trait::async_trait;

use crate::compiler::{CompiledQuery, Dialect};
use crate::decode::{FromRow, Row};
use crate::query::Query;
use crate::Result;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExecuteResult {
    pub rows_affected: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct QueryResult<Row> {
    pub rows: Vec<Row>,
}

#[async_trait]
pub trait Executor: Send + Sync {
    type Row: Send;
    type Error: std::error::Error + Send + Sync + 'static;

    async fn execute(
        &self,
        query: &CompiledQuery,
    ) -> std::result::Result<ExecuteResult, Self::Error>;

    async fn fetch_all(
        &self,
        query: &CompiledQuery,
    ) -> std::result::Result<QueryResult<Self::Row>, Self::Error>;
}

#[async_trait]
pub trait Transaction: Executor + Sized {
    async fn commit(self) -> std::result::Result<(), Self::Error>;
    async fn rollback(self) -> std::result::Result<(), Self::Error>;
}

#[async_trait]
pub trait TransactionManager: Executor {
    type Transaction: Transaction<Row = Self::Row, Error = Self::Error>;

    async fn begin(&self) -> std::result::Result<Self::Transaction, Self::Error>;
}

#[derive(Debug)]
pub struct Database<D, E> {
    dialect: D,
    executor: E,
    marker: PhantomData<fn()>,
}

impl<D, E> Database<D, E>
where
    D: Dialect,
    E: Executor,
{
    pub const fn new(dialect: D, executor: E) -> Self {
        Self {
            dialect,
            executor,
            marker: PhantomData,
        }
    }

    pub fn dialect(&self) -> &D {
        &self.dialect
    }

    pub fn executor(&self) -> &E {
        &self.executor
    }

    pub fn compile<Q: Query>(&self, query: Q) -> Result<CompiledQuery> {
        query.compile(&self.dialect)
    }

    pub async fn execute<Q: Query>(
        &self,
        query: Q,
    ) -> std::result::Result<ExecuteResult, DatabaseError<E::Error>> {
        let query = self.compile(query).map_err(DatabaseError::Build)?;
        self.executor
            .execute(&query)
            .await
            .map_err(DatabaseError::Execute)
    }

    pub async fn fetch_all<Q: Query>(
        &self,
        query: Q,
    ) -> std::result::Result<QueryResult<E::Row>, DatabaseError<E::Error>> {
        let query = self.compile(query).map_err(DatabaseError::Build)?;
        self.executor
            .fetch_all(&query)
            .await
            .map_err(DatabaseError::Execute)
    }

    pub async fn fetch_optional<Q: Query>(
        &self,
        query: Q,
    ) -> std::result::Result<Option<E::Row>, DatabaseError<E::Error>> {
        let result = self.fetch_all(query).await?;
        exactly_optional(result.rows)
    }

    pub async fn fetch_one<Q: Query>(
        &self,
        query: Q,
    ) -> std::result::Result<E::Row, DatabaseError<E::Error>> {
        self.fetch_optional(query)
            .await?
            .ok_or(DatabaseError::NoRows)
    }

    pub async fn fetch_all_as<Q>(
        &self,
        query: Q,
    ) -> std::result::Result<QueryResult<Q::Output>, DatabaseError<E::Error>>
    where
        Q: Query,
        Q::Output: FromRow,
        E::Row: Row,
    {
        let result = self.fetch_all(query).await?;
        let rows = result
            .rows
            .iter()
            .map(Q::Output::from_row)
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(DatabaseError::Decode)?;
        Ok(QueryResult { rows })
    }

    pub async fn fetch_optional_as<Q>(
        &self,
        query: Q,
    ) -> std::result::Result<Option<Q::Output>, DatabaseError<E::Error>>
    where
        Q: Query,
        Q::Output: FromRow,
        E::Row: Row,
    {
        let result = self.fetch_all_as(query).await?;
        exactly_optional(result.rows)
    }

    pub async fn fetch_one_as<Q>(
        &self,
        query: Q,
    ) -> std::result::Result<Q::Output, DatabaseError<E::Error>>
    where
        Q: Query,
        Q::Output: FromRow,
        E::Row: Row,
    {
        self.fetch_optional_as(query)
            .await?
            .ok_or(DatabaseError::NoRows)
    }

    pub fn into_parts(self) -> (D, E) {
        (self.dialect, self.executor)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError<E>
where
    E: std::error::Error + 'static,
{
    #[error(transparent)]
    Build(#[from] crate::Error),
    #[error("database execution failed: {0}")]
    Execute(E),
    #[error("database row decoding failed: {0}")]
    Decode(#[from] crate::DecodeError),
    #[error("query returned no rows")]
    NoRows,
    #[error("query returned {actual} rows where at most one was expected")]
    TooManyRows { actual: usize },
}

fn exactly_optional<Row, E>(rows: Vec<Row>) -> std::result::Result<Option<Row>, DatabaseError<E>>
where
    E: std::error::Error + Send + Sync + 'static,
{
    match rows.len() {
        0 => Ok(None),
        1 => Ok(rows.into_iter().next()),
        actual => Err(DatabaseError::TooManyRows { actual }),
    }
}
