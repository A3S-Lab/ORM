use std::marker::PhantomData;

use crate::ast::{
    Assignment, ConflictAction, ConflictAssignment, ConflictNode, ConflictValue, InsertNode,
    QueryNode, TableNode,
};
use crate::expression::{Column, Selection};
use crate::schema::Table;
use crate::value::IntoSqlValue;

use super::Query;

#[derive(Clone, Debug)]
pub struct InsertRow<T: Table> {
    assignments: Vec<Assignment>,
    marker: PhantomData<fn() -> T>,
}

impl<T: Table> Default for InsertRow<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Table> InsertRow<T> {
    pub const fn new() -> Self {
        Self {
            assignments: Vec::new(),
            marker: PhantomData,
        }
    }

    pub fn value<V>(mut self, column: Column<T, V>, value: impl IntoSqlValue<V>) -> Self {
        self.assignments.push(assignment(column, value));
        self
    }
}

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
            rows: vec![Vec::new()],
            conflict: None,
            returning: Vec::new(),
        },
        marker: PhantomData,
    }
}

impl<T: Table, O> InsertQuery<T, O> {
    /// Add a value to the first row. This preserves the original single-row API.
    pub fn value<V>(mut self, column: Column<T, V>, value: impl IntoSqlValue<V>) -> Self {
        if self.node.rows.is_empty() {
            self.node.rows.push(Vec::new());
        }
        if let Some(row) = self.node.rows.first_mut() {
            row.push(assignment(column, value));
        }
        self
    }

    pub fn row(mut self, row: InsertRow<T>) -> Self {
        if self.node.rows.len() == 1 && self.node.rows.first().is_some_and(Vec::is_empty) {
            if let Some(first) = self.node.rows.first_mut() {
                *first = row.assignments;
            }
        } else {
            self.node.rows.push(row.assignments);
        }
        self
    }

    pub fn rows(mut self, rows: impl IntoIterator<Item = InsertRow<T>>) -> Self {
        for row in rows {
            self = self.row(row);
        }
        self
    }

    pub fn on_conflict<C: ConflictTarget<T>>(mut self, target: C) -> Self {
        self.node.conflict = Some(ConflictNode {
            target: target.columns(),
            action: None,
        });
        self
    }

    pub fn do_nothing(mut self) -> Self {
        self.conflict_mut().action = Some(ConflictAction::DoNothing);
        self
    }

    pub fn do_update<V>(mut self, column: Column<T, V>, value: impl IntoSqlValue<V>) -> Self {
        let assignment = ConflictAssignment {
            table: column.table_name(),
            column: column.name(),
            value: ConflictValue::Bound(value.into_sql_value()),
        };
        self.push_conflict_update(assignment);
        self
    }

    pub fn do_update_from_excluded<V>(mut self, column: Column<T, V>) -> Self {
        let assignment = ConflictAssignment {
            table: column.table_name(),
            column: column.name(),
            value: ConflictValue::Excluded {
                table: column.table_name(),
                column: column.name(),
            },
        };
        self.push_conflict_update(assignment);
        self
    }

    pub fn returning<S: Selection>(mut self, selection: S) -> InsertQuery<T, S::Output> {
        self.node.returning = selection.expressions();
        InsertQuery {
            node: self.node,
            marker: PhantomData,
        }
    }

    fn conflict_mut(&mut self) -> &mut ConflictNode {
        self.node.conflict.get_or_insert_with(|| ConflictNode {
            target: Vec::new(),
            action: None,
        })
    }

    fn push_conflict_update(&mut self, assignment: ConflictAssignment) {
        let conflict = self.conflict_mut();
        match &mut conflict.action {
            Some(ConflictAction::DoUpdate(assignments)) => assignments.push(assignment),
            _ => conflict.action = Some(ConflictAction::DoUpdate(vec![assignment])),
        }
    }
}

impl<T: Table, O> Query for InsertQuery<T, O> {
    type Output = O;

    fn compile(self, dialect: &impl crate::Dialect) -> crate::Result<crate::CompiledQuery> {
        crate::compiler::compile(QueryNode::Insert(self.node), dialect)
    }
}

pub trait ConflictTarget<T: Table> {
    fn columns(self) -> Vec<&'static str>;
}

impl<T: Table, V> ConflictTarget<T> for Column<T, V> {
    fn columns(self) -> Vec<&'static str> {
        vec![self.name()]
    }
}

macro_rules! conflict_target_tuple {
    ($($column:ident),+ $(,)?) => {
        impl<TableType: Table, $($column),+> ConflictTarget<TableType>
            for ($(Column<TableType, $column>,)+)
        {
            #[allow(non_snake_case)]
            fn columns(self) -> Vec<&'static str> {
                let ($($column,)+) = self;
                vec![$($column.name(),)+]
            }
        }
    };
}

conflict_target_tuple!(A, B);
conflict_target_tuple!(A, B, C);
conflict_target_tuple!(A, B, C, D);

fn assignment<T: Table, V>(column: Column<T, V>, value: impl IntoSqlValue<V>) -> Assignment {
    Assignment {
        table: column.table_name(),
        column: column.name(),
        value: value.into_sql_value(),
    }
}
