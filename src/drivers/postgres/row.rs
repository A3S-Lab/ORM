use tokio_postgres::types::Type;

use crate::{Row, Value};

use super::PostgresError;

#[derive(Clone, Debug, PartialEq)]
pub struct PostgresRow {
    values: Vec<Value>,
}

impl PostgresRow {
    pub(crate) fn decode(row: tokio_postgres::Row) -> Result<Self, PostgresError> {
        let values = row
            .columns()
            .iter()
            .enumerate()
            .map(|(index, column)| decode_value(&row, index, column.type_()))
            .collect::<Result<_, _>>()?;
        Ok(Self { values })
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&Value> {
        self.values.get(index)
    }

    pub fn into_values(self) -> Vec<Value> {
        self.values
    }
}

impl Row for PostgresRow {
    fn value(&self, index: usize) -> Option<&Value> {
        self.get(index)
    }
}

fn decode_value(
    row: &tokio_postgres::Row,
    index: usize,
    ty: &Type,
) -> Result<Value, PostgresError> {
    match *ty {
        Type::BOOL => optional(row.try_get(index)?, Value::Bool),
        Type::INT2 => optional(row.try_get::<_, Option<i16>>(index)?, |value| {
            Value::I64(value.into())
        }),
        Type::INT4 => optional(row.try_get::<_, Option<i32>>(index)?, |value| {
            Value::I64(value.into())
        }),
        Type::INT8 => optional(row.try_get(index)?, Value::I64),
        Type::FLOAT4 => optional(row.try_get::<_, Option<f32>>(index)?, |value| {
            Value::F64(value.into())
        }),
        Type::FLOAT8 => optional(row.try_get(index)?, Value::F64),
        Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME => {
            optional(row.try_get(index)?, Value::String)
        }
        Type::BYTEA => optional(row.try_get(index)?, Value::Bytes),
        _ => Err(PostgresError::UnsupportedType(ty.to_string())),
    }
}

fn optional<T>(value: Option<T>, convert: impl FnOnce(T) -> Value) -> Result<Value, PostgresError> {
    Ok(value.map(convert).unwrap_or(Value::Null))
}
