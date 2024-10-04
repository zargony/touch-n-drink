use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use core::iter::Extend;
use embedded_io_async::BufRead;

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

impl TryFrom<Value> for bool {
    type Error = Error<()>;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Boolean(b) => Ok(b),
            _ => Err(Error::InvalidType),
        }
    }
}

impl TryFrom<Value> for f64 {
    type Error = Error<()>;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Number(n) => Ok(n),
            _ => Err(Error::InvalidType),
        }
    }
}

impl TryFrom<Value> for String {
    type Error = Error<()>;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::String(s) => Ok(s),
            _ => Err(Error::InvalidType),
        }
    }
}

/// JSON reader error
#[derive(Debug, PartialEq)]
pub enum Error<E> {
    Read(E),
    Eof,
    Unexpected(char),
    NumberTooLarge,
    InvalidType,
}

impl<E: fmt::Display> fmt::Display for Error<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(err) => write!(f, "Read error: {err}"),
            Self::Eof => write!(f, "Premature EOF"),
            Self::Unexpected(ch) => write!(f, "Unexpected `{ch}`"),
            Self::NumberTooLarge => write!(f, "Number too large"),
            Self::InvalidType => write!(f, "Invalid type"),
        }
    }
}

impl<E> Error<E> {
    fn unexpected(ch: u8) -> Self {
        Self::Unexpected(char::from(ch))
    }
}

/// Asynchronous streaming JSON reader
///
/// This JSON reader reads from a wrapped asynchronous byte reader and parses JSON without storing
/// any JSON source in memory (though the underlying byte reader typically has a memory buffer).
#[derive(Debug)]
pub struct Reader<R> {
    reader: R,
    pos: usize,
}

impl<R: BufRead> Reader<R> {
    /// Create JSON reader
    #[allow(dead_code)]
    pub fn new(reader: R) -> Self {
        Self { reader, pos: 0 }
    }

    /// Read and parse type from JSON
    /// Uses the type's `FromJson` implementation to create it by reading JSON from this reader.
    #[allow(dead_code)]
    pub async fn read<T: FromJson>(&mut self) -> Result<T, Error<R::Error>> {
        T::from_json(self).await
    }

    /// Read and parse any JSON value
    /// A JSON value of any type is read and returned. The returned type `Value` is an enum that
    /// can contain any JSON value. Note that the value is completely read into memory, so for
    /// large objects or arrays, this may allocate a lot memory. See `read_object` and `read_array`
    /// for memory-optimized streaming read of objects and arrays.
    #[allow(dead_code)]
    pub async fn read_any(&mut self) -> Result<Value, Error<R::Error>> {
        self.trim().await?;
        match self.peek().await? {
            b'{' => Ok(Value::Object(Box::pin(self.read_object_value()).await?)),
            b'[' => Ok(Value::Array(Box::pin(self.read_array_value()).await?)),
            b'"' => Ok(Value::String(self.read_string().await?)),
            b'0'..=b'9' | b'-' => Ok(Value::Number(self.read_number().await?)),
            b'f' | b't' => Ok(Value::Boolean(self.read_boolean().await?)),
            b'n' => Ok(self.read_null().await?).map(|()| Value::Null),
            ch => Err(Error::unexpected(ch)),
        }
    }

    /// Read and parse JSON object
    /// A JSON object is read and parsed key by key. The given closure is called for every key
    /// value pair as it is parsed. This doesn't need to allocate memory for all keys and values of
    /// the object, just for one key value pair at a time.
    pub async fn read_object(
        &mut self,
        mut f: impl FnMut(String, Value),
    ) -> Result<(), Error<R::Error>> {
        self.trim().await?;
        self.expect(b'{').await?;
        loop {
            self.trim().await?;
            let key = self.read_string().await?;
            self.expect(b':').await?;
            let value = self.read_any().await?;
            f(key, value);
            self.trim().await?;
            match self.peek().await? {
                b',' => self.consume(),
                b'}' => {
                    self.consume();
                    break Ok(());
                }
                ch => break Err(Error::unexpected(ch)),
            }
        }
    }

