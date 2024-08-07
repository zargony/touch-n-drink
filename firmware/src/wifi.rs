use core::time::Duration;
use esp_hal::clock::Clocks;
use esp_hal::peripherals;
use esp_hal::rng::Rng;
use esp_hal::timer::{ErasedTimer, PeriodicTimer};
use esp_wifi::wifi::{self, ScanConfig, ScanTypeConfig, WifiController, WifiDevice, WifiStaDevice};
use esp_wifi::{EspWifiInitFor, EspWifiInitialization};
use log::{debug, info};

/// Wifi error
pub use esp_wifi::wifi::WifiError;

/// Wifi interface
pub struct Wifi<'d> {
    #[allow(dead_code)]
    inited: EspWifiInitialization,
    #[allow(dead_code)]
    device: WifiDevice<'d, WifiStaDevice>,
    controller: WifiController<'d>,
}

impl<'d> Wifi<'d> {
    /// Create and initialize Wifi interface
    pub async fn new(
        timer: PeriodicTimer<ErasedTimer>,
        rng: Rng,
        radio_clocks: peripherals::RADIO_CLK,
        clocks: &Clocks<'d>,
        wifi: peripherals::WIFI,
    ) -> Result<Self, WifiError> {
        let inited = esp_wifi::initialize(EspWifiInitFor::Wifi, timer, rng, radio_clocks, clocks)
            .map_err(|_| WifiError::NotInitialized)?;

        let (device, mut controller) = wifi::new_with_mode(&inited, wifi, WifiStaDevice)?;
        debug!("Wifi configuration: {:?}", controller.get_configuration());
        debug!("Wifi capabilities: {:?}", controller.get_capabilities());
        debug!("Wifi state: {:?}", wifi::get_wifi_state());

        info!("Starting Wifi controller...");
        controller.start().await?;
        debug!("Wifi state: {:?}", wifi::get_wifi_state());

        Ok(Wifi {
            inited,
            device,
            controller,
        })
    }

    /// Test Wifi interface
    pub async fn test(&mut self) {
        let scan_config = ScanConfig {
            ssid: None,
            bssid: None,
            channel: None,
            show_hidden: true,
            scan_type: ScanTypeConfig::Passive(Duration::from_millis(1000)),
        };
        info!("Starting Wifi scan...");
        let (aps, count) = self
            .controller
            .scan_with_config::<10>(scan_config)
            .await
            .unwrap();
        info!("Wifi scan done.");
        info!("Wifi scan returned {} results: {:?}", count, aps);
        debug!("Wifi state: {:?}", wifi::get_wifi_state());
    }
}
