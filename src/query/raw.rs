use std::marker::PhantomData;

use crate::{CompiledQuery, Dialect, Error, Query, Result, Value};

#[derive(Clone, Debug)]
enum SqlPart {
    Trusted(&'static str),
    Value(Value),
}

/// A trusted static SQL query with separately bound values.
///
/// Text parts must have a static lifetime. Runtime values can only enter the
/// query through bind, which emits a dialect-specific placeholder.
#[derive(Clone, Debug)]
pub struct SqlQuery<O> {
    parts: Vec<SqlPart>,
    marker: PhantomData<fn() -> O>,
}

pub fn sql_query<O>(sql: &'static str) -> SqlQuery<O> {
    SqlQuery {
        parts: vec![SqlPart::Trusted(sql)],
        marker: PhantomData,
    }
}

impl<O> SqlQuery<O> {
    pub fn append(mut self, sql: &'static str) -> Self {
        self.parts.push(SqlPart::Trusted(sql));
        self
    }

    pub fn bind(mut self, value: impl Into<Value>) -> Self {
        self.parts.push(SqlPart::Value(value.into()));
        self
    }
}

impl<O> Query for SqlQuery<O> {
    type Output = O;

    fn compile(self, dialect: &impl Dialect) -> Result<CompiledQuery> {
        let mut sql = String::new();
        let mut parameters = Vec::new();
        for part in self.parts {
            match part {
                SqlPart::Trusted(part) => sql.push_str(part),
                SqlPart::Value(value) => {
                    parameters.push(value);
                    sql.push_str(&dialect.placeholder(parameters.len()));
                }
            }
        }
        if sql.trim().is_empty() {
            return Err(Error::EmptyRawQuery);
        }
        Ok(CompiledQuery { sql, parameters })
    }
}
