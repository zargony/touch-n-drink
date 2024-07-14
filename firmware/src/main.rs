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
//! Pinout OLED 2.42" Display
//!
//!            1   2   3   4
//!           GND VDD SCL SDA
//! GND - 1 |
//! VDD - 2 |
//! SCL - 3 |
//! SDA - 4 |
//!

#![no_std]
#![no_main]

mod display;
mod wifi;

use embassy_executor::Spawner;
use embassy_time::Timer;
use esp_backtrace as _;
use esp_hal::clock::ClockControl;
use esp_hal::gpio::{Io, Level, Output};
use esp_hal::i2c::I2C;
use esp_hal::peripherals::Peripherals;
use esp_hal::prelude::*;
use esp_hal::rng::Rng;
use esp_hal::system::SystemControl;
use esp_hal::timer::{systimer::SystemTimer, timg::TimerGroup};

#[main]
async fn main(_spawner: Spawner) {
    let peripherals = Peripherals::take();
    let system = SystemControl::new(peripherals.SYSTEM);
    let clocks = ClockControl::max(system.clock_control).freeze();
    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);

    let rng = Rng::new(peripherals.RNG);
    let mut led = Output::new(io.pins.gpio8, Level::High);

    // Initialize async executor
    let timg0 = TimerGroup::new_async(peripherals.TIMG0, &clocks);
    esp_hal_embassy::init(&clocks, timg0);

    // Initialize logging
    esp_println::logger::init_logger_from_env();

    // Initialize I2C controller
    let i2c = I2C::new(
        peripherals.I2C0,
        io.pins.gpio9,
        io.pins.gpio10,
        100.kHz(),
        &clocks,
        None,
    );

    // Initialize display
    let mut display = display::Display::new(i2c, 0x3c).unwrap();

    // Display hello screen
    display.clear().unwrap();
    display.hello().unwrap();

    // Initialize Wifi
    let wifi_timer = SystemTimer::new(peripherals.SYSTIMER).alarm0;
    let mut wifi = wifi::Wifi::new(
        wifi_timer,
        rng,
        peripherals.RADIO_CLK,
        &clocks,
        peripherals.WIFI,
    )
    .await
    .unwrap();

    // Test Wifi
    wifi.test().await;

    loop {
        led.toggle();
        Timer::after_millis(500).await;
    }
}
