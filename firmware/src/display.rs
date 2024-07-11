use display_interface_spi::SPIInterface;
use embassy_time::Delay;
use embedded_graphics::mono_font::ascii::FONT_6X13;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::text::Text;
use embedded_hal::digital::OutputPin;
use embedded_hal::spi::SpiBus;
use embedded_hal_bus::spi::ExclusiveDevice;
use ssd1309::mode::GraphicsMode;
use ssd1309::NoOutputPin;

// The `ssd1309` crate unfortunately doesn't support async yet (though `display-interface`,
// `display-interface-spi` and `embedded-hal-bus` do), so we can't use async here yet.

/// Display error
pub use display_interface::DisplayError;

/// Convenient hardware-agnostic display driver
pub struct Display<BUS: SpiBus, DC: OutputPin> {
    driver: GraphicsMode<SPIInterface<ExclusiveDevice<BUS, NoOutputPin, Delay>, DC>>,
}

impl<BUS: SpiBus, DC: OutputPin> Display<BUS, DC> {
    /// Create display driver and initialize display hardware
    pub fn new<RES: OutputPin>(bus: BUS, mut reset: RES, dc: DC) -> Result<Self, DisplayError> {
        // We're exclusively using the SPI bus without CS
        let cs = NoOutputPin::new();
        let spi = ExclusiveDevice::new(bus, cs, Delay).map_err(|_| DisplayError::CSError)?;

        // Build SSD1309 driver and switch to graphics mode
        let mut driver: GraphicsMode<_> = ssd1309::Builder::default()
            .connect(SPIInterface::new(spi, dc))
            .into();

        // Reset and initialize display
        driver
            .reset(&mut reset, &mut Delay)
            .map_err(|_| DisplayError::RSError)?;
        driver.init()?;
        driver.clear();
        driver.flush()?;

        Ok(Display { driver })
    }

    /// Clear display
    pub fn clear(&mut self) -> Result<(), DisplayError> {
        self.driver.clear();
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
