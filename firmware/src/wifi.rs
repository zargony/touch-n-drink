use core::fmt;
use embassy_executor::{task, Spawner};
use embassy_net::dns::{self, DnsQueryType, DnsSocket};
use embassy_net::tcp::client::{TcpClient, TcpClientState};
use embassy_net::{Config, DhcpConfig, IpAddress, Stack, StackResources, StaticConfigV4};
use embassy_time::{Duration, Timer};
use embedded_nal_async::{Dns, TcpConnect};
use esp_hal::clock::Clocks;
use esp_hal::peripherals;
use esp_hal::rng::Rng;
use esp_wifi::wifi::{
    self, ClientConfiguration as WifiClientConfiguration, Configuration as WifiConfiguration,
    WifiController, WifiDevice, WifiEvent, WifiStaDevice, WifiState,
};
use esp_wifi::{EspWifiInitFor, EspWifiTimerSource};
use log::{debug, info, warn};
use rand_core::RngCore;
use static_cell::StaticCell;

/// Delay after Wifi disconnect or connection failure before trying to reconnect
const CONNECT_RETRY_DELAY: Duration = Duration::from_millis(5000);

/// Number of TCP sockets
const NUM_TCP_SOCKETS: usize = 4;

/// Size of receive buffer (per TCP socket)
const RX_BUFFER_SIZE: usize = 2048;

/// Size of transmit buffer (per TCP socket)
const TX_BUFFER_SIZE: usize = 1024;

/// Wifi initialization error
pub use esp_wifi::InitializationError;

/// Static network stack resources (sockets, inflight dns queries)
// Needs at least one socket for DHCP, one socket for DNS, plus additional sockets for connections
static RESOURCES: StaticCell<StackResources<{ 2 + NUM_TCP_SOCKETS }>> = StaticCell::new();

/// Static network stack
static STACK: StaticCell<Stack<WifiDevice<'_, WifiStaDevice>>> = StaticCell::new();

/// Option display helper
struct DisplayOption<T: fmt::Display>(Option<T>);

impl<T: fmt::Display> fmt::Display for DisplayOption<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            None => write!(f, "-"),
            Some(value) => value.fmt(f),
        }
    }
}

/// List display helper
struct DisplayList<'a, T: fmt::Display>(&'a [T]);

impl<'a, T: fmt::Display> fmt::Display for DisplayList<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            write!(f, "-")?;
        } else {
            for (i, elem) in self.0.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                elem.fmt(f)?;
            }
        }
        Ok(())
    }
}

/// Wifi configuration display helper
struct DisplayWifiConfig(WifiConfiguration);

impl fmt::Display for DisplayWifiConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            WifiConfiguration::None => write!(f, "None"),
            WifiConfiguration::Client(client) => write!(f,
                "Client, auth: {:?}, ssid: {}, channel: {}",
                client.auth_method,
                client.ssid,
                DisplayOption(client.channel),
            ),
            WifiConfiguration::AccessPoint(ap) => write!(f,
                "AP, auth: {:?}, ssid: {}, channel: {}",
                ap.auth_method,
                ap.ssid,
                ap.channel,
            ),
            WifiConfiguration::Mixed(client, ap) => write!(f,
                "Client+AP, auth: {:?}, ssid: {}, channel: {}, AP auth: {:?}, ssid: {}, channel: {}",
                client.auth_method,
                client.ssid,
                DisplayOption(client.channel),
                ap.auth_method,
                ap.ssid,
                ap.channel,
            ),
            WifiConfiguration::EapClient(eap) => write!(f,
                "EAP Client, auth: {:?}, ssid: {}, channel: {}",
                eap.auth_method,
                eap.ssid,
                DisplayOption(eap.channel)
            ),
        }
    }
}

/// Network configuration display helper
struct DisplayNetworkConfig(StaticConfigV4);

impl fmt::Display for DisplayNetworkConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ip: {}, gw: {}, dns: {}",
            self.0.address,
            DisplayOption(self.0.gateway),
            DisplayList(&self.0.dns_servers),
        )
    }
}

/// Wifi interface
pub struct Wifi {
    stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>,
    tcp_client_state: TcpClientState<NUM_TCP_SOCKETS, TX_BUFFER_SIZE, RX_BUFFER_SIZE>,
}

