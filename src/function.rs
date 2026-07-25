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

    pub fn over(self) -> crate::WindowExpression<V> {
        crate::WindowExpression::new(self.expression)
    }

    /// Consume this typed expression for composition in another expression.
    pub fn expression(self) -> Expression {
        self.expression
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
    sql_function("count", vec![column.expression()])
}

pub fn count_all() -> TypedExpression<i64> {
    sql_function("count", vec![Expression::Wildcard])
}

pub fn min<T, V>(column: Column<T, V>) -> TypedExpression<V> {
    sql_function("min", vec![column.expression()])
}

pub fn max<T, V>(column: Column<T, V>) -> TypedExpression<V> {
    sql_function("max", vec![column.expression()])
}

/// Build a typed bound-value expression.
pub fn bound<V>(value: impl IntoSqlValue<V>) -> TypedExpression<V> {
    TypedExpression::new(Expression::Value(value.into_sql_value()))
}

/// Call a scalar SQL function with a caller-declared result type.
///
/// The function name is validated as an identifier and every runtime value in
/// its arguments remains a bound parameter.
pub fn sql_function<V>(
    name: &'static str,
    arguments: impl IntoIterator<Item = Expression>,
) -> TypedExpression<V> {
    TypedExpression::new(Expression::Function {
        name,
        arguments: arguments.into_iter().collect(),
    })
}

/// Cast a typed expression to a validated SQL type name.
///
/// When a driver has no native codec for the target type, first cast a bound
/// value to its source SQL type and then cast that expression to the target.
pub fn cast<From, To>(
    expression: TypedExpression<From>,
    sql_type: &'static str,
) -> TypedExpression<To> {
    TypedExpression::new(Expression::Cast {
        expression: Box::new(expression.expression()),
        sql_type,
    })
}
