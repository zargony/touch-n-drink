use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::str::FromStr;
use core::{fmt, num};

/// JSON value conversion error
#[derive(Debug)]
pub struct TryFromValueError;

impl From<num::TryFromIntError> for TryFromValueError {
    fn from(_err: num::TryFromIntError) -> Self {
        Self
    }
}

impl From<num::ParseIntError> for TryFromValueError {
    fn from(_err: num::ParseIntError) -> Self {
        Self
    }
}

impl From<num::ParseFloatError> for TryFromValueError {
    fn from(_err: num::ParseFloatError) -> Self {
        Self
    }
}

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
    Integer(i64),
    Decimal(f64),
    String(String),
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>),
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

impl From<u8> for Value {
    fn from(value: u8) -> Self {
        Self::Integer(i64::from(value))
    }
}

impl From<u16> for Value {
    fn from(value: u16) -> Self {
        Self::Integer(i64::from(value))
    }
}

impl From<u32> for Value {
    fn from(value: u32) -> Self {
        Self::Integer(i64::from(value))
    }
}

impl TryFrom<u64> for Value {
    type Error = TryFromValueError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Ok(Self::Integer(i64::try_from(value)?))
    }
}

impl TryFrom<usize> for Value {
    type Error = TryFromValueError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Ok(Self::Integer(i64::try_from(value)?))
    }
}

impl From<i8> for Value {
    fn from(value: i8) -> Self {
        Self::Integer(i64::from(value))
    }
}

impl From<i16> for Value {
    fn from(value: i16) -> Self {
        Self::Integer(i64::from(value))
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Self {
        Self::Integer(i64::from(value))
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl TryFrom<isize> for Value {
    type Error = TryFromValueError;

    fn try_from(value: isize) -> Result<Self, Self::Error> {
        Ok(Self::Integer(i64::try_from(value)?))
    }
}

impl From<f32> for Value {
    fn from(value: f32) -> Self {
        Self::Decimal(f64::from(value))
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Self::Decimal(value)
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

impl<const N: usize> From<[(String, Value); N]> for Value {
    fn from(value: [(String, Value); N]) -> Self {
        Self::Object(value.into())
    }
}

impl From<BTreeMap<String, Value>> for Value {
    fn from(value: BTreeMap<String, Value>) -> Self {
        Self::Object(value)
    }
}

impl TryFrom<Value> for () {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Null => Ok(()),
            _ => Err(TryFromValueError),
        }
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

impl TryFrom<Value> for u8 {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Integer(n) => Ok(u8::try_from(n)?),
            Value::String(s) => Ok(u8::from_str(&s)?),
            _ => Err(TryFromValueError),
        }
    }
}

impl TryFrom<Value> for u16 {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Integer(n) => Ok(u16::try_from(n)?),
            Value::String(s) => Ok(u16::from_str(&s)?),
            _ => Err(TryFromValueError),
        }
    }
}

impl TryFrom<Value> for u32 {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Integer(n) => Ok(u32::try_from(n)?),
            Value::String(s) => Ok(u32::from_str(&s)?),
            _ => Err(TryFromValueError),
        }
    }
}

impl TryFrom<Value> for u64 {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Integer(n) => Ok(u64::try_from(n)?),
            Value::String(s) => Ok(u64::from_str(&s)?),
            _ => Err(TryFromValueError),
        }
    }
}

impl TryFrom<Value> for usize {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Integer(n) => Ok(usize::try_from(n)?),
            Value::String(s) => Ok(usize::from_str(&s)?),
            _ => Err(TryFromValueError),
        }
    }
}

impl TryFrom<Value> for i8 {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Integer(n) => Ok(i8::try_from(n)?),
            Value::String(s) => Ok(i8::from_str(&s)?),
            _ => Err(TryFromValueError),
        }
    }
}

impl TryFrom<Value> for i16 {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Integer(n) => Ok(i16::try_from(n)?),
            Value::String(s) => Ok(i16::from_str(&s)?),
            _ => Err(TryFromValueError),
        }
    }
}

impl TryFrom<Value> for i32 {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Integer(n) => Ok(i32::try_from(n)?),
            Value::String(s) => Ok(i32::from_str(&s)?),
            _ => Err(TryFromValueError),
        }
    }
}

impl TryFrom<Value> for i64 {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Integer(n) => Ok(n),
            Value::String(s) => Ok(i64::from_str(&s)?),
            _ => Err(TryFromValueError),
        }
    }
}

impl TryFrom<Value> for isize {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Integer(n) => Ok(isize::try_from(n)?),
            Value::String(s) => Ok(isize::from_str(&s)?),
            _ => Err(TryFromValueError),
        }
    }
}

impl TryFrom<Value> for f32 {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            // Easy integer to float conversion (decimal in JSON might be written as integer)
            #[allow(clippy::cast_precision_loss)]
            Value::Integer(n) => Ok(n as f32),
            // Rust Reference: Casting from an f64 to an f32 will produce the closest possible f32
            #[allow(clippy::cast_possible_truncation)]
            Value::Decimal(n) => Ok(n as f32),
            Value::String(s) => Ok(f32::from_str(&s)?),
            _ => Err(TryFromValueError),
        }
    }
}

impl TryFrom<Value> for f64 {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            // Easy integer to float conversion (decimal in JSON might be written as integer)
            #[allow(clippy::cast_precision_loss)]
            Value::Integer(n) => Ok(n as f64),
            Value::Decimal(n) => Ok(n),
            Value::String(s) => Ok(f64::from_str(&s)?),
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

impl TryFrom<Value> for Vec<Value> {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Array(array) => Ok(array),
            _ => Err(TryFromValueError),
        }
    }
}

impl TryFrom<Value> for BTreeMap<String, Value> {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Object(object) => Ok(object),
            _ => Err(TryFromValueError),
        }
    }
}
