use turso::Value;

pub fn new_query_params() -> Vec<(String, Value)> {
    Vec::new()
}

pub fn text_param(key: &str, value: String) -> (String, Value) {
    (key.to_string(), Value::Text(value))
}

pub fn opt_text_param(key: &str, value: Option<String>) -> (String, Value) {
    match value {
        Some(v) => (key.to_string(), Value::Text(v)),
        None => (key.to_string(), Value::Null),
    }
}

pub fn integer_param(key: &str, value: i64) -> (String, Value) {
    (key.to_string(), Value::Integer(value))
}

pub fn opt_integer_param(key: &str, value: Option<i64>) -> (String, Value) {
    match value {
        Some(v) => (key.to_string(), Value::Integer(v)),
        None => (key.to_string(), Value::Null),
    }
}
