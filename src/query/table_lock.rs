use std::marker::PhantomData;

use crate::ast::{QueryNode, TableLockNode, TableNode};
use crate::schema::Table;

use super::Query;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostgresTableLockMode {
    AccessShare,
    RowShare,
    RowExclusive,
    ShareUpdateExclusive,
    Share,
    ShareRowExclusive,
    Exclusive,
    AccessExclusive,
}

#[derive(Clone, Debug)]
pub struct TableLockQuery<T: Table> {
    node: TableLockNode,
    marker: PhantomData<fn() -> T>,
}

pub fn lock_table<T: Table>(mode: PostgresTableLockMode) -> TableLockQuery<T> {
    TableLockQuery {
        node: TableLockNode {
            table: TableNode {
                name: T::NAME,
                alias: None,
            },
            mode,
            no_wait: false,
        },
        marker: PhantomData,
    }
}

impl<T: Table> TableLockQuery<T> {
    /// Fail instead of waiting for a conflicting table lock.
    pub fn no_wait(mut self) -> Self {
        self.node.no_wait = true;
        self
    }
}

impl<T: Table> Query for TableLockQuery<T> {
    type Output = ();

    fn compile(self, dialect: &impl crate::Dialect) -> crate::Result<crate::CompiledQuery> {
        crate::compiler::compile(QueryNode::TableLock(self.node), dialect)
    }
}
