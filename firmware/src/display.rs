use core::fmt;
use derive_more::From;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;
use embedded_hal_async::i2c::I2c;
use log::{debug, info};
use ssd1306::Ssd1306Async;
use ssd1306::mode::{BufferedGraphicsModeAsync, DisplayConfigAsync};
use ssd1306::prelude::I2CInterface;
use ssd1306::rotation::DisplayRotation;
use ssd1306::size::DisplaySize128x64;

/// Display error
#[derive(Debug, From)]
pub struct Error(display_interface::DisplayError);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use display_interface::DisplayError;

        match self.0 {
            DisplayError::InvalidFormatError => write!(f, "Invalid format"),
            DisplayError::BusWriteError => write!(f, "Bus write error"),
            DisplayError::DCError => write!(f, "DC error"),
            DisplayError::CSError => write!(f, "CS error"),
            DisplayError::DataFormatNotImplemented => write!(f, "Format not implemented"),
            DisplayError::RSError => write!(f, "Reset error"),
            DisplayError::OutOfBoundsError => write!(f, "Out of bounds"),
            _ => write!(f, "Interface error"),
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

impl<I2C: I2c> Dimensions for Display<I2C> {
    fn bounding_box(&self) -> Rectangle {
        self.driver.bounding_box()
    }
}

impl<I2C: I2c> DrawTarget for Display<I2C> {
    type Color = BinaryColor;
    type Error = Error;

    fn draw_iter<I: IntoIterator<Item = Pixel<Self::Color>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Self::Error> {
        Ok(self.driver.draw_iter(pixels)?)
    }

    fn fill_contiguous<I: IntoIterator<Item = Self::Color>>(
        &mut self,
        area: &Rectangle,
        colors: I,
    ) -> Result<(), Self::Error> {
        Ok(self.driver.fill_contiguous(area, colors)?)
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        Ok(self.driver.fill_solid(area, color)?)
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        Ok(self.driver.clear(color)?)
    }
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

    /// Flush the graphics buffer, making drawn graphics visible on the physical display
    pub async fn flush(&mut self) -> Result<(), Error> {
        self.driver.flush().await?;
        self.driver.set_display_on(true).await?;
        Ok(())
    }
}
