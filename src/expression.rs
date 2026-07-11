use std::marker::PhantomData;

use crate::value::{IntoSqlValue, Value};

#[derive(Clone, Debug)]
pub struct SelectSubquery(pub(crate) Box<crate::ast::SelectNode>);

#[derive(Clone, Debug)]
pub enum Expression {
    Column {
        table: &'static str,
        name: &'static str,
    },
    Value(Value),
    Subquery(SelectSubquery),
    Function {
        name: &'static str,
        arguments: Vec<Expression>,
    },
    Alias {
        expression: Box<Expression>,
        alias: &'static str,
    },
    Wildcard,
    Window {
        expression: Box<Expression>,
        partition_by: Vec<Expression>,
        order_by: Vec<(Expression, OrderDirection)>,
        frame: Option<WindowFrame>,
    },
    Binary {
        left: Box<Expression>,
        operator: BinaryOperator,
        right: Box<Expression>,
    },
    Unary {
        operator: UnaryOperator,
        expression: Box<Expression>,
    },
    And(Vec<Expression>),
    Or(Vec<Expression>),
}

impl Expression {
    pub fn and(self, other: Expression) -> Self {
        match self {
            Self::And(mut expressions) => {
                expressions.push(other);
                Self::And(expressions)
            }
            expression => Self::And(vec![expression, other]),
        }
    }

