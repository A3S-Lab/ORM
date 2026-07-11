mod cte;
mod delete;
mod insert;
mod raw;
mod select;
mod update;

pub use cte::Cte;
pub use delete::{delete_from, DeleteQuery};
pub use insert::{insert_into, ConflictTarget, InsertQuery, InsertRow};
pub use raw::{sql_query, SqlQuery};
pub use select::{select_from, select_from_as, SelectQuery};
pub use update::{update_table, UpdateQuery};

use crate::compiler::{CompiledQuery, Dialect};
use crate::Result;

pub trait Query {
    type Output;

    fn compile(self, dialect: &impl Dialect) -> Result<CompiledQuery>;
}
