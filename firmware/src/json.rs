use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;
use core::iter::Extend;
use embedded_io_async::{BufRead, Write};

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

impl From<Vec<Value>> for Value {
    fn from(value: Vec<Value>) -> Self {
        Self::Array(value)
    }
}

impl From<Vec<(String, Value)>> for Value {
    fn from(value: Vec<(String, Value)>) -> Self {
        Self::Object(value)
    }
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

/// JSON reader/writer error
#[derive(Debug, PartialEq)]
pub enum Error<E> {
    Io(E),
    Eof,
    Unexpected(char),
    NumberTooLarge,
    InvalidType,
}

impl<E: embedded_io_async::Error> From<E> for Error<E> {
    fn from(err: E) -> Self {
        Self::Io(err)
    }
}

impl<E: fmt::Display> fmt::Display for Error<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
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
            b'0'..=b'9' | b'-' => Ok(Value::Number(self.read().await?)),
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

/// Asynchronous streaming JSON writer
///
/// This JSON writer writes to a wrapped asynchronous byte writer and creates JSON without storing
/// any JSON in memory.
#[derive(Debug)]
pub struct Writer<W> {
    writer: W,
}

impl<W: Write> Writer<W> {
    /// Create JSON writer
    #[allow(dead_code)]
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    /// Returns a reference to the inner writer wrapped by this writer
    #[allow(dead_code)]
    pub fn get_ref(&self) -> &W {
        &self.writer
    }

    /// Returns a mutable reference to the inner writer wrapped by this writer
    #[allow(dead_code)]
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    /// Consumes the writer, returning its inner writer
    #[allow(dead_code)]
    pub fn into_inner(self) -> W {
        self.writer
    }

    /// Write type to JSON
    /// Uses the type's `ToJson` implementation to write JSON to this reader.
    pub async fn write<T: ToJson>(&mut self, value: T) -> Result<(), Error<W::Error>> {
        value.to_json(self).await
    }

    /// Write any JSON value
    pub async fn write_any(&mut self, value: &Value) -> Result<(), Error<W::Error>> {
        match value {
            Value::Object(object) => Box::pin(self.write_object(object)).await,
            Value::Array(array) => Box::pin(self.write_array(array)).await,
            Value::String(string) => self.write_string(string).await,
            Value::Number(number) => self.write_number(*number).await,
            Value::Boolean(boolean) => self.write_boolean(*boolean).await,
            Value::Null => self.write_null().await,
        }
    }

    /// Write JSON object
    pub async fn write_object<'a, K, V, I>(&mut self, iter: I) -> Result<(), Error<W::Error>>
    where
        K: AsRef<str> + 'a,
        V: ToJson + 'a,
        I: IntoIterator<Item = &'a (K, V)>,
    {
        self.writer.write_all(b"{").await?;
        for (i, (k, v)) in iter.into_iter().enumerate() {
            if i > 0 {
                self.writer.write_all(b", ").await?;
            }
            self.write_string(k.as_ref()).await?;
            self.writer.write_all(b": ").await?;
            self.write(v).await?;
        }
        self.writer.write_all(b"}").await?;
        Ok(())
    }

    /// Write JSON array
    pub async fn write_array<'a, T, I>(&mut self, iter: I) -> Result<(), Error<W::Error>>
    where
        T: ToJson + 'a,
        I: IntoIterator<Item = T>,
    {
        self.writer.write_all(b"[").await?;
        for (i, elem) in iter.into_iter().enumerate() {
            if i > 0 {
                self.writer.write_all(b", ").await?;
            }
            self.write(elem).await?;
        }
        self.writer.write_all(b"]").await?;
        Ok(())
    }

    /// Write JSON string
    pub async fn write_string(&mut self, value: &str) -> Result<(), Error<W::Error>> {
        self.writer.write_all(b"\"").await?;
        // OPTIMIZE: Writing each char separately to a writer is quite inefficient
        for ch in value.escape_default() {
            self.writer.write_all(&[ch as u8]).await?;
        }
        self.writer.write_all(b"\"").await?;
        Ok(())
    }

    /// Write JSON number
    pub async fn write_number(&mut self, value: f64) -> Result<(), Error<W::Error>> {
        let buf = value.to_string();
        self.writer.write_all(buf.as_bytes()).await?;
        Ok(())
    }

    /// Write JSON integer
    pub async fn write_integer(&mut self, value: i64) -> Result<(), Error<W::Error>> {
        let buf = value.to_string();
        self.writer.write_all(buf.as_bytes()).await?;
        Ok(())
    }

    /// Write JSON boolean
    pub async fn write_boolean(&mut self, value: bool) -> Result<(), Error<W::Error>> {
        self.writer
            .write_all(if value { b"true" } else { b"false" })
            .await?;
        Ok(())
    }

