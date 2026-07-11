use std::marker::PhantomData;

use crate::ast::{Assignment, InsertNode, QueryNode, TableNode};
use crate::expression::{Column, Selection};
use crate::schema::Table;
use crate::value::IntoSqlValue;

use super::Query;

#[derive(Clone, Debug)]
pub struct InsertQuery<T: Table, O = ()> {
    node: InsertNode,
    marker: PhantomData<fn() -> (T, O)>,
}

pub fn insert_into<T: Table>() -> InsertQuery<T> {
    InsertQuery {
        node: InsertNode {
            table: TableNode {
                name: T::NAME,
                alias: None,
            },
            assignments: Vec::new(),
            returning: Vec::new(),
        },
        marker: PhantomData,
    }
}

impl<T: Table, O> InsertQuery<T, O> {
    pub fn value<V>(mut self, column: Column<T, V>, value: impl IntoSqlValue<V>) -> Self {
        self.node.assignments.push(Assignment {
            table: column.table_name(),
            column: column.name(),
            value: value.into_sql_value(),
        });
        self
    }

    pub fn returning<S: Selection>(mut self, selection: S) -> InsertQuery<T, S::Output> {
        self.node.returning.extend(selection.expressions());
        InsertQuery {
            node: self.node,
            marker: PhantomData,
        }
    }
}

impl<T: Table, O> Query for InsertQuery<T, O> {
    type Output = O;

    fn compile(self, dialect: &impl crate::Dialect) -> crate::Result<crate::CompiledQuery> {
        crate::compiler::compile(QueryNode::Insert(self.node), dialect)
    }
}
