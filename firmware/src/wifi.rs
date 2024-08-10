use esp_hal::clock::Clocks;
use esp_hal::peripherals;
use esp_hal::rng::Rng;
use esp_hal::timer::{ErasedTimer, PeriodicTimer};
use esp_wifi::wifi::{self, WifiController, WifiDevice, WifiStaDevice};
use esp_wifi::{EspWifiInitFor, EspWifiInitialization};
use log::{debug, info};

/// Wifi initialization error
pub use esp_wifi::InitializationError;

// /// Wifi error
// pub use esp_wifi::wifi::WifiError as Error;

/// Wifi interface
pub struct Wifi<'d> {
    _init: EspWifiInitialization,
    _device: WifiDevice<'d, WifiStaDevice>,
    _controller: WifiController<'d>,
}

impl<'d> Wifi<'d> {
    /// Create and initialize Wifi interface
    pub async fn new(
        timer: PeriodicTimer<ErasedTimer>,
        rng: Rng,
        radio_clocks: peripherals::RADIO_CLK,
        clocks: &Clocks<'d>,
        wifi: peripherals::WIFI,
    ) -> Result<Self, InitializationError> {
        let init = esp_wifi::initialize(EspWifiInitFor::Wifi, timer, rng, radio_clocks, clocks)?;

        let (device, mut controller) = wifi::new_with_mode(&init, wifi, WifiStaDevice)?;
        debug!("Static Wifi configuration: {:?}", esp_wifi::CONFIG);
        debug!("Wifi configuration: {:?}", controller.get_configuration());
        debug!("Wifi capabilities: {:?}", controller.get_capabilities());
        debug!("Wifi state: {:?}", wifi::get_wifi_state());

        info!("Starting Wifi controller...");
        controller.start().await?;
        debug!("Wifi state: {:?}", wifi::get_wifi_state());

        Ok(Wifi {
            _init: init,
            _device: device,
            _controller: controller,
        })
    }
}
