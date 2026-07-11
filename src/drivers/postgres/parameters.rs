use bytes::BytesMut;
use tokio_postgres::types::{IsNull, ToSql, Type};

use crate::Value;

use super::PostgresError;

#[derive(Debug)]
struct Null;

impl ToSql for Null {
    fn to_sql(
        &self,
        _ty: &Type,
        _out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn std::error::Error + Send + Sync>> {
        Ok(IsNull::Yes)
    }

    fn accepts(_ty: &Type) -> bool {
        true
    }

    tokio_postgres::types::to_sql_checked!();
}

pub(crate) fn encode(
    values: &[Value],
    types: &[Type],
) -> Result<Vec<Box<dyn ToSql + Sync + Send>>, PostgresError> {
    if values.len() != types.len() {
        return Err(PostgresError::ParameterCount {
            values: values.len(),
            parameters: types.len(),
        });
    }
    values
        .iter()
        .zip(types)
        .map(|(value, ty)| encode_value(value, ty))
        .collect()
}

fn encode_value(value: &Value, ty: &Type) -> Result<Box<dyn ToSql + Sync + Send>, PostgresError> {
    Ok(match value {
        Value::Null => Box::new(Null),
        Value::Bool(value) => Box::new(*value),
        Value::I64(value) => encode_i64(*value, ty)?,
        Value::U64(value) => encode_u64(*value, ty)?,
        Value::F64(value) if *ty == Type::FLOAT4 => Box::new(*value as f32),
        Value::F64(value) => Box::new(*value),
        Value::String(value) => Box::new(value.clone()),
        Value::Bytes(value) => Box::new(value.clone()),
        Value::Array(values) => encode_array(values, ty)?,
        Value::Uuid(value) => Box::new(*value),
        Value::Json(value) => Box::new(value.clone()),
        Value::Date(value) => Box::new(*value),
        Value::Time(value) => Box::new(*value),
        Value::DateTime(value) => Box::new(*value),
        Value::DateTimeUtc(value) => Box::new(*value),
        Value::Decimal(value) => Box::new(*value),
    })
}

fn encode_array(
    values: &[Value],
    ty: &Type,
) -> Result<Box<dyn ToSql + Sync + Send>, PostgresError> {
    Ok(match *ty {
        Type::BOOL_ARRAY => Box::new(array_values(values, |value| match value {
            Value::Bool(value) => Ok(*value),
            _ => Err(array_type(value, "boolean")),
        })?),
        Type::INT2_ARRAY => Box::new(array_values(values, |value| match value {
            Value::I64(value) => i16::try_from(*value).map_err(|_| overflow(*value, "smallint")),
            Value::U64(value) => i16::try_from(*value).map_err(|_| overflow(*value, "smallint")),
            _ => Err(array_type(value, "smallint")),
        })?),
        Type::INT4_ARRAY => Box::new(array_values(values, |value| match value {
            Value::I64(value) => i32::try_from(*value).map_err(|_| overflow(*value, "integer")),
            Value::U64(value) => i32::try_from(*value).map_err(|_| overflow(*value, "integer")),
            _ => Err(array_type(value, "integer")),
        })?),
        Type::INT8_ARRAY => Box::new(array_values(values, |value| match value {
            Value::I64(value) => Ok(*value),
            Value::U64(value) => i64::try_from(*value).map_err(|_| overflow(*value, "bigint")),
            _ => Err(array_type(value, "bigint")),
        })?),
        Type::FLOAT4_ARRAY => Box::new(array_values(values, |value| match value {
            Value::F64(value) => Ok(*value as f32),
            _ => Err(array_type(value, "real")),
        })?),
        Type::FLOAT8_ARRAY => Box::new(array_values(values, |value| match value {
            Value::F64(value) => Ok(*value),
            _ => Err(array_type(value, "double precision")),
        })?),
        Type::TEXT_ARRAY | Type::VARCHAR_ARRAY | Type::BPCHAR_ARRAY | Type::NAME_ARRAY => {
            Box::new(array_values(values, |value| match value {
                Value::String(value) => Ok(value.clone()),
                _ => Err(array_type(value, "text")),
            })?)
        }
        Type::UUID_ARRAY => Box::new(array_values(values, |value| match value {
            Value::Uuid(value) => Ok(*value),
            _ => Err(array_type(value, "uuid")),
        })?),
        Type::JSON_ARRAY | Type::JSONB_ARRAY => {
            Box::new(array_values(values, |value| match value {
                Value::Json(value) => Ok(value.clone()),
                _ => Err(array_type(value, "json")),
            })?)
        }
        Type::DATE_ARRAY => Box::new(array_values(values, |value| match value {
            Value::Date(value) => Ok(*value),
            _ => Err(array_type(value, "date")),
        })?),
        Type::TIME_ARRAY => Box::new(array_values(values, |value| match value {
            Value::Time(value) => Ok(*value),
            _ => Err(array_type(value, "time")),
        })?),
        Type::TIMESTAMP_ARRAY => Box::new(array_values(values, |value| match value {
            Value::DateTime(value) => Ok(*value),
            _ => Err(array_type(value, "timestamp")),
        })?),
        Type::TIMESTAMPTZ_ARRAY => Box::new(array_values(values, |value| match value {
            Value::DateTimeUtc(value) => Ok(*value),
            _ => Err(array_type(value, "timestamp with time zone")),
        })?),
        Type::NUMERIC_ARRAY => Box::new(array_values(values, |value| match value {
            Value::Decimal(value) => Ok(*value),
            _ => Err(array_type(value, "numeric")),
        })?),
        _ => return Err(PostgresError::UnsupportedType(ty.to_string())),
    })
}

fn array_values<T>(
    values: &[Value],
    convert: impl Fn(&Value) -> Result<T, PostgresError>,
) -> Result<Vec<Option<T>>, PostgresError> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| match value {
            Value::Null => Ok(None),
            value => convert(value)
                .map(Some)
                .map_err(|source| PostgresError::ArrayElement {
                    index,
                    source: Box::new(source),
                }),
        })
        .collect()
}

fn array_type(value: &Value, target: &'static str) -> PostgresError {
    PostgresError::ArrayElementType {
        actual: value.kind(),
        target,
    }
}

fn encode_i64(value: i64, ty: &Type) -> Result<Box<dyn ToSql + Sync + Send>, PostgresError> {
    match *ty {
        Type::INT2 => i16::try_from(value)
            .map(|value| Box::new(value) as _)
            .map_err(|_| overflow(value, "smallint")),
        Type::INT4 => i32::try_from(value)
            .map(|value| Box::new(value) as _)
            .map_err(|_| overflow(value, "integer")),
        _ => Ok(Box::new(value)),
    }
}

fn encode_u64(value: u64, ty: &Type) -> Result<Box<dyn ToSql + Sync + Send>, PostgresError> {
    match *ty {
        Type::INT2 => i16::try_from(value)
            .map(|value| Box::new(value) as _)
            .map_err(|_| overflow(value, "smallint")),
        Type::INT4 => i32::try_from(value)
            .map(|value| Box::new(value) as _)
            .map_err(|_| overflow(value, "integer")),
        _ => i64::try_from(value)
            .map(|value| Box::new(value) as _)
            .map_err(|_| overflow(value, "bigint")),
    }
}

fn overflow(value: impl ToString, target: &'static str) -> PostgresError {
    PostgresError::IntegerOverflow {
        value: value.to_string(),
        target,
    }
}

pub(crate) fn references(values: &[Box<dyn ToSql + Sync + Send>]) -> Vec<&(dyn ToSql + Sync)> {
    values.iter().map(|value| value.as_ref() as _).collect()
}
