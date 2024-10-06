use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

/// JSON value conversion error
#[derive(Debug)]
pub struct TryFromValueError;

impl fmt::Display for TryFromValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "JSON value conversion error")
    }
}

/// JSON value
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Boolean(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

impl From<()> for Value {
    fn from(_value: ()) -> Self {
        Self::Null
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Self::Number(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&[Value]> for Value {
    fn from(value: &[Value]) -> Self {
        Self::Array(value.into())
    }
}

impl From<Vec<Value>> for Value {
    fn from(value: Vec<Value>) -> Self {
        Self::Array(value)
    }
}

impl From<&[(String, Value)]> for Value {
    fn from(value: &[(String, Value)]) -> Self {
        Self::Object(value.into())
    }
}

impl From<Vec<(String, Value)>> for Value {
    fn from(value: Vec<(String, Value)>) -> Self {
        Self::Object(value)
    }
}

impl TryFrom<Value> for bool {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Boolean(b) => Ok(b),
            _ => Err(TryFromValueError),
        }
    }
}

impl TryFrom<Value> for f64 {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Number(n) => Ok(n),
            _ => Err(TryFromValueError),
        }
    }
}

impl TryFrom<Value> for String {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::String(s) => Ok(s),
            _ => Err(TryFromValueError),
        }
    }
}
