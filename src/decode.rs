use crate::Value;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DecodeError {
    #[error("result row has no column at index {index}")]
    MissingColumn { index: usize },
    #[error("cannot decode {actual} at column {index} as {expected}")]
    TypeMismatch {
        index: usize,
        expected: &'static str,
        actual: &'static str,
    },
    #[error("integer at column {index} is outside the range of {target}")]
    IntegerOverflow { index: usize, target: &'static str },
}

pub trait Row {
    fn value(&self, index: usize) -> Option<&Value>;
}

pub trait FromValue: Sized {
    const EXPECTED: &'static str;

    fn from_value(value: &Value, index: usize) -> Result<Self, DecodeError>;
}

pub trait FromRow: Sized {
    fn from_row(row: &impl Row) -> Result<Self, DecodeError>;
}

impl<T: FromValue> FromRow for T {
    fn from_row(row: &impl Row) -> Result<Self, DecodeError> {
        decode_at(row, 0)
    }
}

fn decode_at<T: FromValue>(row: &impl Row, index: usize) -> Result<T, DecodeError> {
    let value = row
        .value(index)
        .ok_or(DecodeError::MissingColumn { index })?;
    T::from_value(value, index)
}

fn mismatch<T: FromValue>(value: &Value, index: usize) -> DecodeError {
    DecodeError::TypeMismatch {
        index,
        expected: T::EXPECTED,
        actual: value.kind(),
    }
}

impl FromValue for bool {
    const EXPECTED: &'static str = "boolean";

    fn from_value(value: &Value, index: usize) -> Result<Self, DecodeError> {
        match value {
            Value::Bool(value) => Ok(*value),
            Value::I64(0) => Ok(false),
            Value::I64(1) => Ok(true),
            _ => Err(mismatch::<Self>(value, index)),
        }
    }
}

macro_rules! signed_decoders {
    ($($type:ty),+ $(,)?) => {$(
        impl FromValue for $type {
            const EXPECTED: &'static str = stringify!($type);

            fn from_value(value: &Value, index: usize) -> Result<Self, DecodeError> {
                let value = match value {
                    Value::I64(value) => *value,
                    _ => return Err(mismatch::<Self>(value, index)),
                };
                <$type>::try_from(value).map_err(|_| DecodeError::IntegerOverflow {
                    index,
                    target: stringify!($type),
                })
            }
        }
    )+};
}

macro_rules! unsigned_decoders {
    ($($type:ty),+ $(,)?) => {$(
        impl FromValue for $type {
            const EXPECTED: &'static str = stringify!($type);

            fn from_value(value: &Value, index: usize) -> Result<Self, DecodeError> {
                let value = match value {
                    Value::U64(value) => *value,
                    Value::I64(value) => u64::try_from(*value).map_err(|_| {
                        DecodeError::IntegerOverflow { index, target: stringify!($type) }
                    })?,
                    _ => return Err(mismatch::<Self>(value, index)),
                };
                <$type>::try_from(value).map_err(|_| DecodeError::IntegerOverflow {
                    index,
                    target: stringify!($type),
                })
            }
        }
    )+};
}

signed_decoders!(i8, i16, i32, i64, isize);
unsigned_decoders!(u8, u16, u32, u64, usize);

impl FromValue for f64 {
    const EXPECTED: &'static str = "f64";

    fn from_value(value: &Value, index: usize) -> Result<Self, DecodeError> {
        match value {
            Value::F64(value) => Ok(*value),
            Value::I64(value) => Ok(*value as f64),
            _ => Err(mismatch::<Self>(value, index)),
        }
    }
}

impl FromValue for f32 {
    const EXPECTED: &'static str = "f32";

    fn from_value(value: &Value, index: usize) -> Result<Self, DecodeError> {
        f64::from_value(value, index).map(|value| value as f32)
    }
}

impl FromValue for String {
    const EXPECTED: &'static str = "string";

    fn from_value(value: &Value, index: usize) -> Result<Self, DecodeError> {
        match value {
            Value::String(value) => Ok(value.clone()),
            _ => Err(mismatch::<Self>(value, index)),
        }
    }
}

impl FromValue for Vec<u8> {
    const EXPECTED: &'static str = "bytes";

    fn from_value(value: &Value, index: usize) -> Result<Self, DecodeError> {
        match value {
            Value::Bytes(value) => Ok(value.clone()),
            _ => Err(mismatch::<Self>(value, index)),
        }
    }
}

impl<T: FromValue> FromValue for Option<T> {
    const EXPECTED: &'static str = T::EXPECTED;

    fn from_value(value: &Value, index: usize) -> Result<Self, DecodeError> {
        match value {
            Value::Null => Ok(None),
            value => T::from_value(value, index).map(Some),
        }
    }
}

macro_rules! tuple_rows {
    ($(($type:ident, $index:tt)),+ $(,)?) => {
        impl<$($type: FromValue),+> FromRow for ($($type,)+) {
            fn from_row(row: &impl Row) -> Result<Self, DecodeError> {
                Ok(($(decode_at::<$type>(row, $index)?,)+))
            }
        }
    };
}

tuple_rows!((A, 0), (B, 1));
tuple_rows!((A, 0), (B, 1), (C, 2));
tuple_rows!((A, 0), (B, 1), (C, 2), (D, 3));
tuple_rows!((A, 0), (B, 1), (C, 2), (D, 3), (E, 4));
tuple_rows!((A, 0), (B, 1), (C, 2), (D, 3), (E, 4), (F, 5));
tuple_rows!((A, 0), (B, 1), (C, 2), (D, 3), (E, 4), (F, 5), (G, 6));
tuple_rows!(
    (A, 0),
    (B, 1),
    (C, 2),
    (D, 3),
    (E, 4),
    (F, 5),
    (G, 6),
    (H, 7)
);
