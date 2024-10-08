use crate::{display, nfc};
use core::fmt;

/// Main error type
#[derive(Debug)]
#[allow(clippy::enum_variant_names)]
pub enum Error {
    /// Display output error
    DisplayError(display::Error),
    /// NFC reader error
    NFCError(nfc::Error),
    /// User cancel request
    Cancel,
    /// User interaction timeout
    UserTimeout,
    /// No network connection
    NoNetwork,
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

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DisplayError(err) => write!(f, "Display: {err}"),
            Self::NFCError(err) => write!(f, "NFC: {err}"),
            Self::Cancel => write!(f, "User cancelled"),
            Self::UserTimeout => write!(f, "Timeout waiting for input"),
            Self::NoNetwork => write!(f, "No network connection"),
        }
    }
}
