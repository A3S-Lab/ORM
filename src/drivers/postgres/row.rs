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
        Type::UUID => optional(row.try_get(index)?, Value::Uuid),
        Type::JSON | Type::JSONB => optional(row.try_get(index)?, Value::Json),
        Type::DATE => optional(row.try_get(index)?, Value::Date),
        Type::TIME => optional(row.try_get(index)?, Value::Time),
        Type::TIMESTAMP => optional(row.try_get(index)?, Value::DateTime),
        Type::TIMESTAMPTZ => optional(row.try_get(index)?, Value::DateTimeUtc),
        Type::NUMERIC => optional(row.try_get(index)?, Value::Decimal),
        Type::BOOL_ARRAY => array(
            row.try_get::<_, Option<Vec<Option<bool>>>>(index)?,
            Value::Bool,
        ),
        Type::INT2_ARRAY => array(
            row.try_get::<_, Option<Vec<Option<i16>>>>(index)?,
            |value| Value::I64(value.into()),
        ),
        Type::INT4_ARRAY => array(
            row.try_get::<_, Option<Vec<Option<i32>>>>(index)?,
            |value| Value::I64(value.into()),
        ),
        Type::INT8_ARRAY => array(
            row.try_get::<_, Option<Vec<Option<i64>>>>(index)?,
            Value::I64,
        ),
        Type::FLOAT4_ARRAY => array(
            row.try_get::<_, Option<Vec<Option<f32>>>>(index)?,
            |value| Value::F64(value.into()),
        ),
        Type::FLOAT8_ARRAY => array(
            row.try_get::<_, Option<Vec<Option<f64>>>>(index)?,
            Value::F64,
        ),
        Type::TEXT_ARRAY | Type::VARCHAR_ARRAY | Type::BPCHAR_ARRAY | Type::NAME_ARRAY => array(
            row.try_get::<_, Option<Vec<Option<String>>>>(index)?,
            Value::String,
        ),
        Type::UUID_ARRAY => array(
            row.try_get::<_, Option<Vec<Option<uuid::Uuid>>>>(index)?,
            Value::Uuid,
        ),
        Type::JSON_ARRAY | Type::JSONB_ARRAY => array(
            row.try_get::<_, Option<Vec<Option<serde_json::Value>>>>(index)?,
            Value::Json,
        ),
        Type::DATE_ARRAY => array(
            row.try_get::<_, Option<Vec<Option<chrono::NaiveDate>>>>(index)?,
            Value::Date,
        ),
        Type::TIME_ARRAY => array(
            row.try_get::<_, Option<Vec<Option<chrono::NaiveTime>>>>(index)?,
            Value::Time,
        ),
        Type::TIMESTAMP_ARRAY => array(
            row.try_get::<_, Option<Vec<Option<chrono::NaiveDateTime>>>>(index)?,
            Value::DateTime,
        ),
        Type::TIMESTAMPTZ_ARRAY => array(
            row.try_get::<_, Option<Vec<Option<chrono::DateTime<chrono::Utc>>>>>(index)?,
            Value::DateTimeUtc,
        ),
        Type::NUMERIC_ARRAY => array(
            row.try_get::<_, Option<Vec<Option<rust_decimal::Decimal>>>>(index)?,
            Value::Decimal,
        ),
        _ => Err(PostgresError::UnsupportedType(ty.to_string())),
    }
}

fn array<T>(
    value: Option<Vec<Option<T>>>,
    convert: impl Fn(T) -> Value,
) -> Result<Value, PostgresError> {
    Ok(match value {
        None => Value::Null,
        Some(values) => Value::Array(
            values
                .into_iter()
                .map(|value| value.map(&convert).unwrap_or(Value::Null))
                .collect(),
        ),
    })
}

fn optional<T>(value: Option<T>, convert: impl FnOnce(T) -> Value) -> Result<Value, PostgresError> {
    Ok(value.map(convert).unwrap_or(Value::Null))
}
