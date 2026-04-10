use alloc::string::String;
use core::fmt;
use core::ops::Deref;
use embassy_time::Duration;
use embedded_graphics::geometry::AnchorY;
use embedded_graphics::primitives::Rectangle;
use serde::Deserialize;

/// String with sensitive content (debug and display output redacted)
#[derive(Default, Deserialize)]
#[serde(transparent)]
#[must_use]
pub struct SensitiveString(pub String);

impl fmt::Debug for SensitiveString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            self.0.fmt(f)
        } else {
            "<redacted>".fmt(f)
        }
    }
}

impl fmt::Display for SensitiveString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            self.0.fmt(f)
        } else {
            "<redacted>".fmt(f)
        }
    }
}

impl Deref for SensitiveString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Option display helper
#[must_use]
pub struct DisplayOption<T: fmt::Display>(pub Option<T>);

impl<T: fmt::Display> fmt::Display for DisplayOption<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            None => write!(f, "-"),
            Some(value) => value.fmt(f),
        }
    }
}

/// List slice helper
#[must_use]
pub struct DisplaySlice<'a, T: fmt::Display>(pub &'a [T]);

impl<T: fmt::Display> fmt::Display for DisplaySlice<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            write!(f, "-")?;
        } else {
            for (i, elem) in self.0.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                elem.fmt(f)?;
            }
        }
        Ok(())
    }
}

/// Duration display helper
#[must_use]
pub struct DisplayDuration(pub Duration);

impl fmt::Display for DisplayDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hours = self.0.as_secs() / 3600;
        let min = self.0.as_secs() % 3600 / 60;
        let secs = self.0.as_secs() % 60;
        write!(f, "{hours}h{min}m{secs}s")
    }
}

/// Rectangle helper methods
pub trait RectangleExt {
    /// Split rectangle into two: a top part of given height (header) and bottom part
    fn header(&self, height: u32) -> (Self, Self)
    where
        Self: Sized;

    /// Split rectangle into two: a top part and bottom part of given height (footer)
    fn footer(&self, height: u32) -> (Self, Self)
    where
        Self: Sized;
}

impl RectangleExt for Rectangle {
    fn header(&self, height: u32) -> (Self, Self) {
        (
            self.resized_height(height, AnchorY::Top),
            self.resized_height(self.size.height - height, AnchorY::Bottom),
        )
    }

    fn footer(&self, height: u32) -> (Self, Self) {
        (
            self.resized_height(self.size.height - height, AnchorY::Top),
            self.resized_height(height, AnchorY::Bottom),
        )
    }
}
