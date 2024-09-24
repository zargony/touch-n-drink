use crate::screen::{self, Screen};
use core::fmt;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_hal_async::i2c::I2c;
use log::{debug, info};
use ssd1306::mode::{BufferedGraphicsModeAsync, DisplayConfigAsync};
use ssd1306::prelude::I2CInterface;
use ssd1306::rotation::DisplayRotation;
use ssd1306::size::DisplaySize128x64;
use ssd1306::Ssd1306Async;

/// Display error
#[derive(Debug)]
pub enum Error {
    /// Display interface error
    InterfaceError(display_interface::DisplayError),
    /// Font render error
    FontRenderError(screen::Error<()>),
}

impl From<display_interface::DisplayError> for Error {
    fn from(err: display_interface::DisplayError) -> Self {
        Self::InterfaceError(err)
    }
}

impl From<screen::Error<display_interface::DisplayError>> for Error {
    fn from(err: screen::Error<display_interface::DisplayError>) -> Self {
        use screen::Error;

        match err {
            Error::BackgroundColorNotSupported => {
                Self::FontRenderError(Error::BackgroundColorNotSupported)
            }
            Error::GlyphNotFound(ch) => Self::FontRenderError(Error::GlyphNotFound(ch)),
            Error::DisplayError(err) => Self::InterfaceError(err),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use display_interface::DisplayError;

        match self {
            Self::InterfaceError(err) => match err {
                DisplayError::InvalidFormatError => write!(f, "Invalid format"),
                DisplayError::BusWriteError => write!(f, "Bus write error"),
                DisplayError::DCError => write!(f, "DC error"),
                DisplayError::CSError => write!(f, "CS error"),
                DisplayError::DataFormatNotImplemented => write!(f, "Format not implemented"),
                DisplayError::RSError => write!(f, "Reset error"),
                DisplayError::OutOfBoundsError => write!(f, "Out of bounds"),
                _ => write!(f, "Interface error"),
            },
            Self::FontRenderError(_err) => write!(f, "Font render error"),
        }
    }
}

/// Convenient hardware-agnostic display driver
pub struct Display<I2C> {
    driver: Ssd1306Async<
        I2CInterface<I2C>,
        DisplaySize128x64,
        BufferedGraphicsModeAsync<DisplaySize128x64>,
    >,
}

impl<I2C: I2c> Display<I2C> {
    /// Create display driver and initialize display hardware
    pub async fn new(i2c: I2C) -> Result<Self, Error> {
        debug!("Display: Initializing SSD1306...");

        // Build SSD1306 driver and switch to buffered graphics mode
        let mut driver = Ssd1306Async::new(
            I2CInterface::new(i2c, 0x3c, 0x40),
            DisplaySize128x64,
            DisplayRotation::Rotate0,
        )
        .into_buffered_graphics_mode();

        // Initialize and clear display
        driver.init().await?;
        driver.clear(BinaryColor::Off)?;
        driver.flush().await?;

        info!("Display: SSD1306 initialized");
        Ok(Self { driver })
    }

    /// Turn display off
    pub async fn turn_off(&mut self) -> Result<(), Error> {
        debug!("Display: Power off");
        self.driver.set_display_on(false).await?;
        Ok(())
    }

    /// Clear display
    #[allow(dead_code)]
    pub async fn clear(&mut self) -> Result<(), Error> {
        self.driver.clear(BinaryColor::Off)?;
        self.driver.flush().await?;
        self.driver.set_display_on(true).await?;
        Ok(())
    }

    /// Show screen
    pub async fn screen<S: Screen>(&mut self, screen: &S) -> Result<(), Error> {
        self.driver.clear(BinaryColor::Off)?;
        screen.draw(&mut self.driver)?;
        self.driver.flush().await?;
        self.driver.set_display_on(true).await?;
        Ok(())
    }
}
