use std::marker::PhantomData;

use crate::ast::{DeleteNode, QueryNode, TableNode};
use crate::expression::{Expression, Selection};
use crate::schema::Table;

use super::Query;

#[derive(Clone, Debug)]
pub struct DeleteQuery<T: Table, O = ()> {
    node: DeleteNode,
    marker: PhantomData<fn() -> (T, O)>,
}

pub fn delete_from<T: Table>() -> DeleteQuery<T> {
    DeleteQuery {
        node: DeleteNode {
            table: TableNode {
                name: T::NAME,
                alias: None,
            },
            filter: None,
            returning: Vec::new(),
        },
        marker: PhantomData,
    }
}

impl<T: Table, O> DeleteQuery<T, O> {
    pub fn filter(mut self, expression: Expression) -> Self {
        self.node.filter = Some(match self.node.filter.take() {
            Some(existing) => existing.and(expression),
            None => expression,
        });
        self
    }

    pub fn returning<S: Selection>(mut self, selection: S) -> DeleteQuery<T, S::Output> {
        self.node.returning.extend(selection.expressions());
        DeleteQuery {
            node: self.node,
            marker: PhantomData,
        }
    }
}

impl<T: Table, O> Query for DeleteQuery<T, O> {
    type Output = O;

    fn compile(self, dialect: &impl crate::Dialect) -> crate::Result<crate::CompiledQuery> {
        crate::compiler::compile(QueryNode::Delete(self.node), dialect)
    }
}
