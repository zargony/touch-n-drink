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
mod keypad;
mod nfc;
mod pn532;
mod update;
mod wifi;

use alloc::vec;
use common::{GIT_SHA_STR, VERSION_STR, article, schedule, telemetry, time, user, vereinsflieger};
use core::convert::Infallible;
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::efuse::Efuse;
use esp_hal::gpio::{DriveMode, Input, InputConfig, Level, Output, OutputConfig, Pull};
use esp_hal::i2c::master::{BusTimeout, Config as I2cConfig, I2c};
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::peripherals::Peripherals;
use esp_hal::rng::Rng;
use esp_hal::rtc_cntl::{Rtc, RwdtStage};
use esp_hal::time::{Duration as EspDuration, Rate};
use esp_hal::timer::timg::TimerGroup;
use esp_println::println;
use esp_storage::FlashStorage;
use log::{debug, info, warn};
use rand_core::RngCore;
use reqwless::client::{HttpClient, TlsConfig, TlsVerify};

extern crate alloc;

esp_bootloader_esp_idf::esp_app_desc!();

/// Delay in seconds after which to restart on panic
#[cfg(not(debug_assertions))]
const PANIC_RESTART_DELAY: EspDuration = EspDuration::from_secs(10);
#[cfg(debug_assertions)]
const PANIC_RESTART_DELAY: EspDuration = EspDuration::from_secs(600);

/// Hardware watchdog timeout. RWDT will reset the system if not fed within this time.
const WATCHDOG_TIMEOUT: Duration = Duration::from_secs(10);

/// Custom halt function for esp-backtrace. Called after panic was handled and should halt
/// or restart the system.
#[unsafe(export_name = "custom_halt")]
unsafe fn halt() -> ! {
    unsafe {
        // System may be in any state at this time, thus everything is unsafe here. Stealing the
        // peripherals handle allows us to try to notify the user about this abnormal state and
        // restart the system. Any error should be ignored.
        let peripherals = Peripherals::steal();

        // TODO: Steal display driver and show a panic message to the user

        // Restart automatically after a delay
        println!("Restarting in {} seconds...", PANIC_RESTART_DELAY.as_secs());
        let mut rtc = Rtc::new(peripherals.LPWR);
        rtc.rwdt.set_timeout(RwdtStage::Stage0, PANIC_RESTART_DELAY);
        rtc.rwdt.unlisten();
        rtc.rwdt.enable();
        loop {
            esp_hal::riscv::asm::wfi();
        }
    }
}

