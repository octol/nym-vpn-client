// Copyright 2023 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: GPL-3.0-only

uniffi::setup_scaffolding!();

#[cfg(not(target_os = "ios"))]
use crate::config::WireguardConfig;
use crate::error::{Error, Result};
use crate::mixnet_connect::setup_mixnet_client;
#[cfg(not(target_os = "ios"))]
use crate::tunnel::setup_route_manager;
#[cfg(target_os = "ios")]
use crate::util::wait_for_interrupt;
#[cfg(not(target_os = "ios"))]
use crate::wg_gateway_client::WgGatewayClient;
use error::GatewayDirectoryError;
use futures::channel::{mpsc, oneshot};
use futures::SinkExt;
use log::{debug, error, info};
use mixnet_connect::SharedMixnetClient;
use nym_connection_monitor::ConnectionMonitorTask;
use nym_gateway_directory::{
    Config as GatewayDirectoryConfig, EntryPoint, ExitPoint, GatewayClient, IpPacketRouterAddress,
};
use nym_ip_packet_client::IprClientConnect;
use nym_task::TaskManager;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;
use std::sync::Arc;
#[cfg(not(target_os = "ios"))]
use std::sync::Mutex;
#[cfg(not(target_os = "ios"))]
use talpid_core::dns::DnsMonitor;
#[cfg(not(target_os = "ios"))]
use talpid_routing::RouteManager;
#[cfg(not(target_os = "ios"))]
use tunnel_setup::init_firewall_dns;
#[cfg(not(target_os = "ios"))]
use tunnel_setup::{setup_tunnel, AllTunnelsSetup, TunnelSetup};
#[cfg(not(target_os = "ios"))]
use util::wait_and_handle_interrupt;
use util::wait_for_interrupt_and_signal;

// Public re-export
pub use nym_connection_monitor as connection_monitor;
pub use nym_credential_storage_pre_ecash as credential_storage_pre_ecash;
pub use nym_gateway_directory as gateway_directory;
pub use nym_id_pre_ecash as id_pre_ecash;

pub use nym_ip_packet_requests::IpPair;
pub use nym_sdk::mixnet::{NodeIdentity, Recipient, StoragePaths};
pub use nym_sdk::UserAgent;
pub use nym_task::{
    manager::{SentStatus, TaskStatus},
    StatusReceiver,
};

#[cfg(target_os = "ios")]
use crate::ios::OSTunProvider;
#[cfg(any(target_os = "ios", target_os = "macos"))]
pub use crate::platform::swift;
use crate::platform::uniffi_set_listener_status;
use crate::uniffi_custom_impls::{ExitStatus, StatusEvent};
pub use nym_bin_common;
pub use nym_config;
#[cfg(not(target_os = "ios"))]
use talpid_tunnel::tun_provider::TunProvider;
use tokio::task::JoinHandle;
use tun2::AsyncDevice;

mod bandwidth_controller;
mod platform;
mod tunnel_setup;
mod uniffi_custom_impls;

pub mod config;
pub mod credentials;
pub mod error;
#[cfg(any(target_os = "ios", target_os = "android"))]
mod mobile;
pub mod keys;
pub mod mixnet_connect;
pub mod mixnet_processor;
pub mod routing;
pub mod storage;
pub mod tunnel;
pub mod util;
pub mod wg_gateway_client;
mod wireguard_setup;

const MIXNET_CLIENT_STARTUP_TIMEOUT_SECS: u64 = 30;
pub const SHUTDOWN_TIMER_SECS: u64 = 10;

pub static DEFAULT_DNS_SERVERS: [IpAddr; 4] = [
    IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
    IpAddr::V4(Ipv4Addr::new(1, 0, 0, 1)),
    IpAddr::V6(Ipv6Addr::new(0x2606, 0x4700, 0x4700, 0, 0, 0, 0, 0x1111)),
    IpAddr::V6(Ipv6Addr::new(0x2606, 0x4700, 0x4700, 0, 0, 0, 0, 0x1001)),
];

