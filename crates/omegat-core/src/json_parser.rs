//! Java `org.omegat.util.JsonParser` (Nashorn `JSON.parse`).

use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonError {
    Empty,
    Invalid(String),
}

pub fn parse(input: &str) -> Result<Value, JsonError> {
    if input.is_empty() {
        return Err(JsonError::Empty);
    }
    serde_json::from_str(input).map_err(|e| JsonError::Invalid(e.to_string()))
}

pub fn is_object(v: &Value) -> bool {
    v.is_object()
}

pub fn is_array(v: &Value) -> bool {
    v.is_array()
}
