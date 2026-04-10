#![no_std]
#![expect(async_fn_in_trait)]

pub mod article;
pub mod mixpanel;
pub mod nfc;
pub mod ota;
pub mod reader;
pub mod schedule;
pub mod telemetry;
pub mod time;
pub mod ui;
pub mod user;
pub mod util;
pub mod vereinsflieger;

use core::fmt;
use embassy_time::Timer;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::DrawTarget;
use embedded_nal_async::{Dns, TcpConnect};
use embedded_storage::Storage;
use log::debug;
use rand_core::RngCore;

extern crate alloc;

pub static VERSION_STR: &str = env!("CARGO_PKG_VERSION");
pub static GIT_SHA_STR: &str = env!("GIT_SHORT_SHA");

/// Display interface
pub trait Display: DrawTarget<Color = BinaryColor, Error: fmt::Debug + fmt::Display> {
    /// Flush the graphics buffer, making drawn graphics visible on the physical display.
    /// This is also expected to stop power saving and turn on the display again
    async fn flush(&mut self) -> Result<(), Self::Error>;

    /// Save power by turning off the display
    async fn power_save(&mut self);
}

/// Keypad interface
pub trait Keypad {
    /// Keypad error type
    type Error: fmt::Debug + fmt::Display;

    /// Read key input from keypad
    async fn read(&mut self) -> Result<char, Self::Error>;
}

/// NFC Reader interface
pub trait NfcReader {
    /// NFC reader error type
    type Error: fmt::Debug + fmt::Display;

    /// Read uid from NFC reader
    async fn read(&mut self) -> Result<nfc::Uid, Self::Error>;
}

/// Buzzer interface
pub trait Buzzer {
    /// Output the given tone for given duration
    async fn tone(&mut self, frequency: u32, duration: u64);

    /// Output startup/testing tone
    async fn startup(&mut self) {
        debug!("Buzzer: Playing startup tone");
        self.tone(3136, 1000).await; // G7
    }

    /// Output a short confirmation tone
    async fn confirm(&mut self) {
        debug!("Buzzer: Playing confirm tone");
        self.tone(3136, 100).await; // G7
    }

    /// Output a long denying tone
    async fn deny(&mut self) {
        debug!("Buzzer: Playing deny tone");
        self.tone(392, 500).await; // G4
    }

    /// Output an error tone
    async fn error(&mut self) {
        debug!("Buzzer: Playing error tone");
        self.tone(784, 200).await; // G5
        Timer::after_millis(10).await;
        self.tone(587, 200).await; // D5
        Timer::after_millis(10).await;
        self.tone(392, 500).await; // G4
    }
}

/// Network stack interface
pub trait Network {
    /// TCP client type
    type TcpConnect<'a>: TcpConnect;
    /// DNS resolver type
    type Dns<'a>: Dns;

    /// Return whether network stack is up
    fn is_up(&self) -> bool;

    /// Wait for network stack to come up
    async fn wait_up(&self);
}

/// Firmware update interface
pub trait Updater {
    /// Firmware variant (xtask --chip) for OTA update image downloads
    const FIRMWARE_VARIANT: &'static str;

    /// Firmware update error type
    type Error: fmt::Debug + fmt::Display;
    /// Flash region type
    type Region<'a>: Storage
    where
        Self: 'a;

    /// Flash region to write new firmware to
    ///
    /// # Errors
    ///
    /// An error will be returned if the region to write to could not be determined.
    fn region(&mut self) -> Result<Self::Region<'_>, Self::Error>;

    /// Confirm firmware update written, switch to new firmware for next system start
    ///
    /// # Errors
    ///
    /// An error will be returned if the firmware update could not be committed.
    fn commit(&mut self) -> Result<(), Self::Error>;

    /// Cancel firmware update
    fn cancel(&mut self);

    /// Restart system
    fn restart() -> !;

    /// Whether the system was recently restarted after being updated. Can be used to avoid update
    /// loops if updates are checked on restart and something went wrong with release versioning.
    fn recently_restarted() -> bool;
}

/// Firmware frontend
pub trait Frontend {
    /// Display error type
    type DisplayError: fmt::Debug + fmt::Display;
    /// Keypad error type
    type KeypadError: fmt::Debug + fmt::Display;
    /// NFC reader error type
    type NfcError: fmt::Debug + fmt::Display;

    /// Display
    type Display<'a>: Display<Error = Self::DisplayError>;
    /// Keypad
    type Keypad<'a>: Keypad<Error = Self::KeypadError>;
    /// NFC reader
    type NfcReader<'a>: NfcReader<Error = Self::NfcError>;
    /// Buzzer
    type Buzzer<'a>: Buzzer;
}

/// Firmware frontend resources
#[must_use]
pub struct FrontendResources<'fe, FE: Frontend> {
    pub display: FE::Display<'fe>,
    pub keypad: FE::Keypad<'fe>,
    pub nfc: FE::NfcReader<'fe>,
    pub buzzer: FE::Buzzer<'fe>,
}

/// Firmware backend
pub trait Backend {
    /// Random number generator type
    type Rng<'a>: RngCore;
    /// Network stack type
    type Network<'a>: Network;
    /// Firmware updater type
    type Updater<'a>: Updater;
}

/// Firmware backend resources
#[must_use]
pub struct BackendResources<'be, BE: Backend> {
    pub rng: BE::Rng<'be>,
    pub rtc: time::Rtc,
    pub network: BE::Network<'be>,
    pub articles: article::Articles,
    pub users: user::Users,
    pub schedule: schedule::Daily,
    pub http: reqwless::client::HttpClient<
        'be,
        <BE::Network<'be> as Network>::TcpConnect<'be>,
        <BE::Network<'be> as Network>::Dns<'be>,
    >,
    pub vereinsflieger: vereinsflieger::Vereinsflieger<'be>,
    pub telemetry: telemetry::Telemetry<'be>,
    pub updater: Option<BE::Updater<'be>>,
}

/// Run firmware
pub async fn run<FE: Frontend, BE: Backend>(
    frontend: &mut FrontendResources<'_, FE>,
    backend: &mut BackendResources<'_, BE>,
) -> ! {
    ui::run(frontend, backend).await;
}