#[cfg(not(target_os = "ios"))]
async fn init_wireguard_config(
    gateway_client: &GatewayClient,
    wg_gateway_client: &mut WgGatewayClient,
    wg_gateway: Option<IpAddr>,
    mtu: u16,
) -> Result<(WireguardConfig, IpAddr)> {
    // First we need to register with the gateway to setup keys and IP assignment
    info!("Registering with wireguard gateway");
    let gateway_auth_recipient = wg_gateway_client
        .auth_recipient()
        .gateway()
        .to_base58_string();
    let gateway_host = gateway_client
        .lookup_gateway_ip(&gateway_auth_recipient)
        .await
        .map_err(|source| GatewayDirectoryError::FailedToLookupGatewayIp {
            gateway_id: gateway_auth_recipient,
            source,
        })?;
    let wg_gateway_data = wg_gateway_client.register_wireguard(gateway_host).await?;
    debug!("Received wireguard gateway data: {wg_gateway_data:?}");

    let wireguard_config = WireguardConfig::init(
        wg_gateway_client.keypair(),
        &wg_gateway_data,
        wg_gateway,
        mtu,
    )?;
    Ok((wireguard_config, gateway_host))
}

#[derive(Default)]
struct ShadowHandle {
    _inner: Option<JoinHandle<Result<AsyncDevice>>>,
}

pub struct MixnetVpn {}

pub struct WireguardVpn {}

pub trait Vpn {}

impl Vpn for MixnetVpn {}
impl Vpn for WireguardVpn {}

pub enum SpecificVpn {
    Wg(NymVpn<WireguardVpn>),
    Mix(NymVpn<MixnetVpn>),
}

impl From<NymVpn<WireguardVpn>> for SpecificVpn {
    fn from(value: NymVpn<WireguardVpn>) -> Self {
        Self::Wg(value)
    }
}

impl From<NymVpn<MixnetVpn>> for SpecificVpn {
    fn from(value: NymVpn<MixnetVpn>) -> Self {
        Self::Mix(value)
    }
}

#[derive(Clone, Debug)]
pub struct MixnetClientConfig {
    /// Enable Poission process rate limiting of outbound traffic.
    pub enable_poisson_rate: bool,

    /// Disable constant rate background loop cover traffic
    pub disable_background_cover_traffic: bool,

    /// Enable the credentials mode between the client and the entry gateway.
    pub enable_credentials_mode: bool,

    /// The minimum performance of mixnodes to use.
    pub min_mixnode_performance: Option<u8>,

    /// The minimum performance of gateways to use.
    pub min_gateway_performance: Option<u8>,
}

pub struct GenericNymVpnConfig {
    pub mixnet_client_config: MixnetClientConfig,

    /// Path to the data directory, where keys reside.
    pub data_path: Option<PathBuf>,

    /// Gateway configuration
    pub gateway_config: GatewayDirectoryConfig,

    /// Mixnet public ID of the entry gateway.
    pub entry_point: EntryPoint,

    /// Mixnet recipient address.
    pub exit_point: ExitPoint,

    /// The IP addresses of the TUN device.
    pub nym_ips: Option<IpPair>,

    /// The MTU of the TUN device.
    pub nym_mtu: Option<u16>,

    /// The DNS server to use
    pub dns: Option<IpAddr>,

    /// Disable routing all traffic through the VPN TUN device.
    pub disable_routing: bool,

    /// The user agent to use for HTTP requests. This includes client name, version, platform and
    /// git commit hash.
    pub user_agent: Option<UserAgent>,
}

pub struct NymVpn<T: Vpn> {
    /// VPN configuration, independent of the type used
    pub generic_config: GenericNymVpnConfig,

    /// VPN configuration, depending on the type used
    pub vpn_config: T,

    #[cfg(not(target_os = "ios"))]
    tun_provider: Arc<Mutex<TunProvider>>,

    #[cfg(target_os = "ios")]
    ios_tun_provider: Arc<dyn OSTunProvider>,

    // Necessary so that the device doesn't get closed before cleanup has taken place
    shadow_handle: ShadowHandle,
}

#[derive(Debug, Clone, Copy)]
pub struct MixnetConnectionInfo {
    pub nym_address: Recipient,
    pub entry_gateway: NodeIdentity,
}

