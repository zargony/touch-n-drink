use super::value::TryFromValueError;
use core::fmt;

/// JSON reader/writer error
#[derive(Debug, Clone, PartialEq, Eq)]
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

impl<E> From<TryFromValueError> for Error<E> {
    fn from(_err: TryFromValueError) -> Self {
        Self::InvalidType
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
    pub fn unexpected(ch: u8) -> Self {
        Self::Unexpected(char::from(ch))
    }
}
