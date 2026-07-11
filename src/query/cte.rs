use std::marker::PhantomData;

use crate::ast::{CteNode, SelectNode};
use crate::schema::Table;

#[derive(Clone, Debug)]
pub struct Cte<T: Table> {
    pub(crate) node: CteNode,
    marker: PhantomData<fn() -> T>,
}

impl<T: Table> Cte<T> {
    pub(crate) fn new(query: SelectNode) -> Self {
        Self {
            node: CteNode {
                name: T::NAME,
                query: Box::new(query),
            },
            marker: PhantomData,
        }
    }
}
