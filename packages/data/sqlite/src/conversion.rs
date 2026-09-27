use fabric_package_relational_database::RelationalValue;
use rusqlite::types::Value;

pub(crate) fn sqlite_values(values: Vec<RelationalValue>) -> Vec<Value> {
    values
        .into_iter()
        .map(|value| match value {
            RelationalValue::Null => Value::Null,
            RelationalValue::Integer(value) => Value::Integer(value),
            RelationalValue::Real(value) => Value::Real(value),
            RelationalValue::Text(value) => Value::Text(value),
            RelationalValue::Bytes(value) => Value::Blob(value),
        })
        .collect()
}

pub(crate) fn relational_value(value: Value) -> RelationalValue {
    match value {
        Value::Null => RelationalValue::Null,
        Value::Integer(value) => RelationalValue::Integer(value),
        Value::Real(value) => RelationalValue::Real(value),
        Value::Text(value) => RelationalValue::Text(value),
        Value::Blob(value) => RelationalValue::Bytes(value),
    }
}
