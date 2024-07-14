use embedded_graphics::mono_font::ascii::FONT_6X13;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::text::Text;
use embedded_hal::i2c::I2c;
use ssd1306::mode::{BufferedGraphicsMode, DisplayConfig};
use ssd1306::prelude::I2CInterface;
use ssd1306::rotation::DisplayRotation;
use ssd1306::size::DisplaySize128x64;
use ssd1306::Ssd1306;

// The `ssd1306` crate unfortunately doesn't support async yet (though `display-interface`,
// `display-interface-i2c` and `embedded-hal-bus` do), so we can't use async here yet.
// See also https://github.com/rust-embedded-community/ssd1306/pull/189

/// Display error
pub use display_interface::DisplayError;

/// Convenient hardware-agnostic display driver
pub struct Display<I2C: I2c> {
    driver: Ssd1306<I2CInterface<I2C>, DisplaySize128x64, BufferedGraphicsMode<DisplaySize128x64>>,
}

impl<I2C: I2c> Display<I2C> {
    /// Create display driver and initialize display hardware
    pub fn new(i2c: I2C, addr: u8) -> Result<Self, DisplayError> {
        // Build SSD1306 driver and switch to buffered graphics mode
        let mut driver = Ssd1306::new(
            I2CInterface::new(i2c, addr, 0x40),
            DisplaySize128x64,
            DisplayRotation::Rotate0,
        )
        .into_buffered_graphics_mode();

        // Initialize and clear display
        driver.init()?;
        driver.clear(BinaryColor::Off)?;
        driver.flush()?;

        Ok(Display { driver })
    }

    /// Clear display
    pub fn clear(&mut self) -> Result<(), DisplayError> {
        self.driver.clear(BinaryColor::Off)?;
        self.driver.flush()?;
        Ok(())
    }

    /// Display hello screen
    pub fn hello(&mut self) -> Result<(), DisplayError> {
        let style = MonoTextStyle::new(&FONT_6X13, BinaryColor::On);
        Text::new("Hello, world!", Point::new(0, 20), style).draw(&mut self.driver)?;
        self.driver.flush()?;
        Ok(())
    }
}
