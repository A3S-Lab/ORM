use std::marker::PhantomData;

use crate::expression::{
    Expression, OrderDirection, Selection, SelectionExt, WindowBoundary, WindowFrame,
    WindowFrameUnits,
};
use crate::Column;

#[derive(Clone, Debug)]
pub struct WindowExpression<V> {
    expression: Expression,
    partition_by: Vec<Expression>,
    order_by: Vec<(Expression, OrderDirection)>,
    frame: Option<WindowFrame>,
    marker: PhantomData<fn() -> V>,
}

impl<V> WindowExpression<V> {
    pub(crate) fn new(expression: Expression) -> Self {
        Self {
            expression,
            partition_by: Vec::new(),
            order_by: Vec::new(),
            frame: None,
            marker: PhantomData,
        }
    }

    pub fn partition_by<T, C>(mut self, column: Column<T, C>) -> Self {
        self.partition_by.push(column.expression());
        self
    }

    pub fn order_by<T, C>(mut self, column: Column<T, C>, direction: OrderDirection) -> Self {
        self.order_by.push((column.expression(), direction));
        self
    }

    pub fn frame(
        mut self,
        units: WindowFrameUnits,
        start: WindowBoundary,
        end: WindowBoundary,
    ) -> Self {
        self.frame = Some(WindowFrame { units, start, end });
        self
    }
}

impl<V> Selection for WindowExpression<V> {
    type Output = V;

    fn expressions(self) -> Vec<Expression> {
        vec![Expression::Window {
            expression: Box::new(self.expression),
            partition_by: self.partition_by,
            order_by: self.order_by,
            frame: self.frame,
        }]
    }
}

impl<V> SelectionExt for WindowExpression<V> {}

pub fn row_number() -> WindowExpression<i64> {
    rank_function("row_number")
}

pub fn rank() -> WindowExpression<i64> {
    rank_function("rank")
}

pub fn dense_rank() -> WindowExpression<i64> {
    rank_function("dense_rank")
}

fn rank_function(name: &'static str) -> WindowExpression<i64> {
    WindowExpression::new(Expression::Function {
        name,
        arguments: Vec::new(),
    })
}