    /// Write JSON null
    pub async fn write_null(&mut self) -> Result<(), Error<W::Error>> {
        self.writer.write_all(b"null").await?;
        Ok(())
    }
}

/// Serialize to streaming JSON
pub trait ToJson {
    /// Serialize this type using the given JSON writer
    async fn to_json<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), Error<W::Error>>;
}

impl ToJson for () {
    async fn to_json<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), Error<W::Error>> {
        writer.write_null().await
    }
}

impl ToJson for bool {
    async fn to_json<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), Error<W::Error>> {
        writer.write_boolean(*self).await
    }
}

impl ToJson for u8 {
    async fn to_json<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), Error<W::Error>> {
        writer.write_integer(i64::from(*self)).await
    }
}

impl ToJson for u16 {
    async fn to_json<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), Error<W::Error>> {
        writer.write_integer(i64::from(*self)).await
    }
}

impl ToJson for u32 {
    async fn to_json<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), Error<W::Error>> {
        writer.write_integer(i64::from(*self)).await
    }
}

impl ToJson for u64 {
    async fn to_json<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), Error<W::Error>> {
        writer
            .write_integer(i64::try_from(*self).map_err(|_e| Error::NumberTooLarge)?)
            .await
    }
}

impl ToJson for i8 {
    async fn to_json<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), Error<W::Error>> {
        writer.write_integer(i64::from(*self)).await
    }
}

impl ToJson for i16 {
    async fn to_json<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), Error<W::Error>> {
        writer.write_integer(i64::from(*self)).await
    }
}

impl ToJson for i32 {
    async fn to_json<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), Error<W::Error>> {
        writer.write_integer(i64::from(*self)).await
    }
}

impl ToJson for i64 {
    async fn to_json<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), Error<W::Error>> {
        writer.write_integer(*self).await
    }
}

impl ToJson for f32 {
    async fn to_json<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), Error<W::Error>> {
        #[allow(clippy::cast_lossless)]
        writer.write_number(*self as f64).await
    }
}

impl ToJson for f64 {
    async fn to_json<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), Error<W::Error>> {
        writer.write_number(*self).await
    }
}

impl ToJson for str {
    async fn to_json<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), Error<W::Error>> {
        writer.write_string(self).await
    }
}

impl ToJson for String {
    async fn to_json<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), Error<W::Error>> {
        writer.write_string(self).await
    }
}

impl<T: ToJson> ToJson for [T] {
    async fn to_json<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), Error<W::Error>> {
        writer.write_array(self).await
    }
}

impl<T: ToJson> ToJson for [(&str, T)] {
    async fn to_json<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), Error<W::Error>> {
        writer.write_object(self).await
    }
}

impl<T: ToJson> ToJson for [(String, T)] {
    async fn to_json<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), Error<W::Error>> {
        writer.write_object(self).await
    }
}

impl ToJson for Value {
    async fn to_json<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), Error<W::Error>> {
        writer.write_any(self).await
    }
}

impl<T: ToJson + ?Sized> ToJson for &T {
    async fn to_json<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), Error<W::Error>> {
        (**self).to_json(writer).await
    }
}

impl<T: ToJson + ?Sized> ToJson for &mut T {
    async fn to_json<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), Error<W::Error>> {
        (**self).to_json(writer).await
    }
}

