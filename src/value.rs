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
    Array(Vec<Value>),
    #[cfg(feature = "uuid")]
    Uuid(uuid::Uuid),
    #[cfg(feature = "json")]
    Json(serde_json::Value),
    #[cfg(feature = "chrono")]
    Date(chrono::NaiveDate),
    #[cfg(feature = "chrono")]
    Time(chrono::NaiveTime),
    #[cfg(feature = "chrono")]
    DateTime(chrono::NaiveDateTime),
    #[cfg(feature = "chrono")]
    DateTimeUtc(chrono::DateTime<chrono::Utc>),
    #[cfg(feature = "decimal")]
    Decimal(rust_decimal::Decimal),
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
            Self::Array(_) => "array",
            #[cfg(feature = "uuid")]
            Self::Uuid(_) => "uuid",
            #[cfg(feature = "json")]
            Self::Json(_) => "json",
            #[cfg(feature = "chrono")]
            Self::Date(_) => "date",
            #[cfg(feature = "chrono")]
            Self::Time(_) => "time",
            #[cfg(feature = "chrono")]
            Self::DateTime(_) => "timestamp",
            #[cfg(feature = "chrono")]
            Self::DateTimeUtc(_) => "timestamp with time zone",
            #[cfg(feature = "decimal")]
            Self::Decimal(_) => "decimal",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SqlArray<T>(pub Vec<T>);

impl<T> From<Vec<T>> for SqlArray<T> {
    fn from(values: Vec<T>) -> Self {
        Self(values)
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

impl<T> From<SqlArray<T>> for Value
where
    T: Into<Value>,
{
    fn from(value: SqlArray<T>) -> Self {
        Self::Array(value.0.into_iter().map(Into::into).collect())
    }
}

#[cfg(feature = "uuid")]
impl From<uuid::Uuid> for Value {
    fn from(value: uuid::Uuid) -> Self {
        Self::Uuid(value)
    }
}

#[cfg(feature = "json")]
impl From<serde_json::Value> for Value {
    fn from(value: serde_json::Value) -> Self {
        Self::Json(value)
    }
}

#[cfg(feature = "chrono")]
impl From<chrono::NaiveDate> for Value {
    fn from(value: chrono::NaiveDate) -> Self {
        Self::Date(value)
    }
}

#[cfg(feature = "chrono")]
impl From<chrono::NaiveTime> for Value {
    fn from(value: chrono::NaiveTime) -> Self {
        Self::Time(value)
    }
}

#[cfg(feature = "chrono")]
impl From<chrono::NaiveDateTime> for Value {
    fn from(value: chrono::NaiveDateTime) -> Self {
        Self::DateTime(value)
    }
}

#[cfg(feature = "chrono")]
impl From<chrono::DateTime<chrono::Utc>> for Value {
    fn from(value: chrono::DateTime<chrono::Utc>) -> Self {
        Self::DateTimeUtc(value)
    }
}

#[cfg(feature = "decimal")]
impl From<rust_decimal::Decimal> for Value {
    fn from(value: rust_decimal::Decimal) -> Self {
        Self::Decimal(value)
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
