use crate::{GIT_SHA_STR, VERSION_STR};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_hal::i2c::I2c;
use log::{debug, info};
use ssd1306::mode::{BufferedGraphicsMode, DisplayConfig};
use ssd1306::prelude::I2CInterface;
use ssd1306::rotation::DisplayRotation;
use ssd1306::size::DisplaySize128x64;
use ssd1306::Ssd1306;
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};
use u8g2_fonts::{fonts, FontRenderer};

// The `ssd1306` crate unfortunately doesn't support async yet (though `display-interface`,
// `display-interface-i2c` and `embedded-hal-bus` do), so we can't use async here yet.
// See also https://github.com/rust-embedded-community/ssd1306/pull/189

const SPLASH_TITLE_FONT: FontRenderer = FontRenderer::new::<fonts::u8g2_font_logisoso16_tr>();
const SPLASH_VERSION_FONT: FontRenderer = FontRenderer::new::<fonts::u8g2_font_profont12_tr>();
const SPLASH_FOOTER_FONT: FontRenderer = FontRenderer::new::<fonts::u8g2_font_profont11_tr>();
const BIG_CHAR_FONT: FontRenderer = FontRenderer::new::<fonts::u8g2_font_luBS24_tr>();

/// Display error
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// Display driver error
    #[allow(dead_code)]
    DriverError(display_interface::DisplayError),
    /// Font render error
    #[allow(dead_code)]
    FontRenderError(u8g2_fonts::Error<display_interface::DisplayError>),
}

impl From<display_interface::DisplayError> for Error {
    fn from(err: display_interface::DisplayError) -> Self {
        Self::DriverError(err)
    }
}

impl From<u8g2_fonts::Error<display_interface::DisplayError>> for Error {
    fn from(err: u8g2_fonts::Error<display_interface::DisplayError>) -> Self {
        Self::FontRenderError(err)
    }
}

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

        info!("Display: SSD1306 initialized");

        Ok(Display { driver })
    }

    /// Turn display off
    pub fn turn_off(&mut self) -> Result<(), Error> {
        debug!("Display: Power off");
        self.driver.set_display_on(false)?;
        Ok(())
    }

    /// Clear display
    #[allow(dead_code)]
    pub fn clear(&mut self) -> Result<(), Error> {
        self.driver.clear(BinaryColor::Off)?;
        self.driver.flush()?;
        self.driver.set_display_on(true)?;
        Ok(())
    }

    /// Display splash screen
    pub fn splash(&mut self) -> Result<(), Error> {
        self.driver.clear(BinaryColor::Off)?;
        // TODO: Temporary title, replace with proper bitmap logo
        SPLASH_TITLE_FONT.render_aligned(
            "Touch'n Drink",
            Point::new(63, 28),
            VerticalPosition::Baseline,
            HorizontalAlignment::Center,
            FontColor::Transparent(BinaryColor::On),
            &mut self.driver,
        )?;
        SPLASH_VERSION_FONT.render_aligned(
            VERSION_STR,
            Point::new(63, 28 + 12),
            VerticalPosition::Baseline,
            HorizontalAlignment::Center,
            FontColor::Transparent(BinaryColor::On),
            &mut self.driver,
        )?;
        SPLASH_FOOTER_FONT.render_aligned(
            GIT_SHA_STR,
            Point::new(127, 63),
            VerticalPosition::Baseline,
            HorizontalAlignment::Right,
            FontColor::Transparent(BinaryColor::On),
            &mut self.driver,
        )?;
        self.driver.flush()?;
        self.driver.set_display_on(true)?;
        Ok(())
    }

    /// Display big centered text
    pub fn big_centered_char(&mut self, ch: char) -> Result<(), Error> {
        self.driver.clear(BinaryColor::Off)?;
        BIG_CHAR_FONT.render_aligned(
            ch,
            Point::new(63, 42),
            VerticalPosition::Baseline,
            HorizontalAlignment::Center,
            FontColor::Transparent(BinaryColor::On),
            &mut self.driver,
        )?;
        self.driver.flush()?;
        self.driver.set_display_on(true)?;
        Ok(())
    }
}