#[derive(Debug, Clone, Copy)]
pub struct MixnetExitConnectionInfo {
    pub exit_gateway: NodeIdentity,
    pub exit_ipr: Recipient,
    pub ips: IpPair,
}

impl NymVpn<WireguardVpn> {
    pub fn new_wireguard_vpn(
        entry_point: EntryPoint,
        exit_point: ExitPoint,
        #[cfg(target_os = "android")] android_context: talpid_types::android::AndroidContext,
        #[cfg(target_os = "ios")] ios_tun_provider: Arc<dyn OSTunProvider>,
    ) -> Self {
        #[cfg(not(target_os = "ios"))]
        let tun_provider = Arc::new(Mutex::new(TunProvider::new(
            #[cfg(target_os = "android")]
            android_context,
            #[cfg(target_os = "android")]
            false,
            #[cfg(target_os = "android")]
            None,
            #[cfg(target_os = "android")]
            vec![],
        )));

        Self {
            generic_config: GenericNymVpnConfig {
                mixnet_client_config: MixnetClientConfig {
                    enable_poisson_rate: false,
                    disable_background_cover_traffic: false,
                    enable_credentials_mode: false,
                    min_mixnode_performance: None,
                    min_gateway_performance: None,
                },
                data_path: None,
                gateway_config: nym_gateway_directory::Config::default(),
                entry_point,
                exit_point,
                nym_ips: None,
                nym_mtu: None,
                dns: None,
                disable_routing: false,
                user_agent: None,
            },
            vpn_config: WireguardVpn {},
            #[cfg(not(target_os = "ios"))]
            tun_provider,
            #[cfg(target_os = "ios")]
            ios_tun_provider,
            shadow_handle: ShadowHandle::default(),
        }
    }
}

