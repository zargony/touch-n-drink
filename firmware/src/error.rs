use crate::{display, nfc, vereinsflieger};
use core::fmt;

/// Main error type
#[derive(Debug)]
#[allow(clippy::enum_variant_names)]
pub enum Error {
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

impl From<display::Error> for Error {
    fn from(err: display::Error) -> Self {
        Self::DisplayError(err)
    }
}

impl From<nfc::Error> for Error {
    fn from(err: nfc::Error) -> Self {
        Self::NFCError(err)
    }
}

impl From<vereinsflieger::Error> for Error {
    fn from(err: vereinsflieger::Error) -> Self {
        Self::VereinsfliegerError(err)
    }
}

impl fmt::Display for Error {
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