    /// Read and parse JSON object
    /// A JSON object is read and returned. The returned type `Value::Object` contains all keys
    /// and values. Note that the object is completely read into memory, so for large objects,
    /// this may allocate a lot memory. See `read_object` for memory-optimized streaming read of
    /// objects.
    async fn read_object_value(&mut self) -> Result<Vec<(String, Value)>, Error<R::Error>> {
        let mut vec = Vec::new();
        self.read_object(|k, v| vec.push((k, v))).await?;
        Ok(vec)
    }

    /// Read and parse JSON array
    /// A JSON array is read and parsed element by element. The given closure is called for every
    /// element as it is parsed. This doesn't need to allocate memory for all elements of the
    /// array, just for one element at a time.
    pub async fn read_array(&mut self, mut f: impl FnMut(Value)) -> Result<(), Error<R::Error>> {
        self.trim().await?;
        self.expect(b'[').await?;
        loop {
            let elem = self.read_any().await?;
            f(elem);
            self.trim().await?;
            match self.peek().await? {
                b',' => self.consume(),
                b']' => {
                    self.consume();
                    break Ok(());
                }
                ch => break Err(Error::unexpected(ch)),
            }
        }
    }

    /// Read and parse JSON array
    /// A JSON array is read and returned. The returned type `Value::Array` contains all elements.
    /// Note that the array is completely read into memory, so for large arrays, this may allocate
    /// a lot memory. See `read_array` for memory-optimized streaming read of objects.
    async fn read_array_value(&mut self) -> Result<Vec<Value>, Error<R::Error>> {
        let mut vec = Vec::new();
        self.read_array(|elem| vec.push(elem)).await?;
        Ok(vec)
    }

    /// Read and parse JSON string
    pub async fn read_string(&mut self) -> Result<String, Error<R::Error>> {
        self.expect(b'"').await?;
        let mut s = String::new();
        loop {
            match self.peek().await? {
                b'\\' => {
                    self.consume();
                    let ch = self.peek().await?;
                    s.push(char::from(ch));
                    self.consume();
                }
                b'"' => {
                    self.consume();
                    break Ok(s);
                }
                ch => {
                    // OPTIMIZE: Appending each char separately to a string is quite inefficient
                    s.push(char::from(ch));
                    self.consume();
                }
            }
        }
    }

    /// Read and parse JSON number (decimal)
    pub async fn read_number(&mut self) -> Result<f64, Error<R::Error>> {
        let negative = match self.peek().await? {
            b'-' => {
                self.consume();
                true
            }
            b'0'..=b'9' => false,
            ch => return Err(Error::unexpected(ch)),
        };
        let mut number: f64 = 0.0;
        let mut decimal: f64 = 0.0;
        loop {
            match self.peek().await {
                Ok(ch @ b'0'..=b'9') => {
                    self.consume();
                    let mut value = f64::from(ch - b'0');
                    if decimal == 0.0 {
                        number *= 10.0;
                    } else {
                        value *= decimal;
                        decimal /= 10.0;
                    }
                    if negative {
                        number -= value;
                    } else {
                        number += value;
                    }
                }
                Ok(b'.') => {
                    self.consume();
                    decimal = 0.1;
                }
                Ok(_) | Err(Error::Eof) => break Ok(number),
                Err(err) => break Err(err),
            }
        }
    }

