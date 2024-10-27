use super::error::Error;
use super::value::Value;
use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
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
    /// A JSON object is read and parsed field by field. The given type is created using its
    /// `Default` implementation and its `FromJsonObject` implementation is called to read each
    /// field's value. This doesn't allocate any memory while reading the object (except for the
    /// current key), so the type's implementation can choose how values are stored.
    pub async fn read_object<C: Default, T: for<'ctx> FromJsonObject<Context<'ctx> = C>>(
        &mut self,
    ) -> Result<T, Error<R::Error>> {
        self.read_object_with_context(&C::default()).await
    }

    /// Read and parse JSON object
    /// Same as `read_object`, but allows to pass an additional context reference to the type's
    /// `FromJsonObject` implementation.
    pub async fn read_object_with_context<T: FromJsonObject>(
        &mut self,
        context: &T::Context<'_>,
    ) -> Result<T, Error<R::Error>> {
        let mut obj = T::default();
        self.expect(b'{').await?;
        loop {
            self.trim().await?;
            let key = match self.peek().await? {
                b'}' => {
                    self.consume();
                    break Ok(obj);
                }
                _ => self.read_string().await?,
            };
            self.trim().await?;
            self.expect(b':').await?;
            self.trim().await?;
            obj.read_next(key, self, context).await?;
            self.trim().await?;
            match self.peek().await? {
                b',' => self.consume(),
                b'}' => (),
                ch => break Err(Error::unexpected(ch)),
            }
        }
    }

    /// Read and parse JSON array
    /// A JSON array is read and parsed element by element. The given type is created using its
    /// `Default` implementation and its `FromJsonArray` implementation is called to read each
    /// element. This doesn't allocate any memory while reading the array, so the type's
    /// implementation can choose how elements are stored.
    pub async fn read_array<C: Default, T: for<'ctx> FromJsonArray<Context<'ctx> = C>>(
        &mut self,
    ) -> Result<T, Error<R::Error>> {
        self.read_array_with_context(&C::default()).await
    }

    /// Read and parse JSON array
    /// Same as `read_array`, but allows to pass an additional context reference to the type's
    /// `FromJsonArray` implementation.
    pub async fn read_array_with_context<T: FromJsonArray>(
        &mut self,
        context: &T::Context<'_>,
    ) -> Result<T, Error<R::Error>> {
        let mut vec = T::default();
        self.expect(b'[').await?;
        loop {
            self.trim().await?;
            match self.peek().await? {
                b']' => {
                    self.consume();
                    break Ok(vec);
                }
                _ => vec.read_next(self, context).await?,
            }
            self.trim().await?;
            match self.peek().await? {
                b',' => self.consume(),
                b']' => (),
                ch => break Err(Error::unexpected(ch)),
            }
        }
    }

    /// Read and parse JSON string
    pub async fn read_string(&mut self) -> Result<String, Error<R::Error>> {
        self.expect(b'"').await?;
        let mut buf = Vec::new();
        loop {
            match self.peek().await? {
                // This is safe to check, even in the middle of a UTF-8 character since UTF-8
                // guarantees that no character encoding is a substring of any other character
                b'\\' => {
                    self.consume();
                    let ch = self.peek().await?;
                    buf.push(ch);
                    self.consume();
                }
                b'"' => {
                    self.consume();
                    let s = match String::from_utf8_lossy(&buf) {
                        // It's safe to use `from_utf8_unchecked` if `from_utf8_lossy` returns
                        // borrowed data (which is valid UTF-8)
                        Cow::Borrowed(_s) => unsafe { String::from_utf8_unchecked(buf) },
                        Cow::Owned(s) => s,
                    };
                    break Ok(s);
                }
                ch => {
                    // OPTIMIZE: Appending each char separately to a string is quite inefficient
                    buf.push(ch);
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

    /// Read and discard any remaining data
    pub async fn discard_to_end(&mut self) -> Result<(), Error<R::Error>> {
        loop {
            match self.reader.fill_buf().await?.len() {
                0 => break,
                len => self.reader.consume(len),
            }
        }
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
    async fn from_json<R: BufRead>(json: &mut Reader<R>) -> Result<Self, Error<R::Error>>;
}

impl FromJson for () {
    async fn from_json<R: BufRead>(json: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        json.read_null().await
    }
}

impl FromJson for bool {
    async fn from_json<R: BufRead>(json: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        json.read_boolean().await
    }
}

impl FromJson for u8 {
    async fn from_json<R: BufRead>(json: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        u8::try_from(json.read_integer().await?).map_err(|_e| Error::NumberTooLarge)
    }
}

impl FromJson for u16 {
    async fn from_json<R: BufRead>(json: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        u16::try_from(json.read_integer().await?).map_err(|_e| Error::NumberTooLarge)
    }
}

impl FromJson for u32 {
    async fn from_json<R: BufRead>(json: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        u32::try_from(json.read_integer().await?).map_err(|_e| Error::NumberTooLarge)
    }
}

impl FromJson for u64 {
    async fn from_json<R: BufRead>(json: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        u64::try_from(json.read_integer().await?).map_err(|_e| Error::NumberTooLarge)
    }
}

impl FromJson for usize {
    async fn from_json<R: BufRead>(json: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        usize::try_from(json.read_integer().await?).map_err(|_e| Error::NumberTooLarge)
    }
}

impl FromJson for i8 {
    async fn from_json<R: BufRead>(json: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        i8::try_from(json.read_integer().await?).map_err(|_e| Error::NumberTooLarge)
    }
}

impl FromJson for i16 {
    async fn from_json<R: BufRead>(json: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        i16::try_from(json.read_integer().await?).map_err(|_e| Error::NumberTooLarge)
    }
}

impl FromJson for i32 {
    async fn from_json<R: BufRead>(json: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        i32::try_from(json.read_integer().await?).map_err(|_e| Error::NumberTooLarge)
    }
}

impl FromJson for i64 {
    async fn from_json<R: BufRead>(json: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        json.read_integer().await
    }
}

impl FromJson for isize {
    async fn from_json<R: BufRead>(json: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        isize::try_from(json.read_integer().await?).map_err(|_e| Error::NumberTooLarge)
    }
}

impl FromJson for f32 {
    async fn from_json<R: BufRead>(json: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        // Rust Reference: Casting from an f64 to an f32 will produce the closest possible f32
        #[allow(clippy::cast_possible_truncation)]
        Ok(json.read_decimal().await? as f32)
    }
}

impl FromJson for f64 {
    async fn from_json<R: BufRead>(json: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        json.read_decimal().await
    }
}

impl FromJson for String {
    async fn from_json<R: BufRead>(json: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        json.read_string().await
    }
}

// FIXME: Unfortunately, a generic `T: FromJsonArray` would be a conflicting implementation
impl<T: FromJson> FromJson for Vec<T> {
    async fn from_json<R: BufRead>(json: &mut Reader<R>) -> Result<Vec<T>, Error<R::Error>> {
        json.read_array().await
    }
}

impl<C: Default, T: for<'ctx> FromJsonObject<Context<'ctx> = C>> FromJson for T {
    async fn from_json<R: BufRead>(json: &mut Reader<R>) -> Result<T, Error<R::Error>> {
        json.read_object().await
    }
}

impl FromJson for Value {
    async fn from_json<R: BufRead>(json: &mut Reader<R>) -> Result<Self, Error<R::Error>> {
        json.read_any().await
    }
}

/// Deserialize from streaming JSON array
/// The given method is called for every element and gets a reader that MUST be used to read the
/// next element.
pub trait FromJsonArray: Sized + Default {
    /// Additional context information passed to deserialization
    type Context<'ctx>: ?Sized;

    /// Read next array element from given JSON reader
    async fn read_next<R: BufRead>(
        &mut self,
        json: &mut Reader<R>,
        context: &Self::Context<'_>,
    ) -> Result<(), Error<R::Error>>;
}

impl<T: FromJson> FromJsonArray for Vec<T> {
    type Context<'ctx> = ();

    async fn read_next<R: BufRead>(
        &mut self,
        json: &mut Reader<R>,
        _context: &Self::Context<'_>,
    ) -> Result<(), Error<R::Error>> {
        let elem = json.read().await?;
        self.push(elem);
        Ok(())
    }
}

/// Deserialize from streaming JSON object
/// The given method is called for every field and gets a reader that MUST be used to read the
/// next value.
pub trait FromJsonObject: Sized + Default {
    /// Additional context information passed to deserialization
    type Context<'ctx>: ?Sized;

    /// Read next object value from given JSON reader
    async fn read_next<R: BufRead>(
        &mut self,
        key: String,
        json: &mut Reader<R>,
        context: &Self::Context<'_>,
    ) -> Result<(), Error<R::Error>>;
}

impl<T: FromJson> FromJsonObject for BTreeMap<String, T> {
    type Context<'ctx> = ();

    async fn read_next<R: BufRead>(
        &mut self,
        key: String,
        json: &mut Reader<R>,
        _context: &Self::Context<'_>,
    ) -> Result<(), Error<R::Error>> {
        let value = json.read().await?;
        self.insert(key, value);
        Ok(())
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

        impl FromJsonObject for Test {
            type Context = ();

            async fn read_next<R: BufRead>(
                &mut self,
                key: String,
                json: &mut Reader<R>,
                _context: &Self::Context<'_>,
            ) -> Result<(), Error<R::Error>> {
                match &*key {
                    "foo" => self.foo = json.read().await?,
                    "bar" => self.bar = json.read().await?,
                    "baz" => self.baz = json.read().await?,
                    _ => _ = json.read_any().await?,
                }
                Ok(())
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
            Ok(Value::Object(BTreeMap::from([
                ("foo".into(), Value::String("hi".into())),
                ("bar".into(), Value::Integer(42)),
                ("baz".into(), Value::Boolean(true)),
            ])))
        );
        assert_read_eq!("buzz", read_any, Err(Error::Unexpected('b')));
    }

    #[async_std::test]
    async fn read_object() {
        assert_read_eq!("{}", read_object, Ok(BTreeMap::<String, String>::new()));
        assert_read_eq!(
            r#"{"foo": "hi", "bar": 42, "baz": true}"#,
            read_object,
            Ok(BTreeMap::from([
                ("foo".to_string(), Value::String("hi".into())),
                ("bar".to_string(), Value::Integer(42)),
                ("baz".to_string(), Value::Boolean(true)),
            ]))
        );
    }

    #[async_std::test]
    async fn read_array() {
        assert_read_eq!("[]", read_array, Ok(Vec::<u32>::new()));
        assert_read_eq!("[1, 2, 3, 4]", read_array, Ok(vec![1, 2, 3, 4]));
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
