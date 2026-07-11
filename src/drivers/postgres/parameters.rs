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
    })
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
