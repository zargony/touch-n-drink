//! LSC Touch 'n Drink Firmware
//!
//! Pinout ESP32-C3 Super Mini board
//!
//!                 | USB |
//! A5/MISO/GPIO5 -  5 | 5V - 5V
//!    MOSI/GPIO6 -  6 | G  - GND
//!      SS/GPIO7 -  7 | 33 - 3V3
//!     SDA/GPIO8 -  8 | 4  - GPIO4/A4/SCK
//!     SCL/GPIO9 -  9 | 3  - GPIO3/A3
//!        GPIO10 - 10 | 2  - GPIO2/A2
//!     RX/GPIO20 - 20 | 1  - GPIO1/A1
//!     TX/GPIO21 - 21 | 0  - GPIO0/A0
//!

#![no_std]
#![no_main]

use esp_backtrace as _;
use esp_hal::clock::ClockControl;
use esp_hal::delay::Delay;
use esp_hal::entry;
use esp_hal::gpio::{Io, Level, Output};
use esp_hal::peripherals::Peripherals;
use esp_hal::system::SystemControl;

#[entry]
fn main() -> ! {
    let peripherals = Peripherals::take();
    let system = SystemControl::new(peripherals.SYSTEM);
    let clocks = ClockControl::max(system.clock_control).freeze();
    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);

    esp_println::logger::init_logger_from_env();

    let mut led = Output::new(io.pins.gpio8, Level::High);
    let delay = Delay::new(&clocks);

    loop {
        led.toggle();
        delay.delay_millis(500);
    }
}
