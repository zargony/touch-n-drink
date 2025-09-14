use alloc::boxed::Box;
use alloc::string::ToString;
use core::cell::Cell;
use core::fmt;
use embassy_executor::{task, Spawner};
use embassy_net::dns::{self, DnsQueryType};
use embassy_net::tcp::{self, client::TcpClientState};
use embassy_net::{Config, DhcpConfig, IpAddress, Runner, Stack, StackResources, StaticConfigV4};
use embassy_time::{Duration, Timer};
use esp_hal::peripherals;
use esp_hal::rng::Rng;
use esp_wifi::wifi::{
    self, AuthMethod, ClientConfiguration as WifiClientConfiguration,
    Configuration as WifiConfiguration, WifiController, WifiDevice, WifiEvent, WifiState,
};
use esp_wifi::EspWifiTimerSource;
use log::{debug, info, warn};
use rand_core::RngCore;

/// Delay after Wifi disconnect or connection failure before trying to reconnect
const CONNECT_RETRY_DELAY: Duration = Duration::from_millis(5000);

/// Number of TCP sockets
const NUM_TCP_SOCKETS: usize = 4;

/// Size of transmit buffer (per TCP socket)
const TX_BUFFER_SIZE: usize = 2048;

/// Size of receive buffer (per TCP socket)
const RX_BUFFER_SIZE: usize = 4096;

/// Type of DNS socket
pub type DnsSocket<'d> = dns::DnsSocket<'d>;

/// Type of TCP client
pub type TcpClient<'d> =
    tcp::client::TcpClient<'d, NUM_TCP_SOCKETS, TX_BUFFER_SIZE, RX_BUFFER_SIZE>;

/// Type of TCP connection returned by TCP client
pub type TcpConnection<'d> =
    tcp::client::TcpConnection<'d, NUM_TCP_SOCKETS, TX_BUFFER_SIZE, RX_BUFFER_SIZE>;

/// Wifi initialization error
pub use esp_wifi::InitializationError;

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

impl<T: fmt::Display> fmt::Display for DisplayList<'_, T> {
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
    stack: Stack<'static>,
    dns_socket: DnsSocket<'static>,
    tcp_client: TcpClient<'static>,
    last_up_state: Cell<bool>,
}

impl Wifi {
    /// Create and initialize Wifi interface
    pub fn new(
        timer: impl EspWifiTimerSource + 'static,
        mut rng: Rng,
        wifi: peripherals::WIFI<'static>,
        spawner: Spawner,
        ssid: &str,
        password: &str,
    ) -> Result<Self, InitializationError> {
        debug!("Wifi: Initializing controller...");

        // Several resources below are allocated and leaked to get a `&'static mut` reference.
        // This is ok, since only one instance of `Wifi` can exist and it'll never be dropped.

        // Initialize and start ESP32 Wifi controller
        let esp_wifi_ctrl = Box::new(esp_wifi::init(timer, rng)?);
        let esp_wifi_ctrl = Box::leak(esp_wifi_ctrl);
        let (mut controller, interfaces) = esp_wifi::wifi::new(esp_wifi_ctrl, wifi)?;
        let client_config = WifiClientConfiguration {
            ssid: ssid.to_string(),
            auth_method: if password.is_empty() {
                AuthMethod::None
            } else {
                AuthMethod::WPA2Personal
            },
            password: password.to_string(),
            ..Default::default()
        };
        let wifi_config = WifiConfiguration::Client(client_config);
        controller.set_configuration(&wifi_config)?;
        let wifi_interface = interfaces.sta;

        // Spawn task for handling Wifi connection events
        debug!("Wifi: Spawning connection task...");
        spawner
            .spawn(connection(controller))
            // Panic on failure since failing to spawn a task indicates a serious error
            .expect("Failed to spawn Wifi connection task");

        // Initialize network stack resources (sockets, inflight dns queries). Needs at least one
        // socket for DHCP, one socket for DNS, plus additional sockets for connections.
        let resources = Box::new(StackResources::<{ 2 + NUM_TCP_SOCKETS }>::new());
        let resources = Box::leak(resources);

        // Initialize network stack
        let net_config = Config::dhcpv4(DhcpConfig::default());
        let random_seed = rng.next_u64();
        let (stack, runner) = embassy_net::new(wifi_interface, net_config, resources, random_seed);

        // Spawn task for running network stack
        debug!("Wifi: Spawning network task...");
        spawner
            .spawn(network(runner))
            // Panic on failure since failing to spawn a task indicates a serious error
            .expect("Failed to spawn Wifi network task");

        // Initialize TCP client state (contains tx/rx buffers for TCP sockets)
        let tcp_client_state = Box::new(TcpClientState::new());
        let tcp_client_state = Box::leak(tcp_client_state);

        // Initialize embedded-nal-async compatible DNS socket and TCP client
        let dns_socket = DnsSocket::new(stack);
        let tcp_client = TcpClient::new(stack, tcp_client_state);

        info!(
            "Wifi: Controller initialized. Hw: {}, {}",
            stack.hardware_address(),
            DisplayWifiConfig(wifi_config),
        );
        Ok(Self {
            stack,
            dns_socket,
            tcp_client,
            last_up_state: Cell::new(false),
        })
    }

