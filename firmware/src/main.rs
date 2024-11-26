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

mod article;
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
mod time;
mod ui;
mod user;
mod vereinsflieger;
mod wifi;

use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Input, Level, Output, OutputOpenDrain, Pull};
use esp_hal::i2c::master::{Config as I2cConfig, I2c};
use esp_hal::peripherals::Peripherals;
use esp_hal::prelude::*;
use esp_hal::rng::Rng;
use esp_hal::rtc_cntl::{Rtc, RwdtStage};
use esp_hal::timer::systimer::{SystemTimer, Target};
use esp_hal::timer::timg::TimerGroup;
use esp_println::println;
use log::{error, info};
use rand_core::RngCore;

extern crate alloc;

static VERSION_STR: &str = concat!("v", env!("CARGO_PKG_VERSION"));
static GIT_SHA_STR: &str = env!("GIT_SHORT_SHA");

/// Delay in seconds after which to restart on panic
#[cfg(not(debug_assertions))]
const PANIC_RESTART_DELAY_SECS: u64 = 10;
#[cfg(debug_assertions)]
const PANIC_RESTART_DELAY_SECS: u64 = 600;

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
    println!("Restarting in {} seconds...", PANIC_RESTART_DELAY_SECS);
    let mut rtc = Rtc::new(peripherals.LPWR);
    rtc.rwdt
        .set_timeout(RwdtStage::Stage0, PANIC_RESTART_DELAY_SECS.secs());
    rtc.rwdt.unlisten();
    rtc.rwdt.enable();
    loop {
        esp_hal::riscv::asm::wfi();
    }
}

#[main]
async fn main(spawner: Spawner) {
    let peripherals = esp_hal::init({
        let mut config = esp_hal::Config::default();
        config.cpu_clock = CpuClock::max();
        config
    });
    let mut rng = Rng::new(peripherals.RNG);
    let _led = Output::new(peripherals.GPIO8, Level::High);

    // Initialize global allocator
    esp_alloc::heap_allocator!(100 * 1024);

    // Initialize async executor
    let systimer = SystemTimer::new(peripherals.SYSTIMER).split::<Target>();
    esp_hal_embassy::init(systimer.alarm0);

    // Initialize logging
    esp_println::logger::init_logger_from_env();
    info!("Touch 'n Drink {VERSION_STR} ({GIT_SHA_STR})");

    // Read system configuration
    let config = config::Config::read().await;

    // Initialize article and user look up tables
    let mut articles = article::Articles::new([config.vf_article_id]);
    let mut users = user::Users::new();

    // Initialize I2C controller
    let i2c = I2c::new(
        peripherals.I2C0,
        I2cConfig {
            frequency: 400.kHz(), // Standard-Mode: 100 kHz, Fast-Mode: 400 kHz
            timeout: Some(24),    // Reset bus after 24 bus clock cycles (60 µs) of inactivity
        },
    )
    .with_sda(peripherals.GPIO9)
    .with_scl(peripherals.GPIO10)
    .into_async();

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
            Input::new(peripherals.GPIO5, Pull::Up),
            Input::new(peripherals.GPIO6, Pull::Up),
            Input::new(peripherals.GPIO7, Pull::Up),
        ],
        [
            OutputOpenDrain::new(peripherals.GPIO0, Level::High, Pull::None),
            OutputOpenDrain::new(peripherals.GPIO1, Level::High, Pull::None),
            OutputOpenDrain::new(peripherals.GPIO2, Level::High, Pull::None),
            OutputOpenDrain::new(peripherals.GPIO3, Level::High, Pull::None),
        ],
    );

    // Initialize NFC reader
    let nfc_irq = Input::new(peripherals.GPIO20, Pull::Up);
    let mut nfc = match nfc::Nfc::new(I2cDevice::new(&i2c), nfc_irq).await {
        Ok(nfc) => nfc,
        // Panic on failure since an initialization error indicates a serious error
        Err(err) => panic!("NFC reader initialization failed: {:?}", err),
    };

    // Initialize Wifi
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let wifi = match wifi::Wifi::new(
        timg0.timer0,
        rng,
        peripherals.RADIO_CLK,
        peripherals.WIFI,
        spawner,
        &config.wifi_ssid,
        &config.wifi_password,
    ) {
        Ok(wifi) => wifi,
        // Panic on failure since an initialization error indicates a static configuration error
        Err(err) => panic!("Wifi initialization failed: {:?}", err),
    };

    // Initialize HTTP client
    // As this allocates quite a bit of memory (e.g. for TLS buffers), only a single http client
    // is created that can be passed to an API client whenever a connection needs to be established
    let mut http_resources = http::Resources::new();
    let mut http = http::Http::new(&wifi, rng.next_u64(), &mut http_resources);

    // Initialize Vereinsflieger API client
    let mut vereinsflieger = vereinsflieger::Vereinsflieger::new(
        &config.vf_username,
        &config.vf_password_md5,
        &config.vf_appkey,
        config.vf_cid,
    );

    // Initialize buzzer
    let mut buzzer = buzzer::Buzzer::new(peripherals.LEDC, peripherals.GPIO4);
    let _ = buzzer.startup().await;

    // Create UI
    let mut ui = ui::Ui::new(
        rng,
        &mut display,
        &mut keypad,
        &mut nfc,
        &mut buzzer,
        &wifi,
        &mut http,
        &mut vereinsflieger,
        &mut articles,
        &mut users,
    );

    // Show splash screen for a while, ignore any error
    let _ = ui.show_splash().await;

    loop {
        match ui.init().await {
            // Success or cancel: continue
            Ok(()) | Err(error::Error::Cancel) => break,
            // Display error to user and try again
            Err(err) => {
                error!("Initialization error: {:?}", err);
                let _ = ui.show_error(&err, false).await;
            }
        }
    }

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
                error!("User flow error: {:?}", err);
                let _ = ui.show_error(&err, true).await;
            }
        }
    }
}
