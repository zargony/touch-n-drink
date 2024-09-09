use embassy_executor::{task, Spawner};
use embassy_time::{Duration, Timer};
use esp_hal::clock::Clocks;
use esp_hal::peripherals;
use esp_hal::rng::Rng;
use esp_wifi::wifi::{self, WifiController, WifiDevice, WifiEvent, WifiStaDevice, WifiState};
use esp_wifi::{EspWifiInitFor, EspWifiInitialization, EspWifiTimerSource};
use log::{debug, info, warn};

/// Delay after disconnect or connection failure before trying to reconnect
const CONNECT_RETRY_DELAY: Duration = Duration::from_millis(5000);

/// Wifi initialization error
pub use esp_wifi::InitializationError;

// /// Wifi error
// pub use esp_wifi::wifi::WifiError as Error;

/// Wifi interface
pub struct Wifi<'d> {
    _init: EspWifiInitialization,
    _device: WifiDevice<'d, WifiStaDevice>,
}

impl<'d> Wifi<'d> {
    /// Create and initialize Wifi interface
    pub async fn new(
        timer: impl EspWifiTimerSource,
        rng: Rng,
        radio_clocks: peripherals::RADIO_CLK,
        clocks: &Clocks<'d>,
        wifi: peripherals::WIFI,
        spawner: Spawner,
    ) -> Result<Self, InitializationError> {
        debug!("Wifi: Initializing controller...");

        debug!("Wifi: Static configuration: {:?}", esp_wifi::CONFIG);
        let init = esp_wifi::initialize(EspWifiInitFor::Wifi, timer, rng, radio_clocks, clocks)?;
        let client_config = Default::default();
        let (device, mut controller) = wifi::new_with_config(&init, wifi, client_config)?;

        debug!("Wifi: Starting controller...");
        controller.start().await?;

        debug!("Wifi: Starting connection task...");
        match spawner.spawn(connection(controller)) {
            Ok(()) => (),
            // Panic on failure since failing to spawn a task indicates a serious error
            Err(err) => panic!("Failed to spawn task: {:?}", err),
        }

        info!("Wifi: Controller initialized");
        Ok(Self {
            _init: init,
            _device: device,
        })
    }
}

#[task]
async fn connection(mut controller: WifiController<'static>) -> ! {
    debug!("Wifi: Start connection task");

    loop {
        match wifi::get_wifi_state() {
            // If connected, wait for disconnect
            WifiState::StaConnected => {
                controller.wait_for_event(WifiEvent::StaDisconnected).await;
                warn!("Wifi: Disconnected");
                Timer::after(CONNECT_RETRY_DELAY).await;
            }
            // If disconnected, try to connect
            WifiState::StaStarted | WifiState::StaDisconnected => {
                info!("Wifi: Connecting...");
                debug!("Wifi: Configuration: {:?}", controller.get_configuration());
                match controller.connect().await {
                    Ok(()) => info!("Wifi: Connected"),
                    Err(err) => {
                        warn!("Wifi: Failed to connect: {:?}", err);
                        Timer::after(CONNECT_RETRY_DELAY).await;
                    }
                }
            }
            // Any other state is unexpected and triggers a panic
            state => panic!("Unexpected Wifi state {:?}", state),
        }
    }
}
