use alloc::string::String;
use core::marker::PhantomData;
use core::ops::Range;
use derive_more::Display;
use embedded_io_async::Read;
use serde::de::DeserializeOwned;

/// Reader error
#[derive(Debug, Display)]
pub enum Error<E> {
    /// Read error
    #[display("Read: {_0}")]
    Read(E),
    /// EOF while parsing
    #[display("EOF while parsing")]
    EofWhileParsing,
    /// The element to deserialize doesn't fit into the buffer
    #[display("Buffer too small")]
    BufferTooSmall,
    /// Expected this character to be a `'{'`
    #[display("Expected `{{`")]
    ExpectedObjectBegin,
    /// Expected this character to be a `':'`
    #[display("Expected `:`")]
    ExpectedColon,
    /// Expected this character to be either a `','` or a `'}'`
    #[display("Expected `,` or `}}`")]
    ExpectedObjectCommaOrEnd,
    /// Input has non-whitespace trailing characters
    #[display("Trailing characters")]
    TrailingCharacters,
    /// JSON parse error
    #[display("JSON: {_0}")]
    Json(serde_json::Error),
}

/// Buffered reader
/// Uses an async IO reader to fill a buffer and provide its content.
pub struct BufferedReader<R, const BUFSIZE: usize = 1024> {
    buffer: [u8; BUFSIZE],
    range: Range<usize>,
    reader: R,
    eof: bool,
}

impl<R: Read, const BUFSIZE: usize> BufferedReader<R, BUFSIZE> {
    /// Create buffered reader using the given reader
    pub fn new(reader: R) -> Self {
        Self {
            buffer: [0; BUFSIZE],
            range: 0..0,
            reader,
            eof: false,
        }
    }

    /// Move remaining buffer data to front and try to fill up the buffer by reading data
    pub async fn read(&mut self) -> Result<(), Error<R::Error>> {
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
    pub fn consume(&mut self, amt: usize) {
        self.range.start = (self.range.start + amt).min(self.range.end);
    }

    /// Currently buffered data to parse
    pub fn buffer(&self) -> &[u8] {
        &self.buffer[self.range.clone()]
    }

    /// Trim leading whitespace from buffer
    pub fn trim_whitespace(&mut self) {
        while self.range.start < self.range.end
            && self.buffer[self.range.start].is_ascii_whitespace()
        {
            self.range.start += 1;
        }
    }

    /// Peek next byte in buffer
    pub fn peek(&mut self) -> Result<u8, Error<R::Error>> {
        match self.buffer().first() {
            Some(b) => Ok(*b),
            None if self.range.end == BUFSIZE => Err(Error::BufferTooSmall),
            None => Err(Error::EofWhileParsing),
        }
    }

    /// Peek next line in buffer
    pub fn peek_line(&mut self) -> Result<&[u8], Error<R::Error>> {
        match self.buffer().iter().position(|b| b"\r\n".contains(b)) {
            Some(pos) => Ok(&self.buffer()[..pos]),
            None if self.range.end == BUFSIZE => Err(Error::BufferTooSmall),
            None => Ok(self.buffer()),
        }
    }
}

/// Line reader
/// Uses an async IO reader to read the input stream line by line.
pub struct LineReader<R, const BUFSIZE: usize = 256> {
    buffer: BufferedReader<R, BUFSIZE>,
    last_line_len: usize,
}

impl<R: Read, const BUFSIZE: usize> LineReader<R, BUFSIZE> {
    /// Create line reader object using the given reader
    pub fn new(reader: R) -> Self {
        Self {
            buffer: BufferedReader::new(reader),
            last_line_len: 0,
        }
    }

    /// Return next line
    pub async fn next(&mut self) -> Result<Option<&[u8]>, Error<R::Error>> {
        self.buffer.consume(self.last_line_len);
        self.buffer.read().await?;
        if self.buffer.buffer().is_empty() {
            return Ok(None);
        }
        let line = self.buffer.peek_line()?;
        self.last_line_len = line.len() + 1;
        Ok(Some(line))
    }
}

/// Streaming JSON object reader.
/// Uses an async IO reader to read and parse a JSON object element by element. Requires only one
/// element to fit into memory rather than the whole JSON data and object.
pub struct StreamingJsonObjectReader<R, T, const BUFSIZE: usize = 2048> {
    buffer: BufferedReader<R, BUFSIZE>,
    initialized: bool,
    done: bool,
    element_type: PhantomData<T>,
}

impl<R: Read, T: DeserializeOwned, const BUFSIZE: usize> StreamingJsonObjectReader<R, T, BUFSIZE> {
    /// Create streaming JSON object reader using the given reader
    pub fn new(reader: R) -> Self {
        Self {
            buffer: BufferedReader::new(reader),
            initialized: false,
            done: false,
            element_type: PhantomData,
        }
    }

