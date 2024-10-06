//! Touch 'n Drink Firmware
//!
//! Pinout ESP32-C3 Super Mini board
//!
//!                               | USB |
//! (Keypad Col1) A5/MISO/GPIO5 -  5 | 5V - 5V
//! (Keypad Col2)    MOSI/GPIO6 -  6 | G  - GND
//! (Keypad Col3)      SS/GPIO7 -  7 | 33 - 3V3
//!   (Board LED)     SDA/GPIO8 -  8 | 4  - GPIO4/A4/SCK (Buzzer)
//!     (I2C SDA)     SCL/GPIO9 -  9 | 3  - GPIO3/A3     (Keypad Row4)
//!     (I2C SCL)        GPIO10 - 10 | 2  - GPIO2/A2     (Keypad Row3)
//!     (NFC IRQ)     RX/GPIO20 - 20 | 1  - GPIO1/A1     (Keypad Row2)
//!                   TX/GPIO21 - 21 | 0  - GPIO0/A0     (Keypad Row1)
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
//! Pinout PN532 NFC Module
//!
//!             SCK MISO MOSI SS VCC GND IRQ RSTO
//!              1   2    3   4   5   6   7   8
//! GND - 1 |
//! VCC - 2 |
//! SDA - 3 |
//! SCL - 4 |
//!

#![no_std]
#![no_main]

mod buzzer;
mod config;
mod display;
mod error;
mod http;
mod json;
mod keypad;
mod nfc;
mod pn532;
mod screen;
mod ui;
mod wifi;

use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use esp_backtrace as _;
use esp_hal::clock::ClockControl;
use esp_hal::gpio::{AnyInput, AnyOutput, AnyOutputOpenDrain, AnyPin, Io, Level, Pull};
use esp_hal::i2c::I2C;
use esp_hal::peripherals::Peripherals;
use esp_hal::prelude::*;
use esp_hal::rng::Rng;
use esp_hal::rtc_cntl::Rtc;
use esp_hal::system::SystemControl;
use esp_hal::timer::systimer::{SystemTimer, Target};
use esp_hal::timer::timg::TimerGroup;
use esp_println::println;
use log::{error, info};

extern crate alloc;

static VERSION_STR: &str = concat!("v", env!("CARGO_PKG_VERSION"));
static GIT_SHA_STR: &str = env!("GIT_SHORT_SHA");

/// Custom halt function for esp-backtrace. Called after panic was handled and should halt
/// or restart the system.
#[export_name = "custom_halt"]
unsafe fn halt() -> ! {
    // System may be in any state at this time, thus everything is unsafe here. Stealing the
    // peripherals handle allows us to try to notify the user about this abnormal state and
    // restart the system. Any error should be ignored.
    let peripherals = Peripherals::steal();

    // TODO: Steal display driver and show a panic message to the user

    // Restart automatically after a delay
    println!("Restarting in 10 seconds...");
    let mut rtc = Rtc::new(peripherals.LPWR);
    rtc.rwdt.set_timeout(10_000.millis());
    rtc.rwdt.unlisten();
    rtc.rwdt.enable();
    loop {
        esp_hal::riscv::asm::wfi();
    }
}

#[main]
async fn main(spawner: Spawner) {
    let peripherals = Peripherals::take();
    let system = SystemControl::new(peripherals.SYSTEM);
    let clocks = ClockControl::boot_defaults(system.clock_control).freeze();
    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);
    let rng = Rng::new(peripherals.RNG);
    let _led = AnyOutput::new(io.pins.gpio8, Level::High);

    // Initialize global allocator
    esp_alloc::heap_allocator!(72 * 1024);

    // Initialize async executor
    let systimer = SystemTimer::new(peripherals.SYSTIMER);
    let systimer_alarms = systimer.split::<Target>();
    esp_hal_embassy::init(&clocks, systimer_alarms.alarm0);

    // Initialize logging
    esp_println::logger::init_logger_from_env();
    info!("Touch 'n Drink {VERSION_STR} ({GIT_SHA_STR})");

    // Read system configuration
    let config = config::Config::read().await;

    // Initialize I2C controller
    let i2c = I2C::new_with_timeout_async(
        peripherals.I2C0,
        io.pins.gpio9,
        io.pins.gpio10,
        400.kHz(), // Standard-Mode: 100 kHz, Fast-Mode: 400 kHz
        &clocks,
        Some(20),
    );
    // Share I2C bus. Since the mcu is single-core and I2C is not used in interrupts, I2C access
    // cannot be preempted and we can safely use a NoopMutex for shared access.
    let i2c: Mutex<NoopRawMutex, _> = Mutex::new(i2c);

    // Initialize display
    let mut display = match display::Display::new(I2cDevice::new(&i2c)).await {
        Ok(disp) => disp,
        // Panic on failure since without a display there's no reasonable way to tell the user
        Err(err) => panic!("Display initialization failed: {:?}", err),
    };
    let _ = display.screen(&screen::Splash).await;

    // Initialize keypad
    let mut keypad = keypad::Keypad::new(
        [
            AnyInput::new(io.pins.gpio5, Pull::Up),
            AnyInput::new(io.pins.gpio6, Pull::Up),
            AnyInput::new(io.pins.gpio7, Pull::Up),
        ],
        [
            AnyOutputOpenDrain::new(io.pins.gpio0, Level::High, Pull::None),
            AnyOutputOpenDrain::new(io.pins.gpio1, Level::High, Pull::None),
            AnyOutputOpenDrain::new(io.pins.gpio2, Level::High, Pull::None),
            AnyOutputOpenDrain::new(io.pins.gpio3, Level::High, Pull::None),
        ],
    );

    // Initialize NFC reader
    let nfc_irq = AnyInput::new(io.pins.gpio20, Pull::Up);
    let mut nfc = match nfc::Nfc::new(I2cDevice::new(&i2c), nfc_irq).await {
        Ok(nfc) => nfc,
        // Panic on failure since an initialization error indicates a serious error
        Err(err) => panic!("NFC reader initialization failed: {:?}", err),
    };

    // Initialize Wifi
    let timg0 = TimerGroup::new(peripherals.TIMG0, &clocks);
    let wifi = match wifi::Wifi::new(
        timg0.timer0,
        rng,
        peripherals.RADIO_CLK,
        &clocks,
        peripherals.WIFI,
        spawner,
        &config.wifi_ssid,
        &config.wifi_password,
    )
    .await
    {
        Ok(wifi) => wifi,
        // Panic on failure since an initialization error indicates a static configuration error
        Err(err) => panic!("Wifi initialization failed: {:?}", err),
    };

    // Initialize buzzer
    let buzzer_pin = AnyPin::new(io.pins.gpio4);
    let mut buzzer = buzzer::Buzzer::new(peripherals.LEDC, &clocks, buzzer_pin);
    let _ = buzzer.startup().await;

    // Create UI
    let mut ui = ui::Ui::new(&mut display, &mut keypad, &mut nfc, &mut buzzer, &wifi);

    // Show splash screen for a while, ignore any error
    let _ = ui.show_splash().await;

    // Wait for network to become available (if not already), ignore any error
    let _ = ui.wait_network_up().await;

    loop {
        match ui.run().await {
            // Success: start over again
            Ok(()) => (),
            // Cancel: start over again
            Err(error::Error::Cancel) => info!("User cancelled, starting over..."),
            // Timeout: start over again
            Err(error::Error::UserTimeout) => info!("Timeout waiting for user, starting over..."),
            // Display error to user and start over again
            Err(err) => {
                error!("Unhandled Error: {:?}", err);
                let _ = ui.show_error(&err).await;
            }
        }
    }
}
