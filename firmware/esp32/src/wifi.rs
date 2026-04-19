use alloc::boxed::Box;
use alloc::string::ToString;
use common::util::{DisplayOption, DisplaySlice};
use core::cell::Cell;
use embassy_executor::Spawner;
use embassy_net::dns::{self, DnsQueryType};
use embassy_net::tcp::{self, client::TcpClientState};
use embassy_net::{Config as NetConfig, DhcpConfig, IpAddress, Runner, Stack, StackResources};
use embassy_time::{Duration, Timer};
use esp_hal::peripherals;
use esp_hal::rng::Rng;
use esp_radio::wifi::sta::StationConfig;
use esp_radio::wifi::{
    AuthenticationMethod, Config as WifiConfig, ControllerConfig, CountryInfo, Interface,
    WifiController, WifiError,
};
use log::{debug, info, warn};
use rand_core::Rng as _;

/// Delay after Wifi disconnect or connection failure before trying to reconnect
const CONNECT_RETRY_DELAY: Duration = Duration::from_millis(5000);

/// Number of TCP sockets
const NUM_TCP_SOCKETS: usize = 4;

/// Type of DNS socket
pub type DnsSocket<'d> = dns::DnsSocket<'d>;

/// Type of TCP client
pub type TcpClient<'d> = tcp::client::TcpClient<'d, NUM_TCP_SOCKETS>;

/// Type of TCP connection returned by TCP client
#[expect(dead_code)]
pub type TcpConnection<'d> = tcp::client::TcpConnection<'d, NUM_TCP_SOCKETS, 1024, 1024>;

/// Wifi interface
pub struct Wifi {
    stack: Stack<'static>,
    dns_socket: DnsSocket<'static>,
    tcp_client: TcpClient<'static>,
    last_up_state: Cell<bool>,
}

impl common::Network for Wifi {
    type DnsSocket = DnsSocket<'static>;
    type TcpClient = TcpClient<'static>;

    fn is_up(&self) -> bool {
        (*self).is_up()
    }

    async fn wait_up(&self) {
        (*self).wait_up().await;
    }

    fn dns(&self) -> &'_ Self::DnsSocket {
        &self.dns_socket
    }

    fn tcp(&self) -> &'_ Self::TcpClient {
        &self.tcp_client
    }
}

impl Wifi {
    /// Create and initialize Wifi interface
    pub fn new(
        mut rng: Rng,
        wifi: peripherals::WIFI<'static>,
        spawner: Spawner,
        country: Option<[u8; 2]>,
        ssid: &str,
        password: &str,
    ) -> Result<Self, WifiError> {
        debug!("Wifi: Initializing controller...");

        // Several resources below are allocated and leaked to get a `&'static mut` reference.
        // This is ok, since only one instance of `Wifi` can exist and it'll never be dropped.

        // Initialize and start ESP32 Wifi controller
        let country_info = CountryInfo::from(country.unwrap_or(*b"01"));
        let station_config = StationConfig::default()
            .with_ssid(ssid)
            .with_auth_method(if password.is_empty() {
                AuthenticationMethod::None
            } else {
                AuthenticationMethod::Wpa2Personal
            })
            .with_password(password.to_string());
        let wifi_config = WifiConfig::Station(station_config.clone());
        let controller_config = ControllerConfig::default()
            .with_country_info(country_info)
            .with_initial_config(wifi_config);
        let (controller, interfaces) = esp_radio::wifi::new(wifi, controller_config)?;
        let wifi_interface = interfaces.station;

        // Spawn task for handling Wifi connection events
        debug!("Wifi: Spawning connection task...");
        // Panic on failure since failing to spawn a task indicates a serious error
        spawner.spawn(connection(controller).expect("Failed to spawn Wifi connection task"));

        // Initialize network stack resources (sockets, inflight dns queries). Needs at least one
        // socket for DHCP, one socket for DNS, plus additional sockets for connections.
        let resources = Box::new(StackResources::<{ 2 + NUM_TCP_SOCKETS }>::new());
        let resources = Box::leak(resources);

        // Initialize network stack
        let net_config = NetConfig::dhcpv4(DhcpConfig::default());
        let seed = rng.next_u64();
        let (stack, runner) = embassy_net::new(wifi_interface, net_config, resources, seed);

        // Spawn task for running network stack
        debug!("Wifi: Spawning network task...");
        // Panic on failure since failing to spawn a task indicates a serious error
        spawner.spawn(network(runner).expect("Failed to spawn Wifi network task"));

        // Initialize TCP client state (contains tx/rx buffers for TCP sockets)
        let tcp_client_state = Box::new(TcpClientState::new());
        let tcp_client_state = Box::leak(tcp_client_state);

        // Initialize embedded-nal-async compatible DNS socket and TCP client
        let dns_socket = DnsSocket::new(stack);
        let tcp_client = TcpClient::new(stack, tcp_client_state);

        info!(
            "Wifi: Controller initialized, station mode, ssid: {}, auth: {:?}, channel: {}, hw: {}",
            station_config.ssid().as_str(),
            station_config.auth_method(),
            DisplayOption(station_config.channel()),
            stack.hardware_address(),
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
                        "Wifi: Network configured, ip: {}, gw: {}, dns: {}",
                        network_config.address,
                        DisplayOption(network_config.gateway),
                        DisplaySlice(&network_config.dns_servers),
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
    #[expect(dead_code)]
    pub async fn dns_query(&self, name: &str) -> Result<IpAddress, dns::Error> {
        match self.stack.dns_query(name, DnsQueryType::A).await {
            Ok(addrs) if addrs.is_empty() => {
                warn!("Wifi: DNS query {name} returned empty result");
                Err(dns::Error::Failed)
            }
            Ok(addrs) => {
                debug!("Wifi: DNS query {}: {}", name, DisplaySlice(&addrs));
                Ok(addrs[0])
            }
            Err(err) => {
                warn!("Wifi: DNS query {name} error: {err:?}");
                Err(err)
            }
        }
    }
}

/// Task for handling Wifi connection events
#[embassy_executor::task]
async fn connection(mut controller: WifiController<'static>) -> ! {
    debug!("Wifi: Start connection task");

    loop {
        // Try to connect
        info!("Wifi: Connecting...");
        match controller.connect_async().await {
            Ok(info) => {
                info!(
                    "Wifi: Connected to ssid: {}, bssid: {}, channel: {}, auth: {:?}",
                    info.ssid.as_str(),
                    const_hex::const_encode::<6, false>(&info.bssid).as_str(),
                    info.channel,
                    info.authmode,
                );

                // Wait for disconnect
                let info = controller.wait_for_disconnect_async().await.ok();
                if let Some(info) = info {
                    warn!(
                        "Wifi: Disconnected from ssid: {}, reason: {:?}",
                        info.ssid.as_str(),
                        info.reason,
                    );
                } else {
                    warn!("Wifi: Disconnected (unknown reason)");
                }
            }
            Err(err) => warn!("Wifi: Failed to connect: {err:?}"),
        }

        // Delay before retrying to connect
        Timer::after(CONNECT_RETRY_DELAY).await;
    }
}

/// Task for running network stack
#[embassy_executor::task]
async fn network(mut runner: Runner<'static, Interface<'static>>) {
    debug!("Wifi: Start network task");

    runner.run().await;
}
