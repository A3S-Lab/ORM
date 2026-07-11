mod dialect;
mod validation;

pub use dialect::{Dialect, MysqlDialect, PostgresDialect, SqliteDialect};

use crate::ast::{
    ConflictAction, ConflictValue, DeleteNode, InsertNode, JoinKind, QueryNode, SelectNode,
    TableNode, UpdateNode,
};
use crate::error::{Error, Result};
use crate::expression::{BinaryOperator, Expression, OrderDirection, UnaryOperator};
use crate::value::Value;
use validation::{validate_identifier, verify_assignments, verify_insert_rows};

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledQuery {
    pub sql: String,
    pub parameters: Vec<Value>,
}

pub(crate) fn compile(query: QueryNode, dialect: &impl Dialect) -> Result<CompiledQuery> {
    let mut compiler = Compiler {
        dialect,
        sql: String::new(),
        parameters: Vec::new(),
    };
    match query {
        QueryNode::Select(node) => compiler.select(node)?,
        QueryNode::Insert(node) => compiler.insert(node)?,
        QueryNode::Update(node) => compiler.update(node)?,
        QueryNode::Delete(node) => compiler.delete(node)?,
    }
    Ok(CompiledQuery {
        sql: compiler.sql,
        parameters: compiler.parameters,
    })
}

struct Compiler<'a, D: Dialect> {
    dialect: &'a D,
    sql: String,
    parameters: Vec<Value>,
}