    /// Return next key and element
    pub async fn next(&mut self) -> Result<Option<(String, T)>, Error<R::Error>> {
        self.buffer.read().await?;

        if self.done {
            self.buffer.trim_whitespace();
            match self.buffer.peek() {
                Ok(_) => return Err(Error::TrailingCharacters),
                Err(Error::EofWhileParsing) => return Ok(None),
                Err(err) => return Err(err),
            }
        }

        if !self.initialized {
            self.buffer.trim_whitespace();
            match self.buffer.peek()? {
                b'{' => self.buffer.consume(1),
                _ => return Err(Error::ExpectedObjectBegin),
            }
            self.initialized = true;
        }

        let key: String = self.deserialize()?;

        self.buffer.trim_whitespace();
        match self.buffer.peek()? {
            b':' => self.buffer.consume(1),
            _ => return Err(Error::ExpectedColon),
        }

        let value: T = self.deserialize()?;

        self.buffer.trim_whitespace();
        match self.buffer.peek()? {
            b',' => self.buffer.consume(1),
            b'}' => {
                self.buffer.consume(1);
                self.done = true;
            }
            _ => return Err(Error::ExpectedObjectCommaOrEnd),
        }

        Ok(Some((key, value)))
    }
}

impl<R: Read, T: DeserializeOwned, const BUFSIZE: usize> StreamingJsonObjectReader<R, T, BUFSIZE> {
    /// Deserialize given type
    fn deserialize<U: DeserializeOwned>(&mut self) -> Result<U, Error<R::Error>> {
        let mut de = serde_json::Deserializer::from_slice(self.buffer.buffer()).into_iter();
        let value = match de.next() {
            Some(Ok(value)) => value,
            Some(Err(err)) => return Err(Error::Json(err)),
            None => return Err(Error::EofWhileParsing),
        };
        let len = de.byte_offset();
        self.buffer.consume(len);
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[async_std::test]
    async fn buffered_reader() {
        let input = b"0123456789abcdef";
        let mut reader: BufferedReader<_, 10> = BufferedReader::new(&input[..]);
        reader.read().await.unwrap();
        assert_eq!(reader.buffer(), b"0123456789");
        reader.read().await.unwrap();
        assert_eq!(reader.buffer(), b"0123456789");
        reader.consume(8);
        assert_eq!(reader.buffer(), b"89");
        reader.read().await.unwrap();
        assert_eq!(reader.buffer(), b"89abcdef");
        reader.read().await.unwrap();
        assert_eq!(reader.buffer(), b"89abcdef");
        reader.consume(8);
        assert_eq!(reader.buffer(), b"");
        reader.read().await.unwrap();
        assert_eq!(reader.buffer(), b"");
    }

    #[derive(Debug, PartialEq, Deserialize)]
    struct Person {
        name: String,
        age: u8,
    }

    #[async_std::test]
    async fn streaming_json_object_reader() {
        let input = br#"{"foo": {"name": "Alice", "age": 42}, "bar": {"name": "Bob", "age": 23}}"#;
        let mut reader: StreamingJsonObjectReader<_, Person> =
            StreamingJsonObjectReader::new(&input[..]);
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

    #[async_std::test]
    async fn line_reader() {
        let input = b"one\ntwo\nthree\n";
        let mut reader: LineReader<_> = LineReader::new(&input[..]);
        let line = reader.next().await.unwrap().unwrap();
        assert_eq!(line, b"one");
        let line = reader.next().await.unwrap().unwrap();
        assert_eq!(line, b"two");
        let line = reader.next().await.unwrap().unwrap();
        assert_eq!(line, b"three");
        assert_eq!(reader.next().await.unwrap(), None);
    }

    #[async_std::test]
    async fn line_reader_multi_newlines() {
        let input = b"one\n\ntwo\nthree";
        let mut reader: LineReader<_> = LineReader::new(&input[..]);
        let line = reader.next().await.unwrap().unwrap();
        assert_eq!(line, b"one");
        let line = reader.next().await.unwrap().unwrap();
        assert_eq!(line, b"");
        let line = reader.next().await.unwrap().unwrap();
        assert_eq!(line, b"two");
        let line = reader.next().await.unwrap().unwrap();
        assert_eq!(line, b"three");
        assert_eq!(reader.next().await.unwrap(), None);
    }

    #[async_std::test]
    async fn line_reader_no_trailing_newline() {
        let input = b"one\ntwo\nthree";
        let mut reader: LineReader<_> = LineReader::new(&input[..]);
        let line = reader.next().await.unwrap().unwrap();
        assert_eq!(line, b"one");
        let line = reader.next().await.unwrap().unwrap();
        assert_eq!(line, b"two");
        let line = reader.next().await.unwrap().unwrap();
        assert_eq!(line, b"three");
        assert_eq!(reader.next().await.unwrap(), None);
    }
}