    /// Read and parse JSON number (integer)
    pub async fn read_integer(&mut self) -> Result<i64, Error<R::Error>> {
        let negative = match self.peek().await? {
            b'-' => {
                self.consume();
                true
            }
            b'0'..=b'9' => false,
            ch => return Err(Error::unexpected(ch)),
        };
        let mut number: i64 = 0;
        loop {
            match self.peek().await {
                Ok(ch @ b'0'..=b'9') => {
                    self.consume();
                    let value = i64::from(ch - b'0');
                    number = number.checked_mul(10).ok_or(Error::NumberTooLarge)?;
                    if negative {
                        number = number.checked_sub(value).ok_or(Error::NumberTooLarge)?;
                    } else {
                        number = number.checked_add(value).ok_or(Error::NumberTooLarge)?;
                    }
                }
                Ok(b'.') => break Err(Error::unexpected(b'.')),
                Ok(_) | Err(Error::Eof) => break Ok(number),
                Err(err) => break Err(err),
            }
        }
    }

    /// Read and parse JSON boolean
    pub async fn read_boolean(&mut self) -> Result<bool, Error<R::Error>> {
        match self.peek().await? {
            b'f' => {
                self.expect(b'f').await?;
                self.expect(b'a').await?;
                self.expect(b'l').await?;
                self.expect(b's').await?;
                self.expect(b'e').await?;
                Ok(false)
            }
            b't' => {
                self.expect(b't').await?;
                self.expect(b'r').await?;
                self.expect(b'u').await?;
                self.expect(b'e').await?;
                Ok(true)
            }
            ch => Err(Error::unexpected(ch)),
        }
    }

    /// Read and parse JSON null
    pub async fn read_null(&mut self) -> Result<(), Error<R::Error>> {
        self.expect(b'n').await?;
        self.expect(b'u').await?;
        self.expect(b'l').await?;
        self.expect(b'l').await?;
        Ok(())
    }
}

impl<R: BufRead> Reader<R> {
    /// Peek next character from reader
    async fn peek(&mut self) -> Result<u8, Error<R::Error>> {
        // OPTIMIZE: Minimize calls to fill_buf by keeping a local reference (but: lifetime issues)
        let buf = self.reader.fill_buf().await.map_err(Error::Read)?;
        match buf.get(self.pos) {
            Some(ch) => Ok(*ch),
            None if self.pos == 0 => Err(Error::Eof),
            None => {
                self.reader.consume(self.pos);
                self.pos = 0;
                let buf = self.reader.fill_buf().await.map_err(Error::Read)?;
                match buf.first() {
                    Some(ch) => Ok(*ch),
                    None => Err(Error::Eof),
                }
            }
        }
    }

    /// Consume one character
    fn consume(&mut self) {
        self.pos += 1;
    }

    /// Skip whitespace and peek next character from reader
    async fn trim(&mut self) -> Result<(), Error<R::Error>> {
        loop {
            match self.peek().await? {
                ch if ch.is_ascii_whitespace() => self.consume(),
                _ => break Ok(()),
            }
        }
    }

    /// Expect the given character
    async fn expect(&mut self, expected: u8) -> Result<(), Error<R::Error>> {
        match self.peek().await? {
            ch if ch == expected => {
                self.consume();
                Ok(())
            }
            ch => Err(Error::unexpected(ch)),
        }
    }
}

/// Deserialize from streaming JSON
pub trait FromJson: Sized {
    /// Deserialize this type using the given JSON reader
    async fn from_json<R: BufRead>(reader: &mut Reader<R>) -> Result<Self, Error<R::Error>>;
}

impl FromJson for () {
    async fn from_json<R: BufRead>(reader: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        reader.read_null().await
    }
}

impl FromJson for bool {
    async fn from_json<R: BufRead>(reader: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        reader.read_boolean().await
    }
}

impl FromJson for u8 {
    async fn from_json<R: BufRead>(reader: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        u8::try_from(reader.read_integer().await?).map_err(|_e| Error::NumberTooLarge)
    }
}

impl FromJson for u16 {
    async fn from_json<R: BufRead>(reader: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        u16::try_from(reader.read_integer().await?).map_err(|_e| Error::NumberTooLarge)
    }
}

impl FromJson for u32 {
    async fn from_json<R: BufRead>(reader: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        u32::try_from(reader.read_integer().await?).map_err(|_e| Error::NumberTooLarge)
    }
}