impl NymVpn<MixnetVpn> {
    pub fn new_mixnet_vpn(
        entry_point: EntryPoint,
        exit_point: ExitPoint,
        #[cfg(target_os = "android")] android_context: talpid_types::android::AndroidContext,
        #[cfg(target_os = "ios")] ios_tun_provider: Arc<dyn OSTunProvider>,
    ) -> Self {
        #[cfg(not(target_os = "ios"))]
        let tun_provider = Arc::new(Mutex::new(TunProvider::new(
            #[cfg(target_os = "android")]
            android_context,
            #[cfg(target_os = "android")]
            false,
            #[cfg(target_os = "android")]
            None,
            #[cfg(target_os = "android")]
            vec![],
        )));

        Self {
            generic_config: GenericNymVpnConfig {
                mixnet_client_config: MixnetClientConfig {
                    enable_poisson_rate: false,
                    disable_background_cover_traffic: false,
                    enable_credentials_mode: false,
                    min_mixnode_performance: None,
                    min_gateway_performance: None,
                },
                data_path: None,
                gateway_config: nym_gateway_directory::Config::default(),
                entry_point,
                exit_point,
                nym_ips: None,
                nym_mtu: None,
                dns: None,
                disable_routing: false,
                user_agent: None,
            },
            vpn_config: MixnetVpn {},
            #[cfg(not(target_os = "ios"))]
            tun_provider,
            #[cfg(target_os = "ios")]
            ios_tun_provider,
            shadow_handle: ShadowHandle::default(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn setup_post_mixnet(
        &mut self,
        mixnet_client: SharedMixnetClient,
        #[cfg(not(target_os = "ios"))] route_manager: &mut RouteManager,
        exit_mix_addresses: &IpPacketRouterAddress,
        task_manager: &TaskManager,
        gateway_client: &GatewayClient,
        default_lan_gateway_ip: routing::LanGatewayIp,
        #[cfg(not(target_os = "ios"))] dns_monitor: &mut DnsMonitor,
    ) -> Result<MixnetExitConnectionInfo> {
        let exit_gateway = *exit_mix_addresses.gateway();
        info!("Connecting to exit gateway: {exit_gateway}");
        // Currently the IPR client is only used to connect. The next step would be to use it to
        // spawn a separate task that handles IPR request/responses.
        let mut ipr_client = IprClientConnect::new_from_inner(mixnet_client.inner()).await;
        let our_ips = ipr_client
            .connect(exit_mix_addresses.0, self.generic_config.nym_ips)
            .await?;
        info!("Successfully connected to exit gateway");
        info!("Using mixnet VPN IP addresses: {our_ips}");

        // We need the IP of the gateway to correctly configure the routing table
        let mixnet_client_address = mixnet_client.nym_address().await;
        let gateway_used = mixnet_client_address.gateway().to_base58_string();
        debug!("Entry gateway used for setting up routing table: {gateway_used}");
        let entry_mixnet_gateway_ip: IpAddr = gateway_client
            .lookup_gateway_ip(&gateway_used)
            .await
            .map_err(|source| GatewayDirectoryError::FailedToLookupGatewayIp {
                gateway_id: gateway_used,
                source,
            })?;
        debug!("Gateway ip resolves to: {entry_mixnet_gateway_ip}");

        info!("Setting up routing");
        let routing_config = routing::RoutingConfig::new(
            self,
            our_ips,
            entry_mixnet_gateway_ip,
            default_lan_gateway_ip,
            #[cfg(target_os = "android")]
            mixnet_client.gateway_ws_fd().await,
        );
        debug!("Routing config: {}", routing_config);
        #[cfg(target_os = "ios")]
        let mixnet_tun_dev =
            routing::setup_mixnet_routing(routing_config, self.ios_tun_provider.clone()).await?;

        #[cfg(not(target_os = "ios"))]
        let mixnet_tun_dev = routing::setup_mixnet_routing(
            route_manager,
            routing_config,
            dns_monitor,
            self.generic_config.dns,
        )
        .await?;

        info!("Setting up mixnet processor");
        let processor_config = mixnet_processor::Config::new(exit_mix_addresses.0);
        debug!("Mixnet processor config: {:#?}", processor_config);

        // For other components that will want to send mixnet packets
        let mixnet_client_sender = mixnet_client.split_sender().await;

        // Setup connection monitor shared tag and channels
        let connection_monitor = ConnectionMonitorTask::setup();

        let shadow_handle = mixnet_processor::start_processor(
            processor_config,
            mixnet_tun_dev,
            mixnet_client,
            task_manager,
            our_ips,
            &connection_monitor,
        )
        .await;
        self.set_shadow_handle(shadow_handle);

        connection_monitor.start(
            mixnet_client_sender,
            mixnet_client_address,
            our_ips,
            exit_mix_addresses.0,
            task_manager,
        );

        Ok(MixnetExitConnectionInfo {
            exit_gateway,
            exit_ipr: exit_mix_addresses.0,
            ips: our_ips,
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn setup_tunnel_services(
        &mut self,
        mixnet_client: SharedMixnetClient,
        #[cfg(not(target_os = "ios"))] route_manager: &mut RouteManager,
        exit_mix_addresses: &IpPacketRouterAddress,
        task_manager: &TaskManager,
        gateway_client: &GatewayClient,
        default_lan_gateway_ip: routing::LanGatewayIp,
        #[cfg(not(target_os = "ios"))] dns_monitor: &mut DnsMonitor,
    ) -> Result<(MixnetConnectionInfo, MixnetExitConnectionInfo)> {
        // Now that we have a connection, collection some info about that and return
        let nym_address = mixnet_client.nym_address().await;
        let entry_gateway = *(nym_address.gateway());
        info!("Successfully connected to entry gateway: {entry_gateway}");

        let our_mixnet_connection = MixnetConnectionInfo {
            nym_address,
            entry_gateway,
        };

        // Check that we can ping ourselves before continuing
        info!("Sending mixnet ping to ourselves to verify mixnet connection");
        nym_connection_monitor::self_ping_and_wait(nym_address, mixnet_client.inner()).await?;
        info!("Successfully mixnet pinged ourselves");

        match self
            .setup_post_mixnet(
                mixnet_client.clone(),
                #[cfg(not(target_os = "ios"))]
                route_manager,
                exit_mix_addresses,
                task_manager,
                gateway_client,
                default_lan_gateway_ip,
                #[cfg(not(target_os = "ios"))]
                dns_monitor,
            )
            .await
        {
            Err(err) => {
                error!("Failed to setup post mixnet: {err}");
                debug!("{err:?}");
                mixnet_client.disconnect().await;
                Err(err)
            }
            Ok(exit_connection_info) => Ok((our_mixnet_connection, exit_connection_info)),
        }
    }
}

impl<T: Vpn> NymVpn<T> {
    pub(crate) fn set_shadow_handle(&mut self, shadow_handle: JoinHandle<Result<AsyncDevice>>) {
        self.shadow_handle = ShadowHandle {
            _inner: Some(shadow_handle),
        }
    }
}
impl SpecificVpn {
    pub fn mixnet_client_config(&self) -> MixnetClientConfig {
        match self {
            SpecificVpn::Wg(vpn) => vpn.generic_config.mixnet_client_config.clone(),
            SpecificVpn::Mix(vpn) => vpn.generic_config.mixnet_client_config.clone(),
        }
    }

    pub fn data_path(&self) -> Option<PathBuf> {
        match self {
            SpecificVpn::Wg(vpn) => vpn.generic_config.data_path.clone(),
            SpecificVpn::Mix(vpn) => vpn.generic_config.data_path.clone(),
        }
    }

    pub fn gateway_config(&self) -> GatewayDirectoryConfig {
        match self {
            SpecificVpn::Wg(vpn) => vpn.generic_config.gateway_config.clone(),
            SpecificVpn::Mix(vpn) => vpn.generic_config.gateway_config.clone(),
        }
    }

    pub fn entry_point(&self) -> EntryPoint {
        match self {
            SpecificVpn::Wg(vpn) => vpn.generic_config.entry_point.clone(),
            SpecificVpn::Mix(vpn) => vpn.generic_config.entry_point.clone(),
        }
    }

    pub fn exit_point(&self) -> ExitPoint {
        match self {
            SpecificVpn::Wg(vpn) => vpn.generic_config.exit_point.clone(),
            SpecificVpn::Mix(vpn) => vpn.generic_config.exit_point.clone(),
        }
    }

    pub fn user_agent(&self) -> Option<UserAgent> {
        match self {
            SpecificVpn::Wg(vpn) => vpn.generic_config.user_agent.clone(),
            SpecificVpn::Mix(vpn) => vpn.generic_config.user_agent.clone(),
        }
    }

    // Start the Nym VPN client, and wait for it to shutdown. The use case is in simple console
    // applications where the main way to interact with the running process is to send SIGINT
    // (ctrl-c)
    #[cfg(not(target_os = "ios"))]
    pub async fn run(&mut self) -> Result<()> {
        let mut task_manager = TaskManager::new(SHUTDOWN_TIMER_SECS).named("nym_vpn_lib");
        info!("Setting up route manager");
        #[cfg(not(target_os = "ios"))]
        let mut route_manager = setup_route_manager().await?;
        #[cfg(not(target_os = "ios"))]
        let (mut firewall, mut dns_monitor) = init_firewall_dns(
            #[cfg(target_os = "linux")]
            route_manager.handle()?,
        )
        .await?;
        let tunnels = match setup_tunnel(
            self,
            &mut task_manager,
            #[cfg(not(target_os = "ios"))]
            &mut route_manager,
            #[cfg(not(target_os = "ios"))]
            &mut dns_monitor,
        )
        .await
        {
            Ok(tunnels) => tunnels,
            Err(e) => {
                #[cfg(not(target_os = "ios"))]
                tokio::task::spawn_blocking(move || {
                    dns_monitor
                        .reset()
                        .inspect_err(|err| {
                            log::error!("Failed to reset dns monitor: {err}");
                        })
                        .ok();
                    firewall
                        .reset_policy()
                        .inspect_err(|err| {
                            error!("Failed to reset firewall policy: {err}");
                        })
                        .ok();
                    drop(route_manager);
                })
                .await?;
                return Err(e);
            }
        };
        info!("Nym VPN is now running");

        // Finished starting everything, now wait for mixnet client shutdown
        match tunnels {
            AllTunnelsSetup::Mix(_) => {
                #[cfg(not(target_os = "ios"))]
                wait_and_handle_interrupt(
                    &mut task_manager,
                    #[cfg(not(target_os = "ios"))]
                    route_manager,
                    #[cfg(not(target_os = "ios"))]
                    None,
                )
                .await;
                #[cfg(not(target_os = "ios"))]
                tokio::task::spawn_blocking(move || {
                    dns_monitor.reset().inspect_err(|err| {
                        log::error!("Failed to reset dns monitor: {err}");
                    })
                })
                .await??;
            }
            AllTunnelsSetup::Wg { entry, exit } => {
                wait_and_handle_interrupt(
                    &mut task_manager,
                    route_manager,
                    Some([entry.specific_setup, exit.specific_setup]),
                )
                .await;

                tokio::task::spawn_blocking(move || {
                    dns_monitor.reset().inspect_err(|err| {
                        log::error!("Failed to reset dns monitor: {err}");
                    })
                })
                .await??;
                firewall.reset_policy().map_err(|err| {
                    error!("Failed to reset firewall policy: {err}");
                    Error::FirewallError(err.to_string())
                })?;
            }
        }

        Ok(())
    }

    // Start the Nym VPN client, but also listen for external messages to e.g. disconnect as well
    // as reporting it's status on the provided channel. The usecase when the VPN is embedded in
    // another application, or running as a background process with a graphical interface remote
    // controlling it.
    #[cfg(not(target_os = "ios"))]
    pub async fn run_and_listen(
        &mut self,
        mut vpn_status_tx: nym_task::StatusSender,
        vpn_ctrl_rx: mpsc::UnboundedReceiver<NymVpnCtrlMessage>,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut task_manager = TaskManager::new(SHUTDOWN_TIMER_SECS).named("nym_vpn_lib");

        #[cfg(not(target_os = "ios"))]
        info!("Setting up route manager");
        #[cfg(not(target_os = "ios"))]
        #[cfg(not(target_os = "ios"))]
        let mut route_manager = setup_route_manager().await?;
        #[cfg(not(target_os = "ios"))]
        let (mut firewall, mut dns_monitor) = init_firewall_dns(
            #[cfg(target_os = "linux")]
            route_manager.handle()?,
        )
        .await?;
        let tunnels = match setup_tunnel(
            self,
            &mut task_manager,
            #[cfg(not(target_os = "ios"))]
            &mut route_manager,
            #[cfg(not(target_os = "ios"))]
            &mut dns_monitor,
        )
        .await
        {
            Ok(tunnels) => tunnels,
            Err(e) => {
                #[cfg(not(target_os = "ios"))]
                tokio::task::spawn_blocking(move || {
                    dns_monitor
                        .reset()
                        .inspect_err(|err| {
                            log::error!("Failed to reset dns monitor: {err}");
                        })
                        .ok();
                    firewall
                        .reset_policy()
                        .inspect_err(|err| {
                            error!("Failed to reset firewall policy: {err}");
                        })
                        .ok();
                    drop(route_manager);
                })
                .await?;
                return Err(Box::new(e));
            }
        };

        // Finished starting everything, now wait for mixnet client shutdown
        match tunnels {
            AllTunnelsSetup::Mix(TunnelSetup { specific_setup, .. }) => {
                // Signal back that mixnet is ready and up with all cylinders firing
                // TODO: this should actually be sent much earlier, when the mixnet client is
                // connected. However that would also require starting the status listener earlier.
                // This means that for now, we basically just ignore the status message and use the
                // NymVpnStatusMessage2 sent below instead.
                let start_status = TaskStatus::ReadyWithGateway(
                    specific_setup
                        .mixnet_connection_info
                        .entry_gateway
                        .to_base58_string(),
                );
                task_manager
                    .start_status_listener(vpn_status_tx.clone(), start_status)
                    .await;

                vpn_status_tx
                    .send(Box::new(NymVpnStatusMessage::MixnetConnectionInfo {
                        mixnet_connection_info: specific_setup.mixnet_connection_info,
                        mixnet_exit_connection_info: specific_setup.exit_connection_info,
                    }))
                    .await
                    .unwrap();

                let result = wait_for_interrupt_and_signal(
                    Some(task_manager),
                    vpn_ctrl_rx,
                    #[cfg(not(target_os = "ios"))]
                    route_manager,
                    #[cfg(not(target_os = "ios"))]
                    None,
                )
                .await;
                #[cfg(not(target_os = "ios"))]
                tokio::task::spawn_blocking(move || {
                    dns_monitor.reset().inspect_err(|err| {
                        log::error!("Failed to reset dns monitor: {err}");
                    })
                })
                .await??;
                result
            }
            #[cfg(not(target_os = "ios"))]
            AllTunnelsSetup::Wg { entry, exit } => {
                let result = wait_for_interrupt_and_signal(
                    Some(task_manager),
                    vpn_ctrl_rx,
                    route_manager,
                    Some([entry.specific_setup, exit.specific_setup]),
                )
                .await;
                tokio::task::spawn_blocking(move || {
                    dns_monitor.reset().inspect_err(|err| {
                        log::error!("Failed to reset dns monitor: {err}");
                    })
                })
                .await??;
                firewall.reset_policy().map_err(|err| {
                    error!("Failed to reset firewall policy: {err}");
                    NymVpnExitError::FailedToResetFirewallPolicy {
                        reason: err.to_string(),
                    }
                })?;
                result
            }
        }
    }
}

#[derive(thiserror::Error, Clone, Debug)]
pub enum NymVpnStatusMessage {
    #[error("mixnet connection info")]
    MixnetConnectionInfo {
        mixnet_connection_info: MixnetConnectionInfo,
        mixnet_exit_connection_info: MixnetExitConnectionInfo,
    },
}

#[derive(Debug)]
pub enum NymVpnCtrlMessage {
    Stop,
}

// We are mapping all errors to a generic error since I ran into issues with the error type
// on a platform (mac) that I wasn't able to troubleshoot on in time. Basically it seemed like
// not all error cases satisfied the Sync marker trait.
#[derive(thiserror::Error, Debug)]
pub enum NymVpnExitError {
    #[error("{reason}")]
    Generic { reason: Error },

    // TODO: capture the concrete error type once we have time to investigate on Mac
    #[error("failed to reset firewall policy: {reason}")]
    FailedToResetFirewallPolicy { reason: String },

    #[error("failed to reset dns monitor: {reason}")]
    FailedToResetDnsMonitor { reason: String },
}

#[derive(Debug)]
pub enum NymVpnExitStatusMessage {
    Stopped,
    Failed(Box<dyn std::error::Error + Send + Sync + 'static>),
}

/// Starts the Nym VPN client.
///
/// Examples
///
/// ```no_run
/// use nym_vpn_lib::gateway_directory::{EntryPoint, ExitPoint};
/// use nym_vpn_lib::NodeIdentity;
///
/// let mut vpn_config = nym_vpn_lib::NymVpn::new_mixnet_vpn(EntryPoint::Gateway { identity: NodeIdentity::from_base58_string("Qwertyuiopasdfghjklzxcvbnm1234567890").unwrap()},
/// ExitPoint::Gateway { identity: NodeIdentity::from_base58_string("Qwertyuiopasdfghjklzxcvbnm1234567890".to_string()).unwrap()});
/// let vpn_handle = nym_vpn_lib::spawn_nym_vpn(vpn_config.into());
/// ```
///
/// ```no_run
/// use nym_vpn_lib::gateway_directory::{EntryPoint, ExitPoint};
/// use nym_vpn_lib::NodeIdentity;
///
/// let mut vpn_config = nym_vpn_lib::NymVpn::new_wireguard_vpn(EntryPoint::Gateway { identity: NodeIdentity::from_base58_string("Qwertyuiopasdfghjklzxcvbnm1234567890").unwrap()},
/// ExitPoint::Gateway { identity: NodeIdentity::from_base58_string("Qwertyuiopasdfghjklzxcvbnm1234567890".to_string()).unwrap()});
/// let vpn_handle = nym_vpn_lib::spawn_nym_vpn(vpn_config.into());
/// ```
#[cfg(not(target_os = "ios"))]
pub fn spawn_nym_vpn(nym_vpn: SpecificVpn) -> Result<NymVpnHandle> {
    let (vpn_ctrl_tx, vpn_ctrl_rx) = mpsc::unbounded();
    let (vpn_status_tx, vpn_status_rx) = mpsc::channel(128);
    let (vpn_exit_tx, vpn_exit_rx) = oneshot::channel();

    tokio::spawn(run_nym_vpn(
        nym_vpn,
        vpn_status_tx,
        vpn_ctrl_rx,
        vpn_exit_tx,
    ));

    Ok(NymVpnHandle {
        vpn_ctrl_tx,
        vpn_status_rx,
        vpn_exit_rx,
    })
}

/// Starts the Nym VPN client, in a separate tokio runtime.
///
/// Examples
///
/// ```no_run
/// use nym_vpn_lib::gateway_directory::{EntryPoint, ExitPoint};
/// use nym_vpn_lib::NodeIdentity;
///
/// let mut vpn_config = nym_vpn_lib::NymVpn::new_mixnet_vpn(EntryPoint::Gateway { identity: NodeIdentity::from_base58_string("Qwertyuiopasdfghjklzxcvbnm1234567890").unwrap()},
/// ExitPoint::Gateway { identity: NodeIdentity::from_base58_string("Qwertyuiopasdfghjklzxcvbnm1234567890".to_string()).unwrap()});
/// let vpn_handle = nym_vpn_lib::spawn_nym_vpn_with_new_runtime(vpn_config.into());
/// ```
///
/// ```no_run
/// use nym_vpn_lib::gateway_directory::{EntryPoint, ExitPoint};
/// use nym_vpn_lib::NodeIdentity;
///
/// let mut vpn_config = nym_vpn_lib::NymVpn::new_wireguard_vpn(EntryPoint::Gateway { identity: NodeIdentity::from_base58_string("Qwertyuiopasdfghjklzxcvbnm1234567890").unwrap()},
/// ExitPoint::Gateway { identity: NodeIdentity::from_base58_string("Qwertyuiopasdfghjklzxcvbnm1234567890".to_string()).unwrap()});
/// let vpn_handle = nym_vpn_lib::spawn_nym_vpn_with_new_runtime(vpn_config.into());
/// ```
#[cfg(not(target_os = "ios"))]
pub fn spawn_nym_vpn_with_new_runtime(nym_vpn: SpecificVpn) -> Result<NymVpnHandle> {
    let (vpn_ctrl_tx, vpn_ctrl_rx) = mpsc::unbounded();
    let (vpn_status_tx, vpn_status_rx) = mpsc::channel(128);
    let (vpn_exit_tx, vpn_exit_rx) = oneshot::channel();

    std::thread::spawn(|| {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio run time");
        rt.block_on(run_nym_vpn(
            nym_vpn,
            vpn_status_tx,
            vpn_ctrl_rx,
            vpn_exit_tx,
        ));
    });

    Ok(NymVpnHandle {
        vpn_ctrl_tx,
        vpn_status_rx,
        vpn_exit_rx,
    })
}

#[cfg(not(target_os = "ios"))]
async fn run_nym_vpn(
    mut nym_vpn: SpecificVpn,
    vpn_status_tx: nym_task::StatusSender,
    vpn_ctrl_rx: mpsc::UnboundedReceiver<NymVpnCtrlMessage>,
    vpn_exit_tx: oneshot::Sender<NymVpnExitStatusMessage>,
) {
    match nym_vpn.run_and_listen(vpn_status_tx, vpn_ctrl_rx).await {
        Ok(()) => {
            log::info!("Nym VPN has shut down");
            vpn_exit_tx
                .send(NymVpnExitStatusMessage::Stopped)
                .expect("Failed to send exit status");
        }
        Err(err) => {
            error!("Nym VPN returned error: {err}");
            debug!("{err:?}");
            uniffi_set_listener_status(StatusEvent::Exit(ExitStatus::Failed {
                error: err.to_string(),
            }));
            vpn_exit_tx
                .send(NymVpnExitStatusMessage::Failed(err))
                .expect("Failed to send exit status");
        }
    }
}

pub struct NymVpnHandle {
    pub vpn_ctrl_tx: mpsc::UnboundedSender<NymVpnCtrlMessage>,
    pub vpn_status_rx: nym_task::StatusReceiver,
    pub vpn_exit_rx: oneshot::Receiver<NymVpnExitStatusMessage>,
}