    pub fn or(self, other: Expression) -> Self {
        match self {
            Self::Or(mut expressions) => {
                expressions.push(other);
                Self::Or(expressions)
            }
            expression => Self::Or(vec![expression, other]),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum BinaryOperator {
    Eq,
    NotEq,
    GreaterThan,
    GreaterThanOrEq,
    LessThan,
    LessThanOrEq,
    Like,
    In,
    Is,
    IsNot,
}

#[derive(Clone, Copy, Debug)]
pub enum UnaryOperator {
    IsNull,
    IsNotNull,
    Not,
    Exists,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderDirection {
    Asc,
    Desc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowFrameUnits {
    Rows,
    Range,
    Groups,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowBoundary {
    UnboundedPreceding,
    Preceding(u64),
    CurrentRow,
    Following(u64),
    UnboundedFollowing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowFrame {
    pub units: WindowFrameUnits,
    pub start: WindowBoundary,
    pub end: WindowBoundary,
}

#[derive(Debug)]
pub struct Column<T, V> {
    table: &'static str,
    name: &'static str,
    marker: PhantomData<fn() -> (T, V)>,
}

impl<T, V> Clone for Column<T, V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T, V> Copy for Column<T, V> {}

impl<T, V> Column<T, V> {
    pub const fn new(table: &'static str, name: &'static str) -> Self {
        Self {
            table,
            name,
            marker: PhantomData,
        }
    }

    pub const fn table_name(self) -> &'static str {
        self.table
    }

    pub const fn name(self) -> &'static str {
        self.name
    }

    pub fn expression(self) -> Expression {
        Expression::Column {
            table: self.table,
            name: self.name,
        }
    }

    pub fn eq(self, value: impl IntoSqlValue<V>) -> Expression {
        self.compare(
            BinaryOperator::Eq,
            Expression::Value(value.into_sql_value()),
        )
    }

    pub fn ne(self, value: impl IntoSqlValue<V>) -> Expression {
        self.compare(
            BinaryOperator::NotEq,
            Expression::Value(value.into_sql_value()),
        )
    }

    pub fn gt(self, value: impl IntoSqlValue<V>) -> Expression {
        self.compare(
            BinaryOperator::GreaterThan,
            Expression::Value(value.into_sql_value()),
        )
    }

    pub fn gte(self, value: impl IntoSqlValue<V>) -> Expression {
        self.compare(
            BinaryOperator::GreaterThanOrEq,
            Expression::Value(value.into_sql_value()),
        )
    }

    pub fn lt(self, value: impl IntoSqlValue<V>) -> Expression {
        self.compare(
            BinaryOperator::LessThan,
            Expression::Value(value.into_sql_value()),
        )
    }

    pub fn lte(self, value: impl IntoSqlValue<V>) -> Expression {
        self.compare(
            BinaryOperator::LessThanOrEq,
            Expression::Value(value.into_sql_value()),
        )
    }

    pub fn like(self, value: impl IntoSqlValue<V>) -> Expression {
        self.compare(
            BinaryOperator::Like,
            Expression::Value(value.into_sql_value()),
        )
    }

    pub fn eq_column<OtherTable>(self, other: Column<OtherTable, V>) -> Expression {
        self.compare(BinaryOperator::Eq, other.expression())
    }

    pub fn eq_subquery<Source: crate::Table>(
        self,
        query: crate::query::SelectQuery<Source, V>,
    ) -> Expression {
        self.compare(
            BinaryOperator::Eq,
            Expression::Subquery(SelectSubquery(Box::new(query.into_node()))),
        )
    }

    pub fn in_subquery<Source: crate::Table>(
        self,
        query: crate::query::SelectQuery<Source, V>,
    ) -> Expression {
        self.compare(
            BinaryOperator::In,
            Expression::Subquery(SelectSubquery(Box::new(query.into_node()))),
        )
    }

    pub fn is_null(self) -> Expression {
        Expression::Unary {
            operator: UnaryOperator::IsNull,
            expression: Box::new(self.expression()),
        }
    }

    pub fn is_not_null(self) -> Expression {
        Expression::Unary {
            operator: UnaryOperator::IsNotNull,
            expression: Box::new(self.expression()),
        }
    }

    fn compare(self, operator: BinaryOperator, right: Expression) -> Expression {
        Expression::Binary {
            left: Box::new(self.expression()),
            operator,
            right: Box::new(right),
        }
    }
}

pub fn exists<Source: crate::Table, Output>(
    query: crate::query::SelectQuery<Source, Output>,
) -> Expression {
    Expression::Unary {
        operator: UnaryOperator::Exists,
        expression: Box::new(Expression::Subquery(SelectSubquery(Box::new(
            query.into_node(),
        )))),
    }
}

pub trait Selection {
    type Output;
    fn expressions(self) -> Vec<Expression>;
}

pub struct AliasedSelection<S> {
    selection: S,
    alias: &'static str,
}

pub trait SelectionExt: Selection + Sized {
    fn alias(self, alias: &'static str) -> AliasedSelection<Self> {
        AliasedSelection {
            selection: self,
            alias,
        }
    }
}

impl<T, V> SelectionExt for Column<T, V> {}

impl<S: Selection> Selection for AliasedSelection<S> {
    type Output = S::Output;

    fn expressions(self) -> Vec<Expression> {
        self.selection
            .expressions()
            .into_iter()
            .map(|expression| Expression::Alias {
                expression: Box::new(expression),
                alias: self.alias,
            })
            .collect()
    }
}

impl<T, V> Selection for Column<T, V> {
    type Output = V;

    fn expressions(self) -> Vec<Expression> {
        vec![self.expression()]
    }
}

macro_rules! tuple_selection {
    ($($name:ident),+ $(,)?) => {
        impl<$($name),+> Selection for ($($name,)+)
        where
            $($name: Selection,)+
        {
            type Output = ($($name::Output,)+);

            #[allow(non_snake_case)]
            fn expressions(self) -> Vec<Expression> {
                let ($($name,)+) = self;
                let mut expressions = Vec::new();
                $(expressions.extend($name.expressions());)+
                expressions
            }
        }
    };
}

tuple_selection!(A, B);
tuple_selection!(A, B, C);
tuple_selection!(A, B, C, D);
tuple_selection!(A, B, C, D, E);
tuple_selection!(A, B, C, D, E, F);
tuple_selection!(A, B, C, D, E, F, G);
tuple_selection!(A, B, C, D, E, F, G, H);
