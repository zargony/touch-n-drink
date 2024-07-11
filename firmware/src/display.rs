use dummy_pin::DummyPin;
use embassy_time::Delay;
use embedded_graphics::mono_font::ascii::FONT_6X13;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::text::Text;
use embedded_hal::digital::OutputPin;
use embedded_hal::spi::SpiBus;
use embedded_hal_bus::spi::ExclusiveDevice;
use ssd1306::mode::{BufferedGraphicsMode, DisplayConfig};
use ssd1306::prelude::SPIInterface;
use ssd1306::rotation::DisplayRotation;
use ssd1306::size::DisplaySize128x64;
use ssd1306::Ssd1306;

// The `ssd1306` crate unfortunately doesn't support async yet (though `display-interface`,
// `display-interface-spi` and `embedded-hal-bus` do), so we can't use async here yet.
// See also https://github.com/rust-embedded-community/ssd1306/pull/189

/// Display error
pub use display_interface::DisplayError;

/// Display interface type
type DisplayInterface<BUS, DC> = SPIInterface<ExclusiveDevice<BUS, DummyPin, Delay>, DC>;

/// Convenient hardware-agnostic display driver
pub struct Display<BUS: SpiBus, DC: OutputPin> {
    driver: Ssd1306<
        DisplayInterface<BUS, DC>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
}

impl<BUS: SpiBus, DC: OutputPin> Display<BUS, DC> {
    /// Create display driver and initialize display hardware
    pub fn new<RES: OutputPin>(bus: BUS, mut reset: RES, dc: DC) -> Result<Self, DisplayError> {
        // We're exclusively using the SPI bus without CS
        let cs = DummyPin::new_low();
        let spi = ExclusiveDevice::new(bus, cs, Delay).map_err(|_| DisplayError::CSError)?;

        // Build SSD1306 driver and switch to buffered graphics mode
        let mut driver = Ssd1306::new(
            SPIInterface::new(spi, dc),
            DisplaySize128x64,
            DisplayRotation::Rotate0,
        )
        .into_buffered_graphics_mode();

        // Reset and initialize display
        driver
            .reset(&mut reset, &mut Delay)
            .map_err(|_| DisplayError::RSError)?;
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