impl FromJson for u64 {
    async fn from_json<R: BufRead>(reader: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        u64::try_from(reader.read_integer().await?).map_err(|_e| Error::NumberTooLarge)
    }
}

impl FromJson for i8 {
    async fn from_json<R: BufRead>(reader: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        i8::try_from(reader.read_integer().await?).map_err(|_e| Error::NumberTooLarge)
    }
}

impl FromJson for i16 {
    async fn from_json<R: BufRead>(reader: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        i16::try_from(reader.read_integer().await?).map_err(|_e| Error::NumberTooLarge)
    }
}

impl FromJson for i32 {
    async fn from_json<R: BufRead>(reader: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        i32::try_from(reader.read_integer().await?).map_err(|_e| Error::NumberTooLarge)
    }
}

impl FromJson for i64 {
    async fn from_json<R: BufRead>(reader: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        reader.read_integer().await
    }
}

impl FromJson for f32 {
    async fn from_json<R: BufRead>(reader: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        #[allow(clippy::cast_possible_truncation)]
        Ok(reader.read_number().await? as f32)
    }
}

impl FromJson for f64 {
    async fn from_json<R: BufRead>(reader: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        reader.read_number().await
    }
}

impl FromJson for String {
    async fn from_json<R: BufRead>(reader: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        reader.read_string().await
    }
}

impl<T: Default + Extend<Value>> FromJson for T {
    async fn from_json<R: BufRead>(reader: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        let mut vec = Self::default();
        reader.read_array(|elem| vec.extend([elem])).await?;
        Ok(vec)
    }
}

impl FromJson for Vec<(String, Value)> {
    async fn from_json<R: BufRead>(reader: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        reader.read_object_value().await
    }
}

// FIXME: Generic implementation for `Extend<(String, Value)>` conflicts with `Extend<String>` above
// impl<T: Default + Extend<(String, Value)>> FromJson for T {
//     async fn from_json<R: BufRead>(reader: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
//         let mut vec = Self::default();
//         reader.read_object(|k, v| vec.extend([(k, v)])).await?;
//         Ok(vec)
//     }
// }

