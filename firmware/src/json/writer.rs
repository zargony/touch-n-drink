use super::error::Error;
use super::value::Value;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use embedded_io_async::Write;

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
            Value::Object(object) => Box::pin(self.write(object)).await,
            Value::Array(array) => Box::pin(self.write(array)).await,
            Value::String(string) => self.write(string).await,
            Value::Decimal(number) => self.write(*number).await,
            Value::Integer(number) => self.write(*number).await,
            Value::Boolean(boolean) => self.write(*boolean).await,
            Value::Null => self.write(()).await,
        }
    }

    /// Write JSON object
    pub async fn write_object(&mut self) -> Result<ObjectWriter<W>, Error<W::Error>> {
        ObjectWriter::new(self).await
    }

    /// Write JSON array
    pub async fn write_array<'a, T, I>(&mut self, iter: I) -> Result<(), Error<W::Error>>
    where
        T: ToJson + 'a,
        I: IntoIterator<Item = T>,
    {
        self.write_raw(b"[").await?;
        for (i, elem) in iter.into_iter().enumerate() {
            if i > 0 {
                self.write_raw(b", ").await?;
            }
            self.write(elem).await?;
        }
        self.write_raw(b"]").await?;
        Ok(())
    }

    /// Write JSON string
    pub async fn write_string(&mut self, value: &str) -> Result<(), Error<W::Error>> {
        self.write_raw(b"\"").await?;
        // OPTIMIZE: Writing each char separately to a writer is quite inefficient
        for ch in value.escape_default() {
            self.write_raw(&[ch as u8]).await?;
        }
        self.write_raw(b"\"").await?;
        Ok(())
    }

    /// Write JSON number (decimal)
    pub async fn write_decimal(&mut self, value: f64) -> Result<(), Error<W::Error>> {
        let buf = value.to_string();
        self.write_raw(buf.as_bytes()).await?;
        Ok(())
    }

    /// Write JSON number (integer)
    pub async fn write_integer(&mut self, value: i64) -> Result<(), Error<W::Error>> {
        let buf = value.to_string();
        self.write_raw(buf.as_bytes()).await?;
        Ok(())
    }

    /// Write JSON boolean
    pub async fn write_boolean(&mut self, value: bool) -> Result<(), Error<W::Error>> {
        self.write_raw(if value { b"true" } else { b"false" })
            .await?;
        Ok(())
    }

    /// Write JSON null
    pub async fn write_null(&mut self) -> Result<(), Error<W::Error>> {
        self.write_raw(b"null").await?;
        Ok(())
    }
}

impl<W: Write> Writer<W> {
    /// Write given buffer to JSON
    async fn write_raw(&mut self, bytes: &[u8]) -> Result<(), Error<W::Error>> {
        Ok(self.writer.write_all(bytes).await?)
    }
}

/// JSON object writer
#[allow(clippy::module_name_repetitions)]
pub struct ObjectWriter<'w, W: Write> {
    json: &'w mut Writer<W>,
    has_fields: bool,
}

impl<'w, W: Write> ObjectWriter<'w, W> {
    /// Start object
    pub async fn new(json: &'w mut Writer<W>) -> Result<Self, Error<W::Error>> {
        json.write_raw(b"{").await?;
        Ok(Self {
            json,
            has_fields: false,
        })
    }

    /// Write object field
    pub async fn field<T: ToJson>(
        &mut self,
        key: &str,
        value: T,
    ) -> Result<&mut Self, Error<W::Error>> {
        if self.has_fields {
            self.json.write_raw(b", ").await?;
        }
        self.json.write_string(key).await?;
        self.json.write_raw(b": ").await?;
        self.json.write(value).await?;
        self.has_fields = true;
        Ok(self)
    }

    /// Write object fields from iterable collections
    pub async fn fields_from<'a, K, V, I>(&mut self, iter: I) -> Result<&mut Self, Error<W::Error>>
    where
        K: AsRef<str> + ?Sized + 'a,
        V: ToJson + 'a,
        I: IntoIterator<Item = (&'a K, &'a V)>,
    {
        for (key, value) in iter {
            self.field(key.as_ref(), value).await?;
        }
        Ok(self)
    }

