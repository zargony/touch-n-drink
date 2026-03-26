use crate::user::UserId;
use crate::{display, nfc, ota, vereinsflieger};
use core::future::Future;
use derive_more::{Display, From};

/// Main error type
#[derive(Debug, Display)]
#[display("{kind}")]
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

impl Error {
    /// Error kind
    #[expect(dead_code)]
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
    #[expect(dead_code)]
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
#[derive(Debug, Display, From)]
pub enum ErrorKind {
    /// Display output error
    #[from]
    #[display("Display: {_0}")]
    DisplayError(display::Error),
    /// NFC reader error
    #[from]
    #[display("NFC: {_0}")]
    NFCError(nfc::Error),
    /// Vereinsflieger API error
    #[from]
    #[display("Vereinsflieger: {_0}")]
    VereinsfliegerError(vereinsflieger::Error),
    /// OTA error
    #[from]
    #[display("OTA: {_0}")]
    Ota(ota::Error),
    /// User cancel request
    #[display("User cancelled")]
    Cancel,
    /// User interaction timeout
    #[display("Timeout waiting for input")]
    UserTimeout,
    /// No network connection
    #[display("No network connection")]
    NoNetwork,
    /// Current time is required but not set
    #[display("Unknown current time")]
    CurrentTimeNotSet,
    /// The specified article was not found
    #[display("Article not found")]
    ArticleNotFound,
}
