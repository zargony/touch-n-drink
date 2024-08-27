use crate::screen::{self, Screen};
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
pub type Error = screen::Error<display_interface::DisplayError>;

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
