#![no_std]
#![expect(async_fn_in_trait)]

pub mod article;
pub mod config;
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

use alloc::vec;
use core::fmt;
use core::marker::PhantomData;
use embassy_time::Timer;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::DrawTarget;
use embedded_nal_async::{Dns, TcpConnect};
use embedded_storage::Storage;
use log::debug;
use rand_core::RngCore;
use reqwless::client::{HttpClient, TlsConfig, TlsVerify};

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
    /// DNS resolver type
    type DnsSocket: Dns;
    /// TCP client type
    type TcpClient: TcpConnect;

    /// Return whether network stack is up
    fn is_up(&self) -> bool;

    /// Wait for network stack to come up
    async fn wait_up(&self);

    /// Provide a DNS socket
    fn dns(&self) -> &'_ Self::DnsSocket;

    /// Provide a TCP client
    fn tcp(&self) -> &'_ Self::TcpClient;
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
trait Frontend {
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

/// Firmware frontend adapter that allows to move separate generics to a `Frontend` implementation
#[must_use]
struct FrontendAdapter<DP: Display, KP: Keypad, NFC: NfcReader, BZZ: Buzzer>(
    PhantomData<(DP, KP, NFC, BZZ)>,
);

impl<DP: Display, KP: Keypad, NFC: NfcReader, BZZ: Buzzer> Frontend
    for FrontendAdapter<DP, KP, NFC, BZZ>
{
    type DisplayError = DP::Error;
    type KeypadError = KP::Error;
    type NfcError = NFC::Error;

    type Display<'a> = DP;
    type Keypad<'a> = KP;
    type NfcReader<'a> = NFC;
    type Buzzer<'a> = BZZ;
}

/// Firmware frontend resources
#[must_use]
struct FrontendResources<'fe, FE: Frontend> {
    display: FE::Display<'fe>,
    keypad: FE::Keypad<'fe>,
    nfc: FE::NfcReader<'fe>,
    buzzer: FE::Buzzer<'fe>,
}

/// Firmware backend
trait Backend {
    /// Random number generator type
    type Rng<'a>: RngCore;
    /// Network stack type
    type Network<'a>: Network;
    /// Firmware updater type
    type Updater<'a>: Updater;
}

/// Firmware backend adapter that allows to move separate generics to a `Backend` implementation
#[must_use]
struct BackendAdapter<RNG: RngCore, NET: Network, UPD: Updater>(PhantomData<(RNG, NET, UPD)>);

impl<RNG: RngCore, NET: Network, UPD: Updater> Backend for BackendAdapter<RNG, NET, UPD> {
    type Rng<'a> = RNG;
    type Network<'a> = NET;
    type Updater<'a> = UPD;
}

/// Firmware backend resources
#[must_use]
struct BackendResources<'be, BE: Backend> {
    rng: BE::Rng<'be>,
    rtc: time::Rtc,
    network: &'be BE::Network<'be>,
    articles: article::Articles,
    users: user::Users,
    schedule: schedule::Daily,
    http: reqwless::client::HttpClient<
        'be,
        <BE::Network<'be> as Network>::TcpClient,
        <BE::Network<'be> as Network>::DnsSocket,
    >,
    vereinsflieger: vereinsflieger::Vereinsflieger<'be>,
    telemetry: telemetry::Telemetry<'be>,
    updater: Option<BE::Updater<'be>>,
}

/// Run firmware
#[expect(clippy::too_many_arguments)]
pub async fn run<
    DP: Display,
    KP: Keypad,
    NFC: NfcReader,
    BZZ: Buzzer,
    RNG: RngCore,
    NET: Network,
    UPD: Updater,
>(
    config: config::Config,
    device_id: &str,
    display: DP,
    keypad: KP,
    nfc: NFC,
    buzzer: BZZ,
    mut rng: RNG,
    network: NET,
    updater: Option<UPD>,
) -> ! {
    // Initialize real time clock
    let rtc = time::Rtc::new();

    // Initialize article and user look up tables
    let articles = article::Articles::new(config.vf_article_ids);
    let users = user::Users::new();

    // Initialize scheduler
    let schedule = schedule::Daily::new();

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
    let http = HttpClient::new_with_tls(network.tcp(), network.dns(), tls_config);

    // Initialize Vereinsflieger API client
    let vereinsflieger = vereinsflieger::Vereinsflieger::new(
        &config.vf_username,
        &config.vf_password_md5,
        &config.vf_appkey,
        config.vf_cid,
    );

    // Initialize telemetry
    let telemetry = telemetry::Telemetry::new(config.mp_token.as_deref(), device_id);

    // Initialize frontend resources
    let mut frontend = FrontendResources::<FrontendAdapter<DP, KP, NFC, BZZ>> {
        display,
        keypad,
        nfc,
        buzzer,
    };

    // Initialize backend resources
    let mut backend = BackendResources::<BackendAdapter<RNG, NET, UPD>> {
        rng,
        rtc,
        network: &network,
        articles,
        users,
        schedule,
        http,
        vereinsflieger,
        telemetry,
        updater,
    };

    // Run user interface
    ui::run(&mut frontend, &mut backend).await
}
