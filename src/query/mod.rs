mod delete;
mod insert;
mod select;
mod update;

pub use delete::{delete_from, DeleteQuery};
pub use insert::{insert_into, InsertQuery};
pub use select::{select_from, SelectQuery};
pub use update::{update_table, UpdateQuery};

use crate::compiler::{CompiledQuery, Dialect};
use crate::Result;

pub trait Query {
    type Output;

    fn compile(self, dialect: &impl Dialect) -> Result<CompiledQuery>;
}
