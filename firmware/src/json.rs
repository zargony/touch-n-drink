use alloc::string::String;
use core::fmt;
use core::marker::PhantomData;
use core::ops::Range;
use embedded_io_async::Read;
use serde::de::DeserializeOwned;

/// Streaming JSON error
#[derive(Debug)]
pub enum Error<E> {
    /// Read error
    Read(E),
    /// EOF while parsing
    EofWhileParsing,
    /// The element to deserialize doesn't fit into the buffer
    BufferTooSmall,
    /// Expected this character to be a `'{'`
    ExpectedObjectBegin,
    /// Expected this character to be a `':'`
    ExpectedColon,
    /// Expected this character to be either a `','` or a `'}'`
    ExpectedObjectCommaOrEnd,
    /// JSON has non-whitespace trailing characters after the value
    TrailingCharacters,
    /// JSON parse error
    Json(serde_json::Error),
}

impl<E: fmt::Display> fmt::Display for Error<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(err) => write!(f, "Read error: {err}"),
            Self::BufferTooSmall => write!(f, "Buffer too small"),
            Self::EofWhileParsing => write!(f, "EOF while parsing"),
            Self::ExpectedObjectBegin => write!(f, "Expected `{{`"),
            Self::ExpectedColon => write!(f, "Expected `:`"),
            Self::ExpectedObjectCommaOrEnd => write!(f, "Expected `,` or `}}`"),
            Self::TrailingCharacters => write!(f, "Trailing characters"),
            Self::Json(err) => write!(f, "JSON error: {err}"),
        }
    }
}

/// Streaming JSON object reader.
/// Uses an IO reader to read and parse a JSON object element by element. Requires only one
/// element to fit into memory rather than the whole JSON data and object.
pub struct StreamingJsonObjectReader<R, T, const BUFSIZE: usize = 2048> {
    buffer: [u8; BUFSIZE],
    range: Range<usize>,
    reader: R,
    eof: bool,
    initialized: bool,
    done: bool,
    element_type: PhantomData<T>,
}

impl<R: Read, T: DeserializeOwned, const BUFSIZE: usize> StreamingJsonObjectReader<R, T, BUFSIZE> {
    /// Create streaming JSON object reader using the given reader
    pub fn new(reader: R) -> Self {
        Self {
            buffer: [0; BUFSIZE],
            range: 0..0,
            reader,
            eof: false,
            initialized: false,
            done: false,
            element_type: PhantomData,
        }
    }

    /// Return next key and element
    pub async fn next(&mut self) -> Result<Option<(String, T)>, Error<R::Error>> {
        self.fill_buf().await?;

        if self.done {
            match self.peek() {
                Ok(_) => return Err(Error::TrailingCharacters),
                Err(Error::EofWhileParsing) => return Ok(None),
                Err(err) => return Err(err),
            }
        }

        if !self.initialized {
            match self.peek()? {
                b'{' => self.consume(1),
                _ => return Err(Error::ExpectedObjectBegin),
            }
            self.initialized = true;
        }

        let key: String = self.deserialize()?;

        match self.peek()? {
            b':' => self.consume(1),
            _ => return Err(Error::ExpectedColon),
        }

        let value: T = self.deserialize()?;

        match self.peek()? {
            b',' => self.consume(1),
            b'}' => {
                self.consume(1);
                self.done = true;
            }
            _ => return Err(Error::ExpectedObjectCommaOrEnd),
        }

        Ok(Some((key, value)))
    }
}

impl<R: Read, T: DeserializeOwned, const BUFSIZE: usize> StreamingJsonObjectReader<R, T, BUFSIZE> {
    /// Move remaining buffer data to front and try to fill up the buffer by reading data
    async fn fill_buf(&mut self) -> Result<(), Error<R::Error>> {
        if self.range.start > 0 && self.range.end > 0 {
            self.buffer.copy_within(self.range.clone(), 0);
            self.range.end -= self.range.start;
            self.range.start = 0;
        }

        while !self.eof && self.range.end < BUFSIZE {
            let len = self
                .reader
                .read(&mut self.buffer[self.range.end..])
                .await
                .map_err(Error::Read)?;
            if len == 0 {
                self.eof = true;
            }
            self.range.end += len;
        }
        Ok(())
    }

    /// Consume data
    fn consume(&mut self, amt: usize) {
        self.range.start = (self.range.start + amt).min(self.range.end);
    }

    /// Currently buffered data to parse
    fn buffer(&self) -> &[u8] {
        &self.buffer[self.range.clone()]
    }

    /// Trim leading whitespace from buffer
    fn trim_whitespace(&mut self) {
        while self.range.start < self.range.end
            && self.buffer[self.range.start].is_ascii_whitespace()
        {
            self.range.start += 1;
        }
    }

    /// Skip leading whitespace and peek next byte in buffer
    fn peek(&mut self) -> Result<u8, Error<R::Error>> {
        self.trim_whitespace();
        match self.buffer().first() {
            Some(b) => Ok(*b),
            None if self.range.end == BUFSIZE => Err(Error::BufferTooSmall),
            None => Err(Error::EofWhileParsing),
        }
    }

    /// Deserialize given type
    fn deserialize<U: DeserializeOwned>(&mut self) -> Result<U, Error<R::Error>> {
        let mut de = serde_json::Deserializer::from_slice(self.buffer()).into_iter();
        let value = match de.next() {
            Some(Ok(value)) => value,
            Some(Err(err)) => return Err(Error::Json(err)),
            None => return Err(Error::EofWhileParsing),
        };
        let len = de.byte_offset();
        self.consume(len);
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[async_std::test]
    async fn fill_and_consume() {
        let json = b"0123456789abcdef";
        let mut reader: StreamingJsonObjectReader<_, (), 10> =
            StreamingJsonObjectReader::new(&json[..]);
        reader.fill_buf().await.unwrap();
        assert_eq!(reader.buffer(), b"0123456789");
        reader.fill_buf().await.unwrap();
        assert_eq!(reader.buffer(), b"0123456789");
        reader.consume(8);
        assert_eq!(reader.buffer(), b"89");
        reader.fill_buf().await.unwrap();
        assert_eq!(reader.buffer(), b"89abcdef");
        reader.fill_buf().await.unwrap();
        assert_eq!(reader.buffer(), b"89abcdef");
        reader.consume(8);
        assert_eq!(reader.buffer(), b"");
        reader.fill_buf().await.unwrap();
        assert_eq!(reader.buffer(), b"");
    }

    #[derive(Debug, PartialEq, Deserialize)]
    struct Person {
        name: String,
        age: u8,
    }

    #[async_std::test]
    async fn smoke() {
        let json = br#"{"foo": {"name": "Alice", "age": 42}, "bar": {"name": "Bob", "age": 23}}"#;
        let mut reader: StreamingJsonObjectReader<_, Person> =
            StreamingJsonObjectReader::new(&json[..]);
        let (key, person) = reader.next().await.unwrap().unwrap();
        assert_eq!(key, "foo");
        assert_eq!(person.name, "Alice");
        assert_eq!(person.age, 42);
        let (key, person) = reader.next().await.unwrap().unwrap();
        assert_eq!(key, "bar");
        assert_eq!(person.name, "Bob");
        assert_eq!(person.age, 23);
        assert_eq!(reader.next().await.unwrap(), None);
    }
}