impl Wifi {
    /// Create and initialize Wifi interface
    pub async fn new(
        timer: impl EspWifiTimerSource,
        mut rng: Rng,
        radio_clocks: peripherals::RADIO_CLK,
        clocks: &Clocks<'_>,
        wifi: peripherals::WIFI,
        spawner: Spawner,
    ) -> Result<Self, InitializationError> {
        debug!("Wifi: Initializing controller...");

        // Generate random seed
        let random_seed = rng.next_u64();

        // Initialize and start ESP32 Wifi controller
        debug!("Wifi: Static configuration: {:?}", esp_wifi::CONFIG);
        let init = esp_wifi::initialize(EspWifiInitFor::Wifi, timer, rng, radio_clocks, clocks)?;
        let client_config = WifiClientConfiguration {
            ..Default::default()
        };
        let (device, mut controller) = wifi::new_with_config(&init, wifi, client_config)?;
        debug!("Wifi: Starting controller...");
        controller.start().await?;

        // Spawn task for handling Wifi connection events
        debug!("Wifi: Spawning connection task...");
        match spawner.spawn(connection(controller)) {
            Ok(()) => (),
            // Panic on failure since failing to spawn a task indicates a serious error
            Err(err) => panic!("Failed to spawn Wifi connection task: {:?}", err),
        }

        // Initialize static network stack
        let config = Config::dhcpv4(DhcpConfig::default());
        let resources = RESOURCES.init(StackResources::new());
        let stack = STACK.init(Stack::new(device, config, resources, random_seed));

        // Spawn task for running network stack
        debug!("Wifi: Spawning network task...");
        match spawner.spawn(network(stack)) {
            Ok(()) => (),
            // Panic on failure since failing to spawn a task indicates a serious error
            Err(err) => panic!("Failed to spawn Wifi network task: {:?}", err),
        }

        // Initialize TCP client state (reserves sockets and rx/tx buffers)
        let tcp_client_state = TcpClientState::new();

        info!("Wifi: Controller initialized");
        Ok(Self {
            stack,
            tcp_client_state,
        })
    }

    /// Returns whether network stack is up (Wifi connected and IP address obtained)
    pub fn is_up(&self) -> bool {
        self.stack.is_link_up() && self.stack.is_config_up()
    }

    /// Wait for network stack to come up (Wifi connected and IP address obtained). This function
    /// can potentially take forever, e.g. if Wifi credentials are wrong or DHCP doesn't work.
    pub async fn wait_up(&self) {
        if self.is_up() {
            return;
        }

        debug!("Wifi: Waiting for network to come up...");
        self.stack.wait_config_up().await;
        debug_assert!(self.stack.is_link_up() && self.stack.is_config_up());

        match self.stack.config_v4() {
            Some(network_config) => {
                info!(
                    "Wifi: Network configured, hw: {}, {}",
                    self.stack.hardware_address(),
                    DisplayNetworkConfig(network_config),
                );
            }
            // Panic on failure since no IPv4 indicates a serious error
            None => panic!("Failed to retrieve IPv4 network configuration"),
        }
    }

    /// Query DNS for IP address of given name
    #[allow(dead_code)]
    pub async fn dns_query(&self, name: &str) -> Result<IpAddress, dns::Error> {
        match self.stack.dns_query(name, DnsQueryType::A).await {
            Ok(addrs) if addrs.is_empty() => {
                warn!("Wifi: DNS query {} returned empty result", name);
                Err(dns::Error::Failed)
            }
            Ok(addrs) => {
                debug!("Wifi: DNS query {}: {}", name, DisplayList(&addrs));
                Ok(addrs[0])
            }
            Err(err) => {
                warn!("Wifi: DNS query {} error: {:?}", name, err);
                Err(err)
            }
        }
    }

    /// Provide an embedded-nal-async compatible DNS socket
    #[allow(dead_code)]
    pub fn dns(&self) -> impl Dns + '_ {
        DnsSocket::new(self.stack)
    }

    /// Provide an embedded-nal-async compatible TCP client
    #[allow(dead_code)]
    pub fn tcp(&self) -> impl TcpConnect + '_ {
        TcpClient::new(self.stack, &self.tcp_client_state)
    }
}

/// Task for handling Wifi connection events
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
                let wifi_config = controller
                    .get_configuration()
                    .unwrap_or(WifiConfiguration::None);
                info!("Wifi: {} connecting...", DisplayWifiConfig(wifi_config));
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

/// Task for running network stack
#[task]
async fn network(stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>) -> ! {
    debug!("Wifi: Start network task");

    stack.run().await;
    unreachable!()
}
