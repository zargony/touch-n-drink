use embedded_graphics::mono_font::ascii::{FONT_10X20, FONT_6X10, FONT_9X18_BOLD};
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::text::{Alignment, Baseline, Text, TextStyleBuilder};
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
pub use display_interface::DisplayError as Error;

/// Convenient hardware-agnostic display driver
pub struct Display<I2C> {
    driver: Ssd1306<I2CInterface<I2C>, DisplaySize128x64, BufferedGraphicsMode<DisplaySize128x64>>,
}

impl<I2C: I2c> Display<I2C> {
    /// Create display driver and initialize display hardware
    pub fn new(i2c: I2C, addr: u8) -> Result<Self, Error> {
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
    pub fn clear(&mut self) -> Result<(), Error> {
        self.driver.clear(BinaryColor::Off)?;
        self.driver.flush()?;
        Ok(())
    }

    /// Display splash screen
    pub fn splash(&mut self) -> Result<(), Error> {
        self.driver.clear(BinaryColor::Off)?;
        Text::with_alignment(
            "Touch 'n Drink",
            Point::new(64, 28),
            MonoTextStyle::new(&FONT_9X18_BOLD, BinaryColor::On),
            Alignment::Center,
        )
        .draw(&mut self.driver)?;
        Text::with_alignment(
            concat!("v", env!("CARGO_PKG_VERSION")),
            Point::new(64, 28 + 12),
            MonoTextStyle::new(&FONT_6X10, BinaryColor::On),
            Alignment::Center,
        )
        .draw(&mut self.driver)?;
        self.driver.flush()?;
        Ok(())
    }

    /// Display big centered text
    pub fn big_centered_char(&mut self, ch: char) -> Result<(), Error> {
        let mut buf = [0; 4];
        let text = ch.encode_utf8(&mut buf);
        let character_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
        let text_style = TextStyleBuilder::new()
            .baseline(Baseline::Middle)
            .alignment(Alignment::Center)
            .build();
        Text::with_text_style(text, Point::new(64, 32), character_style, text_style)
            .draw(&mut self.driver)?;
        self.driver.flush()?;
        Ok(())
    }
}