    /// Finish object
    pub async fn finish(&mut self) -> Result<(), Error<W::Error>> {
        self.json.write_raw(b"}").await?;
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

impl ToJson for usize {
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

impl ToJson for isize {
    async fn to_json<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), Error<W::Error>> {
        writer
            .write_integer(i64::try_from(*self).map_err(|_e| Error::NumberTooLarge)?)
            .await
    }
}

impl ToJson for f32 {
    async fn to_json<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), Error<W::Error>> {
        writer.write_decimal(f64::from(*self)).await
    }
}

impl ToJson for f64 {
    async fn to_json<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), Error<W::Error>> {
        writer.write_decimal(*self).await
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

impl<T: ToJson> ToJson for Vec<T> {
    async fn to_json<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), Error<W::Error>> {
        writer.write_array(self).await
    }
}

impl<K: AsRef<str>, V: ToJson> ToJson for [(K, V)] {
    async fn to_json<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), Error<W::Error>> {
        writer
            .write_object()
            .await?
            .fields_from(self.iter().map(|(k, v)| (k.as_ref(), v)))
            .await?
            .finish()
            .await
    }
}

impl<K: AsRef<str>, V: ToJson> ToJson for BTreeMap<K, V> {
    async fn to_json<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), Error<W::Error>> {
        writer
            .write_object()
            .await?
            .fields_from(self)
            .await?
            .finish()
            .await
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

    fn writer() -> Writer<Vec<u8>> {
        Writer::new(Vec::new())
    }

    macro_rules! assert_write_eq {
        ($method:ident, $($value:expr)?, $json:expr) => {{
            let mut writer = writer();
            let res = writer.$method($($value)?).await;
            let json = String::from_utf8(writer.into_inner()).unwrap();
            assert_eq!(res.map(|()| &*json), $json)
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
                    .write_object()
                    .await?
                    .field("foo", &self.foo)
                    .await?
                    .field("bar", self.bar)
                    .await?
                    .field("baz", self.baz)
                    .await?
                    .finish()
                    .await
            }
        }

        assert_write_eq!(
            write,
            Test {
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
        assert_write_eq!(write_any, &Value::Integer(123), Ok("123"));
        assert_write_eq!(write_any, &Value::Decimal(123.456), Ok("123.456"));
        assert_write_eq!(write_any, &Value::String("hello".into()), Ok("\"hello\""));
        assert_write_eq!(
            write_any,
            &Value::Array(vec![
                Value::Integer(1),
                Value::Integer(2),
                Value::Integer(3),
                Value::Integer(4)
            ]),
            Ok("[1, 2, 3, 4]")
        );
        assert_write_eq!(
            write_any,
            &Value::Object(BTreeMap::from([
                ("foo".into(), Value::String("hi".into())),
                ("bar".into(), Value::Integer(42)),
                ("baz".into(), Value::Boolean(true)),
            ])),
            // Value's inner BTreeMap reorders fields
            Ok(r#"{"bar": 42, "baz": true, "foo": "hi"}"#)
        );
    }

    #[async_std::test]
    async fn write_object() {
        let mut writer = writer();
        let res = writer
            .write_object()
            .await
            .unwrap()
            .field("foo", "hi")
            .await
            .unwrap()
            .field("bar", 42)
            .await
            .unwrap()
            .field("baz", true)
            .await
            .unwrap()
            .finish()
            .await;
        let json = String::from_utf8(writer.into_inner()).unwrap();
        assert_eq!(
            res.map(|()| &*json),
            Ok(r#"{"foo": "hi", "bar": 42, "baz": true}"#)
        );
    }

    #[async_std::test]
    async fn write_array() {
        assert_write_eq!(write_array, Vec::<u32>::new(), Ok("[]"));
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
    async fn write_decimal() {
        assert_write_eq!(write_decimal, 0.0, Ok("0"));
        assert_write_eq!(write_decimal, 123.0, Ok("123"));
        assert_write_eq!(write_decimal, -234.0, Ok("-234"));
        assert_write_eq!(write_decimal, 123.456, Ok("123.456"));
        assert_write_eq!(write_decimal, -234.567, Ok("-234.567"));
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
