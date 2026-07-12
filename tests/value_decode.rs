use a3s_orm::{DecodeError, FromRow, FromValue, Row, SqlArray, Value};

#[derive(Debug)]
struct Values(Vec<Value>);

impl Row for Values {
    fn value(&self, index: usize) -> Option<&Value> {
        self.0.get(index)
    }
}

#[test]
fn converts_primitive_values_and_reports_their_kinds() {
    let values = [
        Value::from(true),
        Value::from(-1_i8),
        Value::from(-2_i16),
        Value::from(-3_i32),
        Value::from(-4_i64),
        Value::from(-5_isize),
        Value::from(1_u8),
        Value::from(2_u16),
        Value::from(3_u32),
        Value::from(4_u64),
        Value::from(5_usize),
        Value::from(1.25_f32),
        Value::from(2.5_f64),
        Value::from("borrowed"),
        Value::from(String::from("owned")),
        Value::from(vec![1_u8, 2]),
        Value::from(None::<i64>),
        Value::from(Some(7_i64)),
        Value::from(SqlArray::from(vec![1_i32, 2])),
    ];
    let kinds: Vec<_> = values.iter().map(Value::kind).collect();
    assert_eq!(
        kinds,
        [
            "boolean",
            "signed integer",
            "signed integer",
            "signed integer",
            "signed integer",
            "signed integer",
            "unsigned integer",
            "unsigned integer",
            "unsigned integer",
            "unsigned integer",
            "unsigned integer",
            "floating-point number",
            "floating-point number",
            "string",
            "string",
            "bytes",
            "null",
            "signed integer",
            "array",
        ]
    );
}

#[test]
fn decodes_all_core_scalar_representations() {
    assert!(bool::from_value(&Value::Bool(true), 0).unwrap());
    assert!(!bool::from_value(&Value::I64(0), 0).unwrap());
    assert!(bool::from_value(&Value::I64(1), 0).unwrap());

    macro_rules! signed {
        ($($ty:ty),+ $(,)?) => {$(
            assert_eq!(<$ty>::from_value(&Value::I64(7), 0).unwrap(), 7);
        )+};
    }
    macro_rules! unsigned {
        ($($ty:ty),+ $(,)?) => {$(
            assert_eq!(<$ty>::from_value(&Value::U64(7), 0).unwrap(), 7);
            assert_eq!(<$ty>::from_value(&Value::I64(7), 0).unwrap(), 7);
        )+};
    }
    signed!(i8, i16, i32, i64, isize);
    unsigned!(u8, u16, u32, u64, usize);

    assert_eq!(f64::from_value(&Value::F64(1.5), 0).unwrap(), 1.5);
    assert_eq!(f64::from_value(&Value::I64(2), 0).unwrap(), 2.0);
    assert_eq!(f32::from_value(&Value::F64(1.5), 0).unwrap(), 1.5);
    assert_eq!(String::from_value(&Value::from("text"), 0).unwrap(), "text");
    assert_eq!(
        Vec::<u8>::from_value(&Value::Bytes(vec![1, 2]), 0).unwrap(),
        [1, 2]
    );
    assert_eq!(Option::<i64>::from_value(&Value::Null, 0).unwrap(), None);
    assert_eq!(
        Option::<i64>::from_value(&Value::I64(9), 0).unwrap(),
        Some(9)
    );
    assert_eq!(
        SqlArray::<i32>::from_value(&Value::Array(vec![Value::I64(1), Value::I64(2)]), 0).unwrap(),
        SqlArray(vec![1, 2])
    );
}

#[test]
fn decoding_returns_contextual_errors() {
    assert_eq!(
        bool::from_value(&Value::String("no".into()), 3),
        Err(DecodeError::TypeMismatch {
            index: 3,
            expected: "boolean",
            actual: "string",
        })
    );
    assert_eq!(
        i8::from_value(&Value::I64(128), 4),
        Err(DecodeError::IntegerOverflow {
            index: 4,
            target: "i8"
        })
    );
    assert_eq!(
        u8::from_value(&Value::I64(-1), 5),
        Err(DecodeError::IntegerOverflow {
            index: 5,
            target: "u8"
        })
    );
    assert!(matches!(
        SqlArray::<i32>::from_value(
            &Value::Array(vec![Value::I64(1), Value::String("bad".into())]),
            6
        ),
        Err(DecodeError::ArrayElement {
            index: 6,
            element: 1,
            ..
        })
    ));
    assert!(matches!(
        SqlArray::<i32>::from_value(&Value::I64(1), 7),
        Err(DecodeError::TypeMismatch { index: 7, .. })
    ));
    assert_eq!(
        i64::from_row(&Values(vec![])),
        Err(DecodeError::MissingColumn { index: 0 })
    );
}

#[test]
fn decodes_supported_tuple_arities() {
    let row = Values(vec![
        Value::I64(1),
        Value::I64(2),
        Value::I64(3),
        Value::I64(4),
        Value::I64(5),
        Value::I64(6),
        Value::I64(7),
        Value::I64(8),
    ]);
    assert_eq!(<(i64, i64)>::from_row(&row).unwrap(), (1, 2));
    assert_eq!(<(i64, i64, i64)>::from_row(&row).unwrap(), (1, 2, 3));
    assert_eq!(
        <(i64, i64, i64, i64)>::from_row(&row).unwrap(),
        (1, 2, 3, 4)
    );
    assert_eq!(
        <(i64, i64, i64, i64, i64)>::from_row(&row).unwrap(),
        (1, 2, 3, 4, 5)
    );
    assert_eq!(
        <(i64, i64, i64, i64, i64, i64)>::from_row(&row).unwrap(),
        (1, 2, 3, 4, 5, 6)
    );
    assert_eq!(
        <(i64, i64, i64, i64, i64, i64, i64)>::from_row(&row).unwrap(),
        (1, 2, 3, 4, 5, 6, 7)
    );
    assert_eq!(
        <(i64, i64, i64, i64, i64, i64, i64, i64)>::from_row(&row).unwrap(),
        (1, 2, 3, 4, 5, 6, 7, 8)
    );
}

#[cfg(all(
    feature = "uuid",
    feature = "json",
    feature = "chrono",
    feature = "decimal"
))]
#[test]
fn converts_and_decodes_feature_gated_values() {
    let uuid = uuid::Uuid::nil();
    let json = serde_json::json!({"ok": true});
    let date = chrono::NaiveDate::from_ymd_opt(2026, 7, 12).unwrap();
    let time = chrono::NaiveTime::from_hms_opt(10, 20, 30).unwrap();
    let datetime = date.and_time(time);
    let utc = datetime.and_utc();
    let decimal = rust_decimal::Decimal::new(1234, 2);

    macro_rules! round_trip {
        ($value:expr, $ty:ty, $kind:literal) => {{
            let encoded = Value::from($value.clone());
            assert_eq!(encoded.kind(), $kind);
            assert_eq!(<$ty>::from_value(&encoded, 0).unwrap(), $value);
        }};
    }
    round_trip!(uuid, uuid::Uuid, "uuid");
    round_trip!(json, serde_json::Value, "json");
    round_trip!(date, chrono::NaiveDate, "date");
    round_trip!(time, chrono::NaiveTime, "time");
    round_trip!(datetime, chrono::NaiveDateTime, "timestamp");
    round_trip!(
        utc,
        chrono::DateTime<chrono::Utc>,
        "timestamp with time zone"
    );
    round_trip!(decimal, rust_decimal::Decimal, "decimal");
}
