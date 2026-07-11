use std::marker::PhantomData;

use crate::ast::{
    JoinKind, JoinNode, QueryNode, SelectNode, SetOperationKind, SetOperationNode, TableNode,
};
use crate::expression::{Column, Expression, OrderDirection, Selection};
use crate::schema::{Table, TableRef};

use super::{Cte, Query};

#[derive(Clone, Debug)]
pub struct SelectQuery<T: Table, O = ()> {
    node: SelectNode,
    marker: PhantomData<fn() -> (T, O)>,
}

pub fn select_from<T: Table>() -> SelectQuery<T> {
    SelectQuery::new(TableRef::<T>::new())
}

pub fn select_from_as<Source: Table, Alias: Table>() -> SelectQuery<Alias> {
    SelectQuery::from_table(TableNode {
        name: Source::NAME,
        alias: Some(Alias::NAME),
    })
}

impl<T: Table> SelectQuery<T> {
    pub(crate) fn new(table: TableRef<T>) -> Self {
        Self::from_table(table_node(table))
    }

    fn from_table(from: TableNode) -> Self {
        Self {
            node: SelectNode {
                ctes: Vec::new(),
                from,
                selections: Vec::new(),
                joins: Vec::new(),
                filter: None,
                group_by: Vec::new(),
                having: None,
                set_operations: Vec::new(),
                order_by: Vec::new(),
                limit: None,
                offset: None,
                distinct: false,
            },
            marker: PhantomData,
        }
    }
}

impl<T: Table, O> SelectQuery<T, O> {
    pub fn select<S: Selection>(mut self, selection: S) -> SelectQuery<T, S::Output> {
        self.node.selections = selection.expressions();
        SelectQuery {
            node: self.node,
            marker: PhantomData,
        }
    }

    /// Select every column for driver-owned row access.
    ///
    /// The output is intentionally untyped because a table marker does not
    /// define a Rust record decoder. Use an explicit selection with
    /// `fetch_all_as` when typed decoding is required.
    pub fn select_all(mut self) -> SelectQuery<T, ()> {
        self.node.selections = vec![Expression::Column {
            table: T::NAME,
            name: "*",
        }];
        SelectQuery {
            node: self.node,
            marker: PhantomData,
        }
    }

    pub fn distinct(mut self) -> Self {
        self.node.distinct = true;
        self
    }

    pub fn with<C: Table>(mut self, cte: Cte<C>) -> Self {
        self.node.ctes.push(cte.node);
        self
    }

    pub fn as_cte<C: Table>(self) -> Cte<C> {
        Cte::new(self.node)
    }

    pub fn filter(mut self, expression: Expression) -> Self {
        self.node.filter = Some(match self.node.filter.take() {
            Some(existing) => existing.and(expression),
            None => expression,
        });
        self
    }

    pub fn group_by<TableType, ValueType>(mut self, column: Column<TableType, ValueType>) -> Self {
        self.node.group_by.push(column.expression());
        self
    }

    pub fn having(mut self, expression: Expression) -> Self {
        self.node.having = Some(match self.node.having.take() {
            Some(existing) => existing.and(expression),
            None => expression,
        });
        self
    }

    pub fn inner_join<J: Table>(self, on: Expression) -> Self {
        self.join::<J>(JoinKind::Inner, on)
    }

    pub fn left_join<J: Table>(self, on: Expression) -> Self {
        self.join::<J>(JoinKind::Left, on)
    }

    pub fn right_join<J: Table>(self, on: Expression) -> Self {
        self.join::<J>(JoinKind::Right, on)
    }

    pub fn full_join<J: Table>(self, on: Expression) -> Self {
        self.join::<J>(JoinKind::Full, on)
    }

    pub fn inner_join_as<Source: Table, Alias: Table>(self, on: Expression) -> Self {
        self.join_as::<Source, Alias>(JoinKind::Inner, on)
    }

    pub fn left_join_as<Source: Table, Alias: Table>(self, on: Expression) -> Self {
        self.join_as::<Source, Alias>(JoinKind::Left, on)
    }

    pub fn order_by<TableType, ValueType>(
        mut self,
        column: Column<TableType, ValueType>,
        direction: OrderDirection,
    ) -> Self {
        self.node.order_by.push((column.expression(), direction));
        self
    }

    pub fn limit(mut self, limit: u64) -> Self {
        self.node.limit = Some(limit);
        self
    }

    pub fn offset(mut self, offset: u64) -> Self {
        self.node.offset = Some(offset);
        self
    }

    pub fn union<Source: Table>(self, query: SelectQuery<Source, O>) -> Self {
        self.set_operation(SetOperationKind::Union, query)
    }

    pub fn union_all<Source: Table>(self, query: SelectQuery<Source, O>) -> Self {
        self.set_operation(SetOperationKind::UnionAll, query)
    }

    pub fn intersect<Source: Table>(self, query: SelectQuery<Source, O>) -> Self {
        self.set_operation(SetOperationKind::Intersect, query)
    }

    pub fn except<Source: Table>(self, query: SelectQuery<Source, O>) -> Self {
        self.set_operation(SetOperationKind::Except, query)
    }

    fn join<J: Table>(mut self, kind: JoinKind, on: Expression) -> Self {
        self.node.joins.push(JoinNode {
            kind,
            table: table_node(TableRef::<J>::new()),
            on,
        });
        self
    }

    fn join_as<Source: Table, Alias: Table>(mut self, kind: JoinKind, on: Expression) -> Self {
        self.node.joins.push(JoinNode {
            kind,
            table: TableNode {
                name: Source::NAME,
                alias: Some(Alias::NAME),
            },
            on,
        });
        self
    }

    fn set_operation<Source: Table>(
        mut self,
        kind: SetOperationKind,
        query: SelectQuery<Source, O>,
    ) -> Self {
        self.node.set_operations.push(SetOperationNode {
            kind,
            query: Box::new(query.node),
        });
        self
    }

    pub(crate) fn into_node(self) -> SelectNode {
        self.node
    }
}

impl<T: Table, O> Query for SelectQuery<T, O> {
    type Output = O;

    fn compile(self, dialect: &impl crate::Dialect) -> crate::Result<crate::CompiledQuery> {
        crate::compiler::compile(QueryNode::Select(Box::new(self.node)), dialect)
    }
}

fn table_node<T: Table>(table: TableRef<T>) -> TableNode {
    TableNode {
        name: table.name(),
        alias: None,
    }
}