    /// Returns whether network stack is up (Wifi connected and IP address obtained)
    pub fn is_up(&self) -> bool {
        let up = self.stack.is_link_up() && self.stack.is_config_up();

        // Log network state only if changed since last call
        if up != self.last_up_state.get() {
            if up {
                if let Some(network_config) = self.stack.config_v4() {
                    info!(
                        "Wifi: Network configured. {}",
                        DisplayNetworkConfig(network_config),
                    );
                }
            } else {
                info!("Wifi: Network down");
            }

            self.last_up_state.set(up);
        }

        up
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
        self.is_up();
    }

    /// Query DNS for IP address of given name
    #[allow(dead_code)]
    pub async fn dns_query(&self, name: &str) -> Result<IpAddress, dns::Error> {
        match self.stack.dns_query(name, DnsQueryType::A).await {
            Ok(addrs) if addrs.is_empty() => {
                warn!("Wifi: DNS query {name} returned empty result");
                Err(dns::Error::Failed)
            }
            Ok(addrs) => {
                debug!("Wifi: DNS query {}: {}", name, DisplayList(&addrs));
                Ok(addrs[0])
            }
            Err(err) => {
                warn!("Wifi: DNS query {name} error: {err:?}");
                Err(err)
            }
        }
    }

    /// Provide an embedded-nal-async compatible DNS socket
    pub fn dns(&self) -> &'_ DnsSocket<'_> {
        &self.dns_socket
    }

    /// Provide an embedded-nal-async compatible TCP client
    pub fn tcp(&self) -> &'_ TcpClient<'_> {
        &self.tcp_client
    }
}

/// Task for handling Wifi connection events
#[task]
async fn connection(mut controller: WifiController<'static>) -> ! {
    debug!("Wifi: Start connection task");

    loop {
        // If connected, wait for disconnect
        if wifi::wifi_state() == WifiState::StaConnected {
            controller.wait_for_event(WifiEvent::StaDisconnected).await;
            warn!("Wifi: Disconnected");
            Timer::after(CONNECT_RETRY_DELAY).await;
        }

        // If needed, start controller
        if !matches!(controller.is_started(), Ok(true)) {
            debug!("Wifi: Starting controller...");
            controller.start_async().await.unwrap();
        }

        // Try to connect
        info!("Wifi: Connecting...");
        match controller.connect_async().await {
            Ok(()) => info!("Wifi: Connected"),
            Err(err) => {
                warn!(
                    "Wifi: Failed to connect: {:?}, state {:?}",
                    err,
                    wifi::wifi_state()
                );
                Timer::after(CONNECT_RETRY_DELAY).await;
            }
        }
    }
}

/// Task for running network stack
#[task]
async fn network(mut runner: Runner<'static, WifiDevice<'static>>) {
    debug!("Wifi: Start network task");

    runner.run().await;
}