impl<T: ToJson + ?Sized> ToJson for Box<T> {
    async fn to_json<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), Error<W::Error>> {
        (**self).to_json(writer).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::{LinkedList, VecDeque};
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
        assert_read_eq!("123.456", read_any, Ok(Value::Number(123.456)));
        assert_read_eq!("\"hello\"", read_any, Ok(Value::String("hello".into())));
        assert_read_eq!(
            "[1, 2, 3, 4]",
            read_any,
            Ok(Value::Array(vec![
                Value::Number(1.0),
                Value::Number(2.0),
                Value::Number(3.0),
                Value::Number(4.0),
            ]))
        );
        assert_read_eq!(
            r#"{"foo": "hi", "bar": 42, "baz": true}"#,
            read_any,
            Ok(Value::Object(vec![
                ("foo".into(), Value::String("hi".into())),
                ("bar".into(), Value::Number(42.0)),
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
        assert_eq!(values[1].1, Value::Number(42.0));
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
        assert_eq!(values[0], Value::Number(1.0));
        assert_eq!(values[1], Value::Number(2.0));
        assert_eq!(values[2], Value::Number(3.0));
        assert_eq!(values[3], Value::Number(4.0));
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
    async fn read_number() {
        assert_read_eq!("0", read_number, Ok(0.0));
        assert_read_eq!("123", read_number, Ok(123.0));
        assert_read_eq!("-234", read_number, Ok(-234.0));
        assert_read_eq!("0.0", read_number, Ok(0.0));
        assert_read_eq!("123.456", read_number, Ok(123.456));
        assert_read_eq!("-234.567", read_number, Ok(-234.567));
        assert_read_eq!("null", read_number, Err(Error::Unexpected('n')));
        assert_read_eq!("\"0\"", read_number, Err(Error::Unexpected('"')));
    }

    #[async_std::test]
    async fn read_integer() {
        assert_read_eq!("0", read_integer, Ok(0));
        assert_read_eq!("123", read_integer, Ok(123));
        assert_read_eq!("-234", read_integer, Ok(-234));
        assert_read_eq!("null", read_integer, Err(Error::Unexpected('n')));
        assert_read_eq!("123.456", read_integer, Err(Error::Unexpected('.')));
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

    fn writer() -> Writer<Vec<u8>> {
        Writer::new(Vec::new())
    }

    macro_rules! assert_write_eq {
        ($method:ident, $($value:expr)?, $json:expr) => {{
            let mut writer = writer();
            let res = writer.$method($($value)?).await;
            let json = String::from_utf8(writer.into_inner()).unwrap();
            assert_eq!(res.map(|()| json.as_str()), $json)
        }};
    }

    #[async_std::test]
    async fn write() {
        #[derive(Debug, Default)]
        struct Test {
            foo: String,
            bar: f64,
            baz: bool,
        }

        impl ToJson for Test {
            async fn to_json<W: Write>(
                &self,
                writer: &mut Writer<W>,
            ) -> Result<(), Error<W::Error>> {
                writer
                    .write_object(&[
                        ("foo", Value::String(self.foo.clone())),
                        ("bar", Value::Number(self.bar)),
                        ("baz", Value::Boolean(self.baz)),
                    ])
                    .await
            }
        }

        assert_write_eq!(
            write,
            &Test {
                foo: "hi".into(),
                bar: 42.0,
                baz: true,
            },
            Ok(r#"{"foo": "hi", "bar": 42, "baz": true}"#)
        );
    }

    #[async_std::test]
    async fn write_any() {
        assert_write_eq!(write_any, &Value::Null, Ok("null"));
        assert_write_eq!(write_any, &Value::Boolean(false), Ok("false"));
        assert_write_eq!(write_any, &Value::Number(123.456), Ok("123.456"));
        assert_write_eq!(write_any, &Value::String("hello".into()), Ok("\"hello\""));
    }

    #[async_std::test]
    async fn write_object() {
        assert_write_eq!(
            write_object,
            &[
                ("foo", Value::String("hi".into())),
                ("bar", Value::Number(42.0)),
                ("baz", Value::Boolean(true))
            ],
            Ok(r#"{"foo": "hi", "bar": 42, "baz": true}"#)
        );
        assert_write_eq!(
            write_object,
            &vec![
                ("foo", Value::String("hi".into())),
                ("bar", Value::Number(42.0)),
                ("baz", Value::Boolean(true))
            ],
            Ok(r#"{"foo": "hi", "bar": 42, "baz": true}"#)
        );
    }

    #[async_std::test]
    async fn write_array() {
        assert_write_eq!(write_array, [1, 2, 3, 4], Ok("[1, 2, 3, 4]"));
        assert_write_eq!(write_array, &[1, 2, 3, 4], Ok("[1, 2, 3, 4]"));
        assert_write_eq!(write_array, vec![1, 2, 3, 4], Ok("[1, 2, 3, 4]"));
        assert_write_eq!(
            write_array,
            LinkedList::from([1, 2, 3, 4]),
            Ok("[1, 2, 3, 4]")
        );
        assert_write_eq!(
            write_array,
            VecDeque::from([1, 2, 3, 4]),
            Ok("[1, 2, 3, 4]")
        );
    }

    #[async_std::test]
    async fn write_string() {
        assert_write_eq!(write_string, "", Ok("\"\""));
        assert_write_eq!(write_string, "hello", Ok("\"hello\""));
        assert_write_eq!(write_string, "hello \"world\"", Ok(r#""hello \"world\"""#));
    }

    #[async_std::test]
    async fn write_number() {
        assert_write_eq!(write_number, 0.0, Ok("0"));
        assert_write_eq!(write_number, 123.0, Ok("123"));
        assert_write_eq!(write_number, -234.0, Ok("-234"));
        assert_write_eq!(write_number, 123.456, Ok("123.456"));
        assert_write_eq!(write_number, -234.567, Ok("-234.567"));
    }

    #[async_std::test]
    async fn write_integer() {
        assert_write_eq!(write_integer, 0, Ok("0"));
        assert_write_eq!(write_integer, 123, Ok("123"));
        assert_write_eq!(write_integer, -234, Ok("-234"));
    }

    #[async_std::test]
    async fn write_boolean() {
        assert_write_eq!(write_boolean, false, Ok("false"));
        assert_write_eq!(write_boolean, true, Ok("true"));
    }

    #[async_std::test]
    async fn write_null() {
        assert_write_eq!(write_null, , Ok("null"));
    }
}