impl<D: Dialect> Compiler<'_, D> {
    fn select(&mut self, node: SelectNode) -> Result<()> {
        if node.selections.is_empty() {
            return Err(Error::EmptySelection);
        }
        if !node.ctes.is_empty() {
            let mut names = std::collections::HashSet::with_capacity(node.ctes.len());
            for cte in &node.ctes {
                if !names.insert(cte.name) {
                    return Err(Error::DuplicateCte(cte.name.to_owned()));
                }
            }
            self.sql.push_str("with ");
            for (index, cte) in node.ctes.into_iter().enumerate() {
                if index > 0 {
                    self.sql.push_str(", ");
                }
                self.identifier(cte.name)?;
                self.sql.push_str(" as (");
                self.select(*cte.query)?;
                self.sql.push(')');
            }
            self.sql.push(' ');
        }
        self.sql.push_str("select ");
        if node.distinct {
            self.sql.push_str("distinct ");
        }
        self.expression_list(&node.selections)?;
        self.sql.push_str(" from ");
        self.table(&node.from)?;
        for join in node.joins {
            self.sql.push(' ');
            self.sql.push_str(match join.kind {
                JoinKind::Inner => "inner join ",
                JoinKind::Left => "left join ",
                JoinKind::Right => "right join ",
                JoinKind::Full => "full join ",
            });
            self.table(&join.table)?;
            self.sql.push_str(" on ");
            self.expression(&join.on)?;
        }
        self.filter(node.filter.as_ref())?;
        if !node.group_by.is_empty() {
            self.sql.push_str(" group by ");
            self.expression_list(&node.group_by)?;
        }
        if let Some(having) = node.having.as_ref() {
            self.sql.push_str(" having ");
            self.expression(having)?;
        }
        if !node.order_by.is_empty() {
            self.sql.push_str(" order by ");
            for (index, (expression, direction)) in node.order_by.iter().enumerate() {
                if index > 0 {
                    self.sql.push_str(", ");
                }
                self.expression(expression)?;
                self.sql.push_str(match direction {
                    OrderDirection::Asc => " asc",
                    OrderDirection::Desc => " desc",
                });
            }
        }
        if let Some(limit) = node.limit {
            self.sql.push_str(" limit ");
            self.parameter(Value::U64(limit));
        }
        if let Some(offset) = node.offset {
            self.sql.push_str(" offset ");
            self.parameter(Value::U64(offset));
        }
        Ok(())
    }

    fn insert(&mut self, node: InsertNode) -> Result<()> {
        if node.rows.is_empty() || node.rows[0].is_empty() {
            return Err(Error::EmptyInsert);
        }
        verify_insert_rows(&node)?;
        self.sql.push_str("insert into ");
        self.table(&node.table)?;
        self.sql.push_str(" (");
        for (index, assignment) in node.rows[0].iter().enumerate() {
            if index > 0 {
                self.sql.push_str(", ");
            }
            self.identifier(assignment.column)?;
        }
        self.sql.push_str(") values ");
        for (row_index, row) in node.rows.into_iter().enumerate() {
            if row_index > 0 {
                self.sql.push_str(", ");
            }
            self.sql.push('(');
            for (column_index, assignment) in row.into_iter().enumerate() {
                if column_index > 0 {
                    self.sql.push_str(", ");
                }
                self.parameter(assignment.value);
            }
            self.sql.push(')');
        }
        if let Some(conflict) = node.conflict {
            if !self.dialect.supports_on_conflict() {
                return Err(Error::Compilation(format!(
                    "{} does not support on conflict clauses",
                    self.dialect.name()
                )));
            }
            self.sql.push_str(" on conflict (");
            for (index, column) in conflict.target.iter().enumerate() {
                if index > 0 {
                    self.sql.push_str(", ");
                }
                self.identifier(column)?;
            }
            self.sql.push_str(") ");
            match conflict.action {
                Some(ConflictAction::DoNothing) => self.sql.push_str("do nothing"),
                Some(ConflictAction::DoUpdate(assignments)) => {
                    self.sql.push_str("do update set ");
                    for (index, assignment) in assignments.into_iter().enumerate() {
                        if index > 0 {
                            self.sql.push_str(", ");
                        }
                        self.identifier(assignment.column)?;
                        self.sql.push_str(" = ");
                        match assignment.value {
                            ConflictValue::Bound(value) => self.parameter(value),
                            ConflictValue::Excluded { column, .. } => {
                                self.sql.push_str("excluded.");
                                self.identifier(column)?;
                            }
                        }
                    }
                }
                None => return Err(Error::MissingConflictAction),
            }
        }
        self.returning(&node.returning)
    }

    fn update(&mut self, node: UpdateNode) -> Result<()> {
        if node.assignments.is_empty() {
            return Err(Error::EmptyUpdate);
        }
        verify_assignments(&node.table, &node.assignments, false)?;
        self.sql.push_str("update ");
        self.table(&node.table)?;
        self.sql.push_str(" set ");
        for (index, assignment) in node.assignments.into_iter().enumerate() {
            if index > 0 {
                self.sql.push_str(", ");
            }
            self.identifier(assignment.column)?;
            self.sql.push_str(" = ");
            self.parameter(assignment.value);
        }
        self.filter(node.filter.as_ref())?;
        self.returning(&node.returning)
    }

    fn delete(&mut self, node: DeleteNode) -> Result<()> {
        self.sql.push_str("delete from ");
        self.table(&node.table)?;
        self.filter(node.filter.as_ref())?;
        self.returning(&node.returning)
    }

    fn returning(&mut self, expressions: &[Expression]) -> Result<()> {
        if expressions.is_empty() {
            return Ok(());
        }
        if !self.dialect.supports_returning() {
            return Err(Error::Compilation(format!(
                "{} does not support returning clauses",
                self.dialect.name()
            )));
        }
        self.sql.push_str(" returning ");
        self.expression_list(expressions)
    }

    fn filter(&mut self, expression: Option<&Expression>) -> Result<()> {
        if let Some(expression) = expression {
            self.sql.push_str(" where ");
            self.expression(expression)?;
        }
        Ok(())
    }

    fn expression_list(&mut self, expressions: &[Expression]) -> Result<()> {
        for (index, expression) in expressions.iter().enumerate() {
            if index > 0 {
                self.sql.push_str(", ");
            }
            self.expression(expression)?;
        }
        Ok(())
    }

    fn expression(&mut self, expression: &Expression) -> Result<()> {
        match expression {
            Expression::Column { table, name } => {
                self.identifier(table)?;
                self.sql.push('.');
                if *name == "*" {
                    self.sql.push('*');
                } else {
                    self.identifier(name)?;
                }
            }
            Expression::Value(value) => self.parameter(value.clone()),
            Expression::Subquery(query) => self.subquery(&query.0, true)?,
            Expression::Function { name, arguments } => {
                self.identifier(name)?;
                self.sql.push('(');
                self.expression_list(arguments)?;
                self.sql.push(')');
            }
            Expression::Alias { expression, alias } => {
                self.expression(expression)?;
                self.sql.push_str(" as ");
                self.identifier(alias)?;
            }
            Expression::Wildcard => self.sql.push('*'),
            Expression::Binary {
                left,
                operator,
                right,
            } => {
                self.sql.push('(');
                self.expression(left)?;
                self.sql.push_str(match operator {
                    BinaryOperator::Eq => " = ",
                    BinaryOperator::NotEq => " <> ",
                    BinaryOperator::GreaterThan => " > ",
                    BinaryOperator::GreaterThanOrEq => " >= ",
                    BinaryOperator::LessThan => " < ",
                    BinaryOperator::LessThanOrEq => " <= ",
                    BinaryOperator::Like => " like ",
                    BinaryOperator::In => " in ",
                    BinaryOperator::Is => " is ",
                    BinaryOperator::IsNot => " is not ",
                });
                self.expression(right)?;
                self.sql.push(')');
            }
            Expression::Unary {
                operator,
                expression,
            } => match operator {
                UnaryOperator::IsNull => {
                    self.expression(expression)?;
                    self.sql.push_str(" is null");
                }
                UnaryOperator::IsNotNull => {
                    self.expression(expression)?;
                    self.sql.push_str(" is not null");
                }
                UnaryOperator::Not => {
                    self.sql.push_str("not (");
                    self.expression(expression)?;
                    self.sql.push(')');
                }
                UnaryOperator::Exists => {
                    self.sql.push_str("exists ");
                    match expression.as_ref() {
                        Expression::Subquery(query) => self.subquery(&query.0, false)?,
                        expression => self.expression(expression)?,
                    }
                }
            },
            Expression::And(expressions) => self.boolean_group(expressions, " and ")?,
            Expression::Or(expressions) => self.boolean_group(expressions, " or ")?,
        }
        Ok(())
    }

    fn subquery(&mut self, query: &SelectNode, scalar: bool) -> Result<()> {
        if scalar && query.selections.len() != 1 {
            return Err(Error::InvalidScalarSubquery(query.selections.len()));
        }
        self.sql.push('(');
        self.select(query.clone())?;
        self.sql.push(')');
        Ok(())
    }

    fn boolean_group(&mut self, expressions: &[Expression], separator: &str) -> Result<()> {
        if expressions.is_empty() {
            return Err(Error::Compilation(
                "boolean expression group cannot be empty".to_string(),
            ));
        }
        self.sql.push('(');
        for (index, expression) in expressions.iter().enumerate() {
            if index > 0 {
                self.sql.push_str(separator);
            }
            self.expression(expression)?;
        }
        self.sql.push(')');
        Ok(())
    }

    fn table(&mut self, table: &TableNode) -> Result<()> {
        self.identifier(table.name)?;
        if let Some(alias) = table.alias {
            self.sql.push_str(" as ");
            self.identifier(alias)?;
        }
        Ok(())
    }

    fn identifier(&mut self, identifier: &str) -> Result<()> {
        validate_identifier(identifier)?;
        self.sql.push(self.dialect.identifier_quote());
        for character in identifier.chars() {
            if character == self.dialect.identifier_quote() {
                self.sql.push(character);
            }
            self.sql.push(character);
        }
        self.sql.push(self.dialect.identifier_quote());
        Ok(())
    }

    fn parameter(&mut self, value: Value) {
        self.parameters.push(value);
        self.sql
            .push_str(&self.dialect.placeholder(self.parameters.len()));
    }
}
