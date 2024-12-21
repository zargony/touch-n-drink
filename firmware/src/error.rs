use crate::json::{self, ToJson};
use crate::user::UserId;
use crate::{display, nfc, vereinsflieger};
use alloc::string::ToString;
use core::fmt;
use core::future::Future;
use embedded_io_async::Write;

/// Main error type
#[derive(Debug)]
pub struct Error {
    /// Error kind with optional embedded causing error type
    kind: ErrorKind,
    /// Optional user whose action caused the error
    user_id: Option<UserId>,
}

impl<T: Into<ErrorKind>> From<T> for Error {
    fn from(err: T) -> Self {
        Self {
            kind: err.into(),
            user_id: None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.kind)
    }
}

impl Error {
    /// Error kind
    #[allow(dead_code)]
    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }

    /// True if user cancelled an action
    pub fn is_cancel(&self) -> bool {
        matches!(self.kind, ErrorKind::Cancel)
    }

    /// True if user interaction timed out
    pub fn is_user_timeout(&self) -> bool {
        matches!(self.kind, ErrorKind::UserTimeout)
    }

    /// User whose action caused the error, if any
    pub fn user_id(&self) -> Option<UserId> {
        self.user_id
    }

    /// Try running the provided closure and associate the given user id with any error that might
    /// be returned by it
    #[allow(dead_code)]
    pub fn try_with<T, F>(user_id: UserId, f: F) -> Result<T, Self>
    where
        F: FnOnce() -> Result<T, Self>,
    {
        f().map_err(|err| Self {
            kind: err.kind,
            user_id: Some(user_id),
        })
    }

    /// Try running the provided future and associate the given user id with any error that might
    /// be returned by it
    pub async fn try_with_async<T, F>(user_id: UserId, fut: F) -> Result<T, Self>
    where
        F: Future<Output = Result<T, Self>>,
    {
        fut.await.map_err(|err| Self {
            kind: err.kind,
            user_id: Some(user_id),
        })
    }
}

/// Error kind with optional embedded causing error type
#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub enum ErrorKind {
    /// Display output error
    DisplayError(display::Error),
    /// NFC reader error
    NFCError(nfc::Error),
    /// Vereinsflieger API error
    VereinsfliegerError(vereinsflieger::Error),
    /// User cancel request
    Cancel,
    /// User interaction timeout
    UserTimeout,
    /// No network connection
    NoNetwork,
    /// The specified article was not found
    ArticleNotFound,
}

impl From<display::Error> for ErrorKind {
    fn from(err: display::Error) -> Self {
        Self::DisplayError(err)
    }
}

impl From<nfc::Error> for ErrorKind {
    fn from(err: nfc::Error) -> Self {
        Self::NFCError(err)
    }
}

impl From<vereinsflieger::Error> for ErrorKind {
    fn from(err: vereinsflieger::Error) -> Self {
        Self::VereinsfliegerError(err)
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DisplayError(err) => write!(f, "Display: {err}"),
            Self::NFCError(err) => write!(f, "NFC: {err}"),
            Self::VereinsfliegerError(err) => write!(f, "Vereinsflieger: {err}"),
            Self::Cancel => write!(f, "User cancelled"),
            Self::UserTimeout => write!(f, "Timeout waiting for input"),
            Self::NoNetwork => write!(f, "No network connection"),
            Self::ArticleNotFound => write!(f, "Article not found"),
        }
    }
}

impl ToJson for ErrorKind {
    async fn to_json<W: Write>(
        &self,
        json: &mut json::Writer<W>,
    ) -> Result<(), json::Error<W::Error>> {
        json.write(self.to_string()).await
    }
}