impl FromJson for Value {
    async fn from_json<R: BufRead>(reader: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        reader.read_any().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(s: &str) -> Reader<&[u8]> {
        Reader::new(s.as_bytes())
    }

    #[async_std::test]
    async fn read() {
        #[derive(Debug, Default)]
        struct Test {
            foo: String,
            bar: f64,
            baz: bool,
        }

        impl FromJson for Test {
            async fn from_json<R: BufRead>(
                reader: &mut Reader<R>,
            ) -> Result<Self, Error<R::Error>> {
                let mut test = Self::default();
                reader
                    .read_object(|k, v| match k.as_str() {
                        "foo" => test.foo = v.try_into().unwrap(),
                        "bar" => test.bar = v.try_into().unwrap(),
                        "baz" => test.baz = v.try_into().unwrap(),
                        _ => (),
                    })
                    .await?;
                Ok(test)
            }
        }

        let json = r#"{"foo": "hi", "bar": 42, "baz": true}"#;
        let test: Test = r(json).read().await.unwrap();
        assert_eq!(test.foo, "hi");
        assert_eq!(test.bar, 42.0);
        assert_eq!(test.baz, true);
    }

    #[async_std::test]
    async fn read_any() {
        assert_eq!(r("null").read_any().await, Ok(Value::Null));
        assert_eq!(r("false").read_any().await, Ok(Value::Boolean(false)));
        assert_eq!(r("123.456").read_any().await, Ok(Value::Number(123.456)));
        assert_eq!(
            r(r#""hello""#).read_any().await,
            Ok(Value::String("hello".into()))
        );
        assert_eq!(r("buzz").read_any().await, Err(Error::Unexpected('b')));
    }

    #[async_std::test]
    async fn read_object() {
        let json = r#"{"foo": "hi", "bar": 42, "baz": true}"#;
        let mut values = Vec::new();
        let collect = |k, v| values.push((k, v));
        assert_eq!(r(json).read_object(collect).await, Ok(()));
        assert_eq!(values.len(), 3);
        assert_eq!(values[0].0, "foo");
        assert_eq!(values[0].1, Value::String("hi".into()));
        assert_eq!(values[1].0, "bar");
        assert_eq!(values[1].1, Value::Number(42.0));
        assert_eq!(values[2].0, "baz");
        assert_eq!(values[2].1, Value::Boolean(true));
    }

    #[async_std::test]
    async fn read_array() {
        let json = "[1, 2, 3, 4]";
        let mut values = Vec::new();
        let collect = |v| values.push(v);
        assert_eq!(r(json).read_array(collect).await, Ok(()));
        assert_eq!(values.len(), 4);
        assert_eq!(values[0], Value::Number(1.0));
        assert_eq!(values[1], Value::Number(2.0));
        assert_eq!(values[2], Value::Number(3.0));
        assert_eq!(values[3], Value::Number(4.0));
    }

    #[async_std::test]
    async fn read_string() {
        assert_eq!(r(r#""""#).read_string().await.unwrap(), "");
        assert_eq!(r(r#""hello""#).read_string().await.unwrap(), "hello");
        assert_eq!(
            r(r#""hello \"world\"""#).read_string().await.unwrap(),
            r#"hello "world""#
        );
        assert_eq!(r(r#""hello"#).read_string().await, Err(Error::Eof));
    }

    #[async_std::test]
    async fn read_number() {
        assert_eq!(r("0").read_number().await, Ok(0.0));
        assert_eq!(r("123").read_number().await, Ok(123.0));
        assert_eq!(r("-234").read_number().await, Ok(-234.0));
        assert_eq!(r("0.0").read_number().await, Ok(0.0));
        assert_eq!(r("123.456").read_number().await, Ok(123.456));
        assert_eq!(r("-234.567").read_number().await, Ok(-234.567));
        assert_eq!(r("null").read_number().await, Err(Error::Unexpected('n')));
        assert_eq!(r(r#""0""#).read_number().await, Err(Error::Unexpected('"')));
    }

    #[async_std::test]
    async fn read_integer() {
        assert_eq!(r("0").read_integer().await, Ok(0));
        assert_eq!(r("123").read_integer().await, Ok(123));
        assert_eq!(r("-234").read_integer().await, Ok(-234));
        assert_eq!(r("null").read_integer().await, Err(Error::Unexpected('n')));
        assert_eq!(
            r("123.456").read_integer().await,
            Err(Error::Unexpected('.'))
        );
        assert_eq!(
            r(r#""0""#).read_integer().await,
            Err(Error::Unexpected('"'))
        );
    }

    #[async_std::test]
    async fn read_boolean() {
        assert_eq!(r("false").read_boolean().await, Ok(false));
        assert_eq!(r("true").read_boolean().await, Ok(true));
        assert_eq!(r("t").read_boolean().await, Err(Error::Eof));
        assert_eq!(r("0").read_boolean().await, Err(Error::Unexpected('0')));
        assert_eq!(r("True").read_boolean().await, Err(Error::Unexpected('T')));
        assert_eq!(r("1234").read_boolean().await, Err(Error::Unexpected('1')));
        assert_eq!(
            r(r#""true""#).read_boolean().await,
            Err(Error::Unexpected('"'))
        );
    }

    #[async_std::test]
    async fn read_null() {
        assert_eq!(r("null").read_null().await, Ok(()));
        assert_eq!(r("n").read_null().await, Err(Error::Eof));
        assert_eq!(r("0").read_null().await, Err(Error::Unexpected('0')));
        assert_eq!(r("1234").read_null().await, Err(Error::Unexpected('1')));
        assert_eq!(
            r(r#""null""#).read_null().await,
            Err(Error::Unexpected('"'))
        );
    }
}
