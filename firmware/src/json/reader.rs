use super::error::Error;
use super::value::Value;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::iter::Extend;
use core::str::FromStr;
use embedded_io_async::BufRead;

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
    pub fn new(reader: R) -> Self {
        Self { reader, pos: 0 }
    }

    /// Returns a reference to the inner reader wrapped by this reader
    #[allow(dead_code)]
    pub fn get_ref(&self) -> &R {
        &self.reader
    }

    /// Returns a mutable reference to the inner reader wrapped by this reader
    #[allow(dead_code)]
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.reader
    }

    /// Consumes the reader, returning its inner reader
    #[allow(dead_code)]
    pub fn into_inner(self) -> R {
        self.reader
    }

    /// Read and parse type from JSON
    /// Uses the type's `FromJson` implementation to create it by reading JSON from this reader.
    pub async fn read<T: FromJson>(&mut self) -> Result<T, Error<R::Error>> {
        T::from_json(self).await
    }

    /// Read and parse any JSON value
    /// A JSON value of any type is read and returned. The returned type `Value` is an enum that
    /// can contain any JSON value. Note that the value is completely read into memory, so for
    /// large objects or arrays, this may allocate a lot memory. See `read_object` and `read_array`
    /// for memory-optimized streaming read of objects and arrays.
    pub async fn read_any(&mut self) -> Result<Value, Error<R::Error>> {
        self.trim().await?;
        match self.peek().await? {
            b'{' => Ok(Value::Object(Box::pin(self.read()).await?)),
            b'[' => Ok(Value::Array(Box::pin(self.read()).await?)),
            b'"' => Ok(Value::String(self.read().await?)),
            b'0'..=b'9' | b'-' => self.read_number().await,
            b'f' | b't' => Ok(Value::Boolean(self.read().await?)),
            b'n' => Ok(self.read().await?).map(|()| Value::Null),
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

    /// Read and parse JSON number (either integer or decimal)
    pub async fn read_number(&mut self) -> Result<Value, Error<R::Error>> {
        let s = self.read_digits().await?;
        match i64::from_str(&s) {
            Ok(number) => Ok(Value::Integer(number)),
            Err(_) => Ok(Value::Decimal(
                f64::from_str(&s).map_err(|_e| Error::InvalidType)?,
            )),
        }
    }

    /// Read and parse JSON number (decimal)
    pub async fn read_decimal(&mut self) -> Result<f64, Error<R::Error>> {
        let s = self.read_digits().await?;
        f64::from_str(&s).map_err(|_e| Error::InvalidType)
    }

    /// Read and parse JSON number (integer)
    pub async fn read_integer(&mut self) -> Result<i64, Error<R::Error>> {
        let s = self.read_digits().await?;
        i64::from_str(&s).map_err(|_e| Error::InvalidType)
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
        let buf = self.reader.fill_buf().await?;
        match buf.get(self.pos) {
            Some(ch) => Ok(*ch),
            None if self.pos == 0 => Err(Error::Eof),
            None => {
                self.reader.consume(self.pos);
                self.pos = 0;
                let buf = self.reader.fill_buf().await?;
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

    /// Read digits for parsing a number
    async fn read_digits(&mut self) -> Result<String, Error<R::Error>> {
        let mut s = String::new();
        match self.peek().await? {
            ch @ (b'-' | b'0'..=b'9') => {
                self.consume();
                s.push(char::from(ch));
            }
            ch => return Err(Error::unexpected(ch)),
        }
        loop {
            match self.peek().await {
                Ok(ch @ (b'0'..=b'9' | b'.')) => {
                    self.consume();
                    s.push(char::from(ch));
                }
                Ok(_) | Err(Error::Eof) => break Ok(s),
                Err(err) => break Err(err),
            }
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

impl FromJson for usize {
    async fn from_json<R: BufRead>(reader: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        usize::try_from(reader.read_integer().await?).map_err(|_e| Error::NumberTooLarge)
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

impl FromJson for isize {
    async fn from_json<R: BufRead>(reader: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        isize::try_from(reader.read_integer().await?).map_err(|_e| Error::NumberTooLarge)
    }
}

impl FromJson for f32 {
    async fn from_json<R: BufRead>(reader: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        // Rust Reference: Casting from an f64 to an f32 will produce the closest possible f32
        #[allow(clippy::cast_possible_truncation)]
        Ok(reader.read_decimal().await? as f32)
    }
}

impl FromJson for f64 {
    async fn from_json<R: BufRead>(reader: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        reader.read_decimal().await
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
        let mut vec = Self::default();
        reader.read_object(|k, v| vec.extend([(k, v)])).await?;
        Ok(vec)
    }
}

impl FromJson for Value {
    async fn from_json<R: BufRead>(reader: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        reader.read_any().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn reader(s: &str) -> Reader<&[u8]> {
        Reader::new(s.as_bytes())
    }

    macro_rules! assert_read_eq {
        ($json:expr, $method:ident, $value:expr) => {{
            assert_eq!(reader($json).$method().await, $value);
        }};
    }

    #[async_std::test]
    async fn read() {
        #[derive(Debug, Default, PartialEq)]
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
        assert_read_eq!(
            json,
            read,
            Ok(Test {
                foo: "hi".into(),
                bar: 42.0,
                baz: true,
            })
        );
    }

    #[async_std::test]
    async fn read_any() {
        assert_read_eq!("null", read_any, Ok(Value::Null));
        assert_read_eq!("false", read_any, Ok(Value::Boolean(false)));
        assert_read_eq!("123", read_any, Ok(Value::Integer(123)));
        assert_read_eq!("123.456", read_any, Ok(Value::Decimal(123.456)));
        assert_read_eq!("\"hello\"", read_any, Ok(Value::String("hello".into())));
        assert_read_eq!(
            "[1, 2, 3, 4]",
            read_any,
            Ok(Value::Array(vec![
                Value::Integer(1),
                Value::Integer(2),
                Value::Integer(3),
                Value::Integer(4),
            ]))
        );
        assert_read_eq!(
            r#"{"foo": "hi", "bar": 42, "baz": true}"#,
            read_any,
            Ok(Value::Object(vec![
                ("foo".into(), Value::String("hi".into())),
                ("bar".into(), Value::Integer(42)),
                ("baz".into(), Value::Boolean(true)),
            ]))
        );
        assert_read_eq!("buzz", read_any, Err(Error::Unexpected('b')));
    }

    #[async_std::test]
    async fn read_object() {
        let json = r#"{"foo": "hi", "bar": 42, "baz": true}"#;
        let mut values = Vec::new();
        let collect = |k, v| values.push((k, v));
        assert_eq!(reader(json).read_object(collect).await, Ok(()));
        assert_eq!(values.len(), 3);
        assert_eq!(values[0].0, "foo");
        assert_eq!(values[0].1, Value::String("hi".into()));
        assert_eq!(values[1].0, "bar");
        assert_eq!(values[1].1, Value::Integer(42));
        assert_eq!(values[2].0, "baz");
        assert_eq!(values[2].1, Value::Boolean(true));
    }

    #[async_std::test]
    async fn read_array() {
        let json = "[1, 2, 3, 4]";
        let mut values = Vec::new();
        let collect = |v| values.push(v);
        assert_eq!(reader(json).read_array(collect).await, Ok(()));
        assert_eq!(values.len(), 4);
        assert_eq!(values[0], Value::Integer(1));
        assert_eq!(values[1], Value::Integer(2));
        assert_eq!(values[2], Value::Integer(3));
        assert_eq!(values[3], Value::Integer(4));
    }

    #[async_std::test]
    async fn read_string() {
        assert_read_eq!("\"\"", read_string, Ok("".into()));
        assert_read_eq!("\"hello\"", read_string, Ok("hello".into()));
        assert_read_eq!(
            r#""hello \"world\"""#,
            read_string,
            Ok("hello \"world\"".into())
        );
        assert_read_eq!("\"hello", read_string, Err(Error::Eof));
    }

    #[async_std::test]
    async fn read_decimal() {
        assert_read_eq!("0", read_decimal, Ok(0.0));
        assert_read_eq!("123", read_decimal, Ok(123.0));
        assert_read_eq!("-234", read_decimal, Ok(-234.0));
        assert_read_eq!("0.0", read_decimal, Ok(0.0));
        assert_read_eq!("123.456", read_decimal, Ok(123.456));
        assert_read_eq!("-234.567", read_decimal, Ok(-234.567));
        assert_read_eq!("null", read_decimal, Err(Error::Unexpected('n')));
        assert_read_eq!("\"0\"", read_decimal, Err(Error::Unexpected('"')));
    }

    #[async_std::test]
    async fn read_integer() {
        assert_read_eq!("0", read_integer, Ok(0));
        assert_read_eq!("123", read_integer, Ok(123));
        assert_read_eq!("-234", read_integer, Ok(-234));
        assert_read_eq!("0.0", read_integer, Err(Error::InvalidType));
        assert_read_eq!("123.456", read_integer, Err(Error::InvalidType));
        assert_read_eq!("-234.567", read_integer, Err(Error::InvalidType));
        assert_read_eq!("null", read_integer, Err(Error::Unexpected('n')));
        assert_read_eq!("\"0\"", read_integer, Err(Error::Unexpected('"')));
    }

    #[async_std::test]
    async fn read_boolean() {
        assert_read_eq!("false", read_boolean, Ok(false));
        assert_read_eq!("true", read_boolean, Ok(true));
        assert_read_eq!("t", read_boolean, Err(Error::Eof));
        assert_read_eq!("0", read_boolean, Err(Error::Unexpected('0')));
        assert_read_eq!("True", read_boolean, Err(Error::Unexpected('T')));
        assert_read_eq!("1234", read_boolean, Err(Error::Unexpected('1')));
        assert_read_eq!("\"true\"", read_boolean, Err(Error::Unexpected('"')));
    }

    #[async_std::test]
    async fn read_null() {
        assert_read_eq!("null", read_null, Ok(()));
        assert_read_eq!("n", read_null, Err(Error::Eof));
        assert_read_eq!("0", read_null, Err(Error::Unexpected('0')));
        assert_read_eq!("1234", read_null, Err(Error::Unexpected('1')));
        assert_read_eq!("\"null\"", read_null, Err(Error::Unexpected('"')));
    }
}
