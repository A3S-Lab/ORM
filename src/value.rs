/// A bound SQL parameter. Values are never interpolated into generated SQL.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    I64(i64),
    U64(u64),
    F64(f64),
    String(String),
    Bytes(Vec<u8>),
}

impl Value {
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "boolean",
            Self::I64(_) => "signed integer",
            Self::U64(_) => "unsigned integer",
            Self::F64(_) => "floating-point number",
            Self::String(_) => "string",
            Self::Bytes(_) => "bytes",
        }
    }
}

/// Converts a Rust value into a parameter for a specific typed column.
pub trait IntoSqlValue<T> {
    fn into_sql_value(self) -> Value;
}

impl<T> IntoSqlValue<T> for T
where
    T: Into<Value>,
{
    fn into_sql_value(self) -> Value {
        self.into()
    }
}

impl IntoSqlValue<String> for &str {
    fn into_sql_value(self) -> Value {
        Value::String(self.to_owned())
    }
}

impl<T> IntoSqlValue<Option<T>> for T
where
    T: Into<Value>,
{
    fn into_sql_value(self) -> Value {
        self.into()
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

macro_rules! signed_values {
    ($($type:ty),* $(,)?) => {
        $(
            impl From<$type> for Value {
                fn from(value: $type) -> Self {
                    Self::I64(value as i64)
                }
            }
        )*
    };
}

macro_rules! unsigned_values {
    ($($type:ty),* $(,)?) => {
        $(
            impl From<$type> for Value {
                fn from(value: $type) -> Self {
                    Self::U64(value as u64)
                }
            }
        )*
    };
}

signed_values!(i8, i16, i32, i64, isize);
unsigned_values!(u8, u16, u32, u64, usize);

impl From<f32> for Value {
    fn from(value: f32) -> Self {
        Self::F64(value as f64)
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Self::F64(value)
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<Vec<u8>> for Value {
    fn from(value: Vec<u8>) -> Self {
        Self::Bytes(value)
    }
}

impl<T> From<Option<T>> for Value
where
    T: Into<Value>,
{
    fn from(value: Option<T>) -> Self {
        value.map(Into::into).unwrap_or(Self::Null)
    }
}
