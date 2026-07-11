use std::marker::PhantomData;

use crate::ast::{Assignment, QueryNode, TableNode, UpdateNode};
use crate::expression::{Column, Expression, Selection};
use crate::schema::Table;
use crate::value::IntoSqlValue;

use super::Query;

#[derive(Clone, Debug)]
pub struct UpdateQuery<T: Table, O = ()> {
    node: UpdateNode,
    marker: PhantomData<fn() -> (T, O)>,
}

pub fn update_table<T: Table>() -> UpdateQuery<T> {
    UpdateQuery {
        node: UpdateNode {
            table: TableNode {
                name: T::NAME,
                alias: None,
            },
            assignments: Vec::new(),
            filter: None,
            returning: Vec::new(),
        },
        marker: PhantomData,
    }
}

impl<T: Table, O> UpdateQuery<T, O> {
    pub fn set<V>(mut self, column: Column<T, V>, value: impl IntoSqlValue<V>) -> Self {
        self.node.assignments.push(Assignment {
            table: column.table_name(),
            column: column.name(),
            value: value.into_sql_value(),
        });
        self
    }

    pub fn filter(mut self, expression: Expression) -> Self {
        self.node.filter = Some(match self.node.filter.take() {
            Some(existing) => existing.and(expression),
            None => expression,
        });
        self
    }

    pub fn returning<S: Selection>(mut self, selection: S) -> UpdateQuery<T, S::Output> {
        self.node.returning.extend(selection.expressions());
        UpdateQuery {
            node: self.node,
            marker: PhantomData,
        }
    }
}

impl<T: Table, O> Query for UpdateQuery<T, O> {
    type Output = O;

    fn compile(self, dialect: &impl crate::Dialect) -> crate::Result<crate::CompiledQuery> {
        crate::compiler::compile(QueryNode::Update(self.node), dialect)
    }
}
