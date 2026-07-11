use crate::expression::{Expression, OrderDirection};
use crate::value::Value;

#[derive(Clone, Debug)]
pub(crate) struct TableNode {
    pub name: &'static str,
    pub alias: Option<&'static str>,
}

#[derive(Clone, Debug)]
pub(crate) struct SelectNode {
    pub from: TableNode,
    pub selections: Vec<Expression>,
    pub joins: Vec<JoinNode>,
    pub filter: Option<Expression>,
    pub order_by: Vec<(Expression, OrderDirection)>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
    pub distinct: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct JoinNode {
    pub kind: JoinKind,
    pub table: TableNode,
    pub on: Expression,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum JoinKind {
    Inner,
    Left,
    Right,
    Full,
}

#[derive(Clone, Debug)]
pub(crate) struct InsertNode {
    pub table: TableNode,
    pub assignments: Vec<Assignment>,
    pub returning: Vec<Expression>,
}

#[derive(Clone, Debug)]
pub(crate) struct UpdateNode {
    pub table: TableNode,
    pub assignments: Vec<Assignment>,
    pub filter: Option<Expression>,
    pub returning: Vec<Expression>,
}

#[derive(Clone, Debug)]
pub(crate) struct DeleteNode {
    pub table: TableNode,
    pub filter: Option<Expression>,
    pub returning: Vec<Expression>,
}

#[derive(Clone, Debug)]
pub(crate) struct Assignment {
    pub table: &'static str,
    pub column: &'static str,
    pub value: Value,
}

#[derive(Clone, Debug)]
pub(crate) enum QueryNode {
    Select(SelectNode),
    Insert(InsertNode),
    Update(UpdateNode),
    Delete(DeleteNode),
}
