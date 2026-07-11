use std::marker::PhantomData;

use crate::expression::{BinaryOperator, Expression, Selection, SelectionExt};
use crate::value::IntoSqlValue;
use crate::Column;

#[derive(Clone, Debug)]
pub struct TypedExpression<V> {
    expression: Expression,
    marker: PhantomData<fn() -> V>,
}

impl<V> TypedExpression<V> {
    pub(crate) fn new(expression: Expression) -> Self {
        Self {
            expression,
            marker: PhantomData,
        }
    }

    pub fn eq(self, value: impl IntoSqlValue<V>) -> Expression {
        self.compare(BinaryOperator::Eq, value)
    }

    pub fn ne(self, value: impl IntoSqlValue<V>) -> Expression {
        self.compare(BinaryOperator::NotEq, value)
    }

    pub fn gt(self, value: impl IntoSqlValue<V>) -> Expression {
        self.compare(BinaryOperator::GreaterThan, value)
    }

    pub fn gte(self, value: impl IntoSqlValue<V>) -> Expression {
        self.compare(BinaryOperator::GreaterThanOrEq, value)
    }

    pub fn lt(self, value: impl IntoSqlValue<V>) -> Expression {
        self.compare(BinaryOperator::LessThan, value)
    }

    pub fn lte(self, value: impl IntoSqlValue<V>) -> Expression {
        self.compare(BinaryOperator::LessThanOrEq, value)
    }

    fn compare(self, operator: BinaryOperator, value: impl IntoSqlValue<V>) -> Expression {
        Expression::Binary {
            left: Box::new(self.expression),
            operator,
            right: Box::new(Expression::Value(value.into_sql_value())),
        }
    }
}

impl<V> Selection for TypedExpression<V> {
    type Output = V;

    fn expressions(self) -> Vec<Expression> {
        vec![self.expression]
    }
}

impl<V> SelectionExt for TypedExpression<V> {}

pub fn count<T, V>(column: Column<T, V>) -> TypedExpression<i64> {
    function("count", vec![column.expression()])
}

pub fn count_all() -> TypedExpression<i64> {
    function("count", vec![Expression::Wildcard])
}

pub fn min<T, V>(column: Column<T, V>) -> TypedExpression<V> {
    function("min", vec![column.expression()])
}

pub fn max<T, V>(column: Column<T, V>) -> TypedExpression<V> {
    function("max", vec![column.expression()])
}

fn function<V>(name: &'static str, arguments: Vec<Expression>) -> TypedExpression<V> {
    TypedExpression::new(Expression::Function { name, arguments })
}