#[embassy_executor::task]
async fn watchdog(mut rtc: Rtc<'static>) -> ! {
    debug!("Start watchdog task");

    // Enable watchdog
    rtc.rwdt.set_timeout(
        RwdtStage::Stage0,
        EspDuration::from_micros(WATCHDOG_TIMEOUT.as_micros()),
    );
    rtc.rwdt.listen();
    rtc.rwdt.enable();

    // Periodically feed watchdog
    let timeout = WATCHDOG_TIMEOUT / 2;
    loop {
        Timer::after(timeout).await;
        rtc.rwdt.feed();
    }
}

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let esp_config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(esp_config);
    let mut rng = Rng::new();
    let _led = Output::new(peripherals.GPIO8, Level::High, OutputConfig::default());

    // Initialize global allocator
    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 64 * 1024);
    esp_alloc::heap_allocator!(size: 150 * 1024);

    // Initialize async executor
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_int = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_int.software_interrupt0);

    // Initialize logging
    esp_println::logger::init_logger_from_env();
    info!("Touch 'n Drink v{VERSION_STR} ({GIT_SHA_STR})");

    // Start feeding the watchdog
    let rtc = Rtc::new(peripherals.LPWR);
    debug!("Spawning watchdog task...");
    spawner
        .spawn(watchdog(rtc))
        // Panic on failure since failing to spawn a task indicates a serious error
        .expect("Failed to spawn watchdog task");

    // Read system configuration
    let mut flash = FlashStorage::new(peripherals.FLASH);
    let config = config::Config::read(&mut flash);

    // Initialize I2C controller
    let i2c_config = I2cConfig::default()
        // Standard-Mode: 100 kHz, Fast-Mode: 400 kHz
        .with_frequency(Rate::from_khz(400))
        // Reset bus after 24 bus clock cycles (60 µs) of inactivity
        .with_timeout(BusTimeout::BusCycles(24));
    let i2c = I2c::new(peripherals.I2C0, i2c_config)
        // Panic on failure since without I2C there's no reasonable way to tell the user
        .expect("I2C initialization failed")
        .with_sda(peripherals.GPIO9)
        .with_scl(peripherals.GPIO10)
        .into_async();

    // Share I2C bus. Since the mcu is single-core and I2C is not used in interrupts, I2C access
    // cannot be preempted and we can safely use a NoopMutex for shared access.
    let i2c: Mutex<NoopRawMutex, _> = Mutex::new(i2c);

    // Initialize display
    let display = display::Display::new(I2cDevice::new(&i2c))
        .await
        // Panic on failure since without a display there's no reasonable way to tell the user
        .expect("Display initialization failed");

    // Initialize keypad
    let keypad_input_config = InputConfig::default().with_pull(Pull::Up);
    let keypad_output_config = OutputConfig::default().with_drive_mode(DriveMode::OpenDrain);
    let keypad = keypad::Keypad::new(
        [
            Input::new(peripherals.GPIO5, keypad_input_config),
            Input::new(peripherals.GPIO6, keypad_input_config),
            Input::new(peripherals.GPIO7, keypad_input_config),
        ],
        [
            Output::new(peripherals.GPIO0, Level::High, keypad_output_config),
            Output::new(peripherals.GPIO1, Level::High, keypad_output_config),
            Output::new(peripherals.GPIO2, Level::High, keypad_output_config),
            Output::new(peripherals.GPIO3, Level::High, keypad_output_config),
        ],
    );

    // Initialize NFC reader
    let nfc_irq_input_config = InputConfig::default().with_pull(Pull::Up);
    let nfc_irq = Input::new(peripherals.GPIO20, nfc_irq_input_config);
    let nfc = nfc::Nfc::new(I2cDevice::new(&i2c), nfc_irq)
        .await
        // Panic on failure since an initialization error indicates a serious error
        .expect("NFC reader initialization failed");

    // Initialize buzzer
    let buzzer = buzzer::Buzzer::new(peripherals.LEDC, peripherals.GPIO4);

    // Initialize real time clock
    let rtc = time::Rtc::new();

    // Initialize article and user look up tables
    let articles = article::Articles::new(config.vf_article_ids);
    let users = user::Users::new();

    // Initialize scheduler
    let schedule = schedule::Daily::new();

    // Initialize Wifi
    let wifi = wifi::Wifi::new(
        rng,
        peripherals.WIFI,
        spawner,
        config.wifi_country,
        &config.wifi_ssid,
        &config.wifi_password,
    )
    // Panic on failure since an initialization error indicates a static configuration error
    .expect("Wifi initialization failed");

    // Initialize HTTP client
    // As this allocates quite a bit of memory (e.g. for TLS buffers), only a single http client
    // is created that can be passed to an API client whenever a connection needs to be established
    // TLS read buffer needs to fit an encrypted TLS record. Actual size depends on server
    // configuration. Maximum allowed value for a TLS record is 16640, so this is a safe amount.
    let mut tls_read_buffer = vec![0; 16640].into_boxed_slice();
    let mut tls_write_buffer = vec![0; 2048].into_boxed_slice();
    // FIXME: reqwless with embedded-tls can't verify TLS certificates (though pinning is
    // supported). This is bad since it makes communication vulnerable to MITM attacks.
    let tls_config = TlsConfig::new(
        rng.next_u64(),
        &mut tls_read_buffer,
        &mut tls_write_buffer,
        TlsVerify::None,
    );
    let http = HttpClient::new_with_tls(wifi.tcp(), wifi.dns(), tls_config);

    // Initialize Vereinsflieger API client
    let vereinsflieger = vereinsflieger::Vereinsflieger::new(
        &config.vf_username,
        &config.vf_password_md5,
        &config.vf_appkey,
        config.vf_cid,
    );

    // Initialize telemetry
    let device_id: const_hex::Buffer<6, false> =
        const_hex::Buffer::new().const_format(&Efuse::read_base_mac_address());
    let telemetry = telemetry::Telemetry::new(config.mp_token.as_deref(), device_id.as_str());

    // Initialize firmware updater
    let mut updater_buffer = [0; update::BUFFER_SIZE];
    let updater = match update::Updater::new(&mut flash, &mut updater_buffer) {
        Ok(updater) => Some(updater),
        Err(err) => {
            warn!("Firmware updates unavailable: {err}");
            None
        }
    };

    // Prepare firmware frontend and backend
    let mut frontend = common::FrontendResources::<Frontend> {
        display,
        keypad,
        nfc,
        buzzer,
    };
    let mut backend = common::BackendResources::<Backend> {
        rng,
        rtc,
        network: &wifi,
        articles,
        users,
        schedule,
        http,
        vereinsflieger,
        telemetry,
        updater,
    };

    // Run firmware
    common::run(&mut frontend, &mut backend).await
}

/// Firmware frontend
pub struct Frontend;

impl common::Frontend for Frontend {
    type DisplayError = display::Error;
    type KeypadError = Infallible;
    type NfcError = nfc::Error;

    type Display<'a> = display::Display<I2cDevice<'a, NoopRawMutex, I2c<'a, esp_hal::Async>>>;
    type Keypad<'a> = keypad::Keypad<'a, 3, 4>;
    type NfcReader<'a> = nfc::Nfc<I2cDevice<'a, NoopRawMutex, I2c<'a, esp_hal::Async>>, Input<'a>>;
    type Buzzer<'a> = buzzer::Buzzer<'a>;
}

/// Firmware backend
struct Backend;

impl common::Backend for Backend {
    type Rng<'a> = Rng;
    type Network<'a> = &'a wifi::Wifi;
    type Updater<'a> = update::Updater<'a>;
}
