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
//! Pinout 3x4 Matrix Keypad
//!
//!  1   2    3    4    5    6    7    8   9
//!  nc Col2 Row1 Col1 Row4 Col3 Row3 Row2 nc
//!

#![no_std]
#![no_main]

mod display;
mod keypad;
mod wifi;

use embassy_executor::Spawner;
use embassy_time::{with_timeout, Duration};
use esp_backtrace as _;
use esp_hal::clock::ClockControl;
use esp_hal::gpio::{AnyInput, AnyOutput, Io, Level, Output, Pull};
use esp_hal::i2c::I2C;
use esp_hal::peripherals::Peripherals;
use esp_hal::prelude::*;
use esp_hal::rng::Rng;
use esp_hal::system::SystemControl;
use esp_hal::timer::{
    systimer::SystemTimer, timg::TimerGroup, ErasedTimer, OneShotTimer, PeriodicTimer,
};
use log::info;

// When you are okay with using a nightly compiler it's better to use https://docs.rs/static_cell/2.1.0/static_cell/macro.make_static.html
macro_rules! mk_static {
    ($t:ty, $val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

#[main]
async fn main(_spawner: Spawner) {
    let peripherals = Peripherals::take();
    let system = SystemControl::new(peripherals.SYSTEM);
    let clocks = ClockControl::max(system.clock_control).freeze();
    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);

    let rng = Rng::new(peripherals.RNG);
    let mut led = Output::new(io.pins.gpio8, Level::High);

    // Initialize async executor
    let systimer = SystemTimer::new(peripherals.SYSTIMER);
    let embassy_timer = OneShotTimer::new(systimer.alarm0.into());
    esp_hal_embassy::init(
        &clocks,
        mk_static!([OneShotTimer<ErasedTimer>; 1], [embassy_timer]),
    );

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
    let mut display = match display::Display::new(i2c, 0x3c) {
        Ok(disp) => disp,
        // Panic on failure since without a display there's no reasonable way to tell the user
        Err(err) => panic!("Display initialization failed: {:?}", err),
    };

    // Initialize keypad
    let mut keypad = keypad::Keypad::new(
        [
            AnyInput::new(io.pins.gpio5, Pull::Up),
            AnyInput::new(io.pins.gpio6, Pull::Up),
            AnyInput::new(io.pins.gpio7, Pull::Up),
        ],
        [
            AnyOutput::new(io.pins.gpio0, Level::High),
            AnyOutput::new(io.pins.gpio1, Level::High),
            AnyOutput::new(io.pins.gpio2, Level::High),
            AnyOutput::new(io.pins.gpio3, Level::High),
        ],
    );

    // Initialize Wifi
    let timg0 = TimerGroup::new(peripherals.TIMG0, &clocks, None);
    let wifi_timer = PeriodicTimer::new(timg0.timer0.into());
    let mut wifi = match wifi::Wifi::new(
        wifi_timer,
        rng,
        peripherals.RADIO_CLK,
        &clocks,
        peripherals.WIFI,
    )
    .await
    {
        Ok(wifi) => wifi,
        // Panic on failure since an initialization error indicates a static configuration error
        Err(err) => panic!("Wifi initialization failed: {:?}", err),
    };

    // Display hello screen
    display.clear().unwrap();
    display.hello().unwrap();

    // Test Wifi
    wifi.test().await;

    let mut displaying_key = false;
    loop {
        led.toggle();

        match with_timeout(Duration::from_secs(5), keypad.read()).await {
            Ok(Ok(key)) => {
                info!("Key pressed: {:?}", key);
                display.clear().unwrap();
                display.big_centered_char(key.as_char()).unwrap();
                displaying_key = true;
            }
            Err(_) if displaying_key => {
                display.clear().unwrap();
                display.hello().unwrap();
                displaying_key = false;
            }
            _ => {}
        }
    }
}
