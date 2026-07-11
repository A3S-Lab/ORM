//! Type-safe SQL query building inspired by Kysely.
//!
//! `a3s-orm` keeps schema typing, query construction, SQL compilation, and
//! execution behind separate interfaces. It does not use an Active Record
//! model and never performs implicit runtime value conversion.

mod ast;
pub mod compiler;
pub mod decode;
#[cfg(feature = "sqlite")]
pub mod drivers;
pub mod error;
pub mod executor;
pub mod expression;
pub mod query;
pub mod schema;
pub mod value;

pub use compiler::{CompiledQuery, Dialect, MysqlDialect, PostgresDialect, SqliteDialect};
pub use decode::{DecodeError, FromRow, FromValue, Row};
#[cfg(feature = "sqlite")]
pub use drivers::sqlite::{SqliteError, SqliteExecutor, SqliteRow};
pub use error::{Error, Result};
pub use executor::{
    Database, DatabaseError, ExecuteResult, Executor, QueryResult, Transaction, TransactionManager,
};
pub use expression::{Column, Expression, OrderDirection};
pub use query::{delete_from, insert_into, select_from, update_table, Query};
pub use schema::{Table, TableRef};
pub use value::{IntoSqlValue, Value};

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
