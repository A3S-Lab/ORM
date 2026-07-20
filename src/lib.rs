//! Type-safe SQL query building inspired by Kysely.
//!
//! `a3s-orm` keeps schema typing, query construction, SQL compilation, and
//! execution behind separate interfaces. It does not use an Active Record
//! model and never performs implicit runtime value conversion.

#![cfg_attr(feature = "sqlite", doc = include_str!("../README.md"))]

mod ast;
pub mod compiler;
pub mod decode;
pub mod drivers;
pub mod error;
pub mod executor;
pub mod expression;
pub mod function;
pub mod migration;
pub mod query;
pub mod schema;
pub mod value;
pub mod window;

pub use compiler::{CompiledQuery, Dialect, MysqlDialect, PostgresDialect, SqliteDialect};
pub use decode::{DecodeError, FromRow, FromValue, Row};
#[cfg(feature = "postgres")]
pub use drivers::postgres::{
    PostgresError, PostgresExecutor, PostgresIsolationLevel, PostgresMigrationError,
    PostgresMigrationOptions, PostgresOptionsError, PostgresPoolHealth,
    PostgresPoolMetricsSnapshot, PostgresPoolOptions, PostgresPoolStatus, PostgresRetryClass,
    PostgresRow, PostgresTlsError, PostgresTlsOptions, PostgresTransaction,
    PostgresTransactionAccessMode, PostgresTransactionError, PostgresTransactionOptions,
};
#[cfg(feature = "sqlite")]
pub use drivers::sqlite::{
    SqliteError, SqliteExecutor, SqliteJournalMode, SqliteMigrationError, SqliteOptions, SqliteRow,
    SqliteSavepoint, SqliteSavepointError, SqliteTransaction, SqliteTransactionError,
};
pub use error::{Error, Result};
pub use executor::{
    Database, DatabaseError, ExecuteResult, Executor, QueryResult, Transaction, TransactionManager,
};
pub use expression::{
    exists, Column, Expression, OrderDirection, SelectionExt, WindowBoundary, WindowFrame,
    WindowFrameUnits,
};
pub use function::{count, count_all, max, min, TypedExpression};
pub use migration::{
    pending_migrations, AppliedMigration, Migration, MigrationBackend, MigrationError,
    MigrationReport, Migrator, PreparedMigration,
};
pub use query::{
    delete_from, insert_into, select_from, select_from_as, sql_query, update_table, ConflictTarget,
    InsertRow, Query, SqlQuery,
};
pub use schema::{Table, TableRef};
pub use value::{IntoSqlValue, SqlArray, Value};
pub use window::{dense_rank, rank, row_number, WindowExpression};

/// Define a typed table marker and its columns.
///
/// ```
/// use a3s_orm::orm_table;
///
/// orm_table! {
///     pub struct Person => "person" {
///         id: i64 => "id",
///         name: String => "name",
///     }
/// }
/// ```
///
/// Column values are checked against the schema type:
///
/// ```compile_fail
/// use a3s_orm::{insert_into, orm_table};
///
/// orm_table! {
///     struct Person => "person" {
///         age: i32 => "age",
///     }
/// }
///
/// let _ = insert_into::<Person>().value(Person::age(), "not an integer");
/// ```
///
/// Assignments cannot use a column owned by another table:
///
/// ```compile_fail
/// use a3s_orm::{orm_table, update_table};
///
/// orm_table! { struct Person => "person" { name: String => "name" } }
/// orm_table! { struct Pet => "pet" { name: String => "name" } }
///
/// let _ = update_table::<Person>().set(Pet::name(), "wrong table");
/// ```
#[macro_export]
macro_rules! orm_table {
    (
        $(#[$table_meta:meta])*
        $visibility:vis struct $table:ident => $table_name:literal {
            $(
                $(#[$column_meta:meta])*
                $column:ident : $value:ty => $column_name:literal
            ),* $(,)?
        }
    ) => {
        $(#[$table_meta])*
        #[derive(Debug, Clone, Copy, Default)]
        $visibility struct $table;

        impl $crate::Table for $table {
            const NAME: &'static str = $table_name;
        }

        impl $table {
            $(
                $(#[$column_meta])*
                $visibility const fn $column() -> $crate::Column<$table, $value> {
                    $crate::Column::new($table_name, $column_name)
                }
            )*
        }
    };
}
