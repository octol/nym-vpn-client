// Copyright 2023 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: GPL-3.0-only
// #![cfg_attr(not(target_os = "macos"), allow(dead_code))]

#[cfg(target_os = "android")]
pub mod android;
pub(crate) mod error;
#[cfg(any(target_os = "ios", target_os = "macos"))]
pub mod swift;

mod account;

use std::{env, path::PathBuf, sync::Arc, time::Duration};

use account::AccountControllerHandle;
use lazy_static::lazy_static;
use log::*;
use tokio::{
    runtime::Runtime,
    sync::{mpsc, Mutex},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use url::Url;

use nym_gateway_directory::Config as GatewayDirectoryConfig;

use self::error::VpnError;
#[cfg(target_os = "android")]
use crate::tunnel_provider::android::AndroidTunProvider;
#[cfg(target_os = "ios")]
use crate::tunnel_provider::ios::OSTunProvider;
use crate::{
    gateway_directory::GatewayClient,
    tunnel_state_machine::{
        BandwidthEvent, ConnectionEvent, DnsOptions, GatewayPerformanceOptions,
        MixnetTunnelOptions, NymConfig, TunnelCommand, TunnelEvent, TunnelSettings, TunnelState,
        TunnelStateMachine, TunnelType, WireguardTunnelOptions,
    },
    uniffi_custom_impls::{
        AccountLinks, AccountStateSummary, BandwidthStatus, ConnectionStatus, EntryPoint,
        ExitPoint, GatewayMinPerformance, GatewayType, Location, NetworkEnvironment, SystemMessage,
        TunStatus, UserAgent,
    },
};

lazy_static! {
    static ref RUNTIME: Runtime = Runtime::new().unwrap();
    static ref STATE_MACHINE_HANDLE: Mutex<Option<StateMachineHandle>> = Mutex::new(None);
    static ref ACCOUNT_CONTROLLER_HANDLE: Mutex<Option<AccountControllerHandle>> = Mutex::new(None);
    static ref NETWORK_ENVIRONMENT: Mutex<Option<nym_vpn_network_config::Network>> =
        Mutex::new(None);
}

#[allow(non_snake_case)]
#[uniffi::export]
pub fn startVPN(config: VPNConfig) -> Result<(), VpnError> {
    RUNTIME.block_on(start_vpn_inner(config))
}

async fn start_vpn_inner(config: VPNConfig) -> Result<(), VpnError> {
    // TODO: we do a pre-connect check here. This mirrors the logic in the daemon.
    // We want to move this check into the state machine so that it happens during the connecting
    // state instead. This would allow us more flexibility in waiting for the account to be ready
    // and handle errors in a unified manner.
    let timeout = Duration::from_secs(10);
    account::assert_account_ready_to_connect(timeout).await?;

    let mut guard = STATE_MACHINE_HANDLE.lock().await;

    if guard.is_none() {
        let state_machine_handle = start_state_machine(config).await?;
        state_machine_handle.send_command(TunnelCommand::Connect);
        *guard = Some(state_machine_handle);
        Ok(())
    } else {
        Err(VpnError::InvalidStateError {
            details: "State machine is already running.".to_owned(),
        })
    }
}

#[allow(non_snake_case)]
#[uniffi::export]
pub fn stopVPN() -> Result<(), VpnError> {
    RUNTIME.block_on(stop_vpn_inner())
}

async fn stop_vpn_inner() -> Result<(), VpnError> {
    let mut guard = STATE_MACHINE_HANDLE.lock().await;

    match guard.take() {
        Some(state_machine_handle) => {
            // TODO: add timeout
            state_machine_handle.shutdown_and_wait().await;
            Ok(())
        }
        None => Err(VpnError::InvalidStateError {
            details: "State machine is not running.".to_owned(),
        }),
    }
}

#[allow(non_snake_case)]
#[uniffi::export]
pub fn configureLib(data_dir: String) -> Result<(), VpnError> {
    init_logger();
    start_account_controller(data_dir)
}

#[allow(non_snake_case)]
#[uniffi::export]
pub fn shutdown() -> Result<(), VpnError> {
    RUNTIME.block_on(account::stop_account_controller_inner())
}

fn start_account_controller(data_dir: String) -> Result<(), VpnError> {
    RUNTIME.block_on(account::start_account_controller_inner(PathBuf::from(
        data_dir,
    )))
}

pub fn init_logger() {
    let log_level = env::var("RUST_LOG").unwrap_or("info".to_string());
    info!("Setting log level: {}", log_level);
    #[cfg(target_os = "ios")]
    swift::init_logs(log_level);
    #[cfg(target_os = "android")]
    android::init_logs(log_level);
}

#[allow(non_snake_case)]
#[uniffi::export]
pub fn initLogger() {
    init_logger();
}

/// Fetches the network environment details from the network name and initializes the environment,
/// including exporting to the environment
#[allow(non_snake_case)]
#[uniffi::export]
pub fn initEnvironment(network_name: &str) -> Result<(), VpnError> {
    RUNTIME.block_on(init_environment(network_name))
}

async fn init_environment(network_name: &str) -> Result<(), VpnError> {
    let network = nym_vpn_network_config::Network::fetch(network_name).map_err(|err| {
        VpnError::InternalError {
            details: err.to_string(),
        }
    })?;

    // To bridge with old code, export to environment. New code should now rely on this.
    network.export_to_env();

    let mut guard = NETWORK_ENVIRONMENT.lock().await;
    *guard = Some(network);

    Ok(())
}

#[allow(non_snake_case)]
#[uniffi::export]
pub fn currentEnvironment() -> Result<NetworkEnvironment, VpnError> {
    RUNTIME.block_on(current_environment())
}

async fn current_environment() -> Result<NetworkEnvironment, VpnError> {
    let network = NETWORK_ENVIRONMENT.lock().await.clone();
    network
        .map(NetworkEnvironment::from)
        .ok_or(VpnError::InternalError {
            details: "No network environment initialized".to_string(),
        })
}

// Fetch the network environment details from the network name.
// TODO: also add the ability to catch this information for subsequent use.
#[allow(non_snake_case)]
#[uniffi::export]
pub fn fetchEnvironment(network_name: &str) -> Result<NetworkEnvironment, VpnError> {
    RUNTIME.block_on(fetch_environment(network_name))
}

async fn fetch_environment(network_name: &str) -> Result<NetworkEnvironment, VpnError> {
    nym_vpn_network_config::Network::fetch(network_name)
        .map(NetworkEnvironment::from)
        .map_err(|err| VpnError::InternalError {
            details: err.to_string(),
        })
}

#[allow(non_snake_case)]
#[uniffi::export]
pub fn fetchSystemMessages(network_name: &str) -> Result<Vec<SystemMessage>, VpnError> {
    RUNTIME.block_on(fetch_system_messages(network_name))
}

async fn fetch_system_messages(network_name: &str) -> Result<Vec<SystemMessage>, VpnError> {
    nym_vpn_network_config::Network::fetch(network_name)
        .map(|network| {
            network
                .nym_vpn_network
                .system_messages
                .into_current_iter()
                .map(SystemMessage::from)
                .collect()
        })
        .map_err(|err| VpnError::InternalError {
            details: err.to_string(),
        })
}

#[allow(non_snake_case)]
#[uniffi::export]
pub fn fetchAccountLinks(
    account_store_path: &str,
    network_name: &str,
    locale: &str,
) -> Result<AccountLinks, VpnError> {
    RUNTIME.block_on(fetch_account_links(
        account_store_path,
        network_name,
        locale,
    ))
}

async fn fetch_account_links(
    path: &str,
    network_name: &str,
    locale: &str,
) -> Result<AccountLinks, VpnError> {
    let account_id = account::get_account_id(path).await?;
    nym_vpn_network_config::Network::fetch(network_name)
        .and_then(|network| {
            network
                .nym_vpn_network
                .try_into_parsed_links(locale, &account_id)
        })
        .map(AccountLinks::from)
        .map_err(|err| VpnError::InternalError {
            details: err.to_string(),
        })
}

#[allow(non_snake_case)]
#[uniffi::export]
pub fn storeAccountMnemonic(mnemonic: String, path: String) -> Result<(), VpnError> {
    RUNTIME.block_on(account::store_account_mnemonic(&mnemonic, &path))
}

#[allow(non_snake_case)]
#[uniffi::export]
pub fn isAccountMnemonicStored(path: String) -> Result<bool, VpnError> {
    RUNTIME.block_on(account::is_account_mnemonic_stored(&path))
}

#[allow(non_snake_case)]
#[uniffi::export]
pub fn removeAccountMnemonic(path: String) -> Result<bool, VpnError> {
    RUNTIME.block_on(account::remove_account_mnemonic(&path))
}

#[allow(non_snake_case)]
#[uniffi::export]
pub fn resetDeviceIdentity(path: String) -> Result<(), VpnError> {
    RUNTIME.block_on(account::reset_device_identity(&path))
}

#[allow(non_snake_case)]
#[uniffi::export]
pub fn updateAccountState() -> Result<(), VpnError> {
    RUNTIME.block_on(account::update_account_state())
}

#[allow(non_snake_case)]
#[uniffi::export]
pub fn getAccountState() -> Result<AccountStateSummary, VpnError> {
    RUNTIME.block_on(account::get_account_state())
}

#[allow(non_snake_case)]
#[uniffi::export]
pub fn getGatewayCountries(
    gw_type: GatewayType,
    user_agent: Option<UserAgent>,
    min_gateway_performance: Option<GatewayMinPerformance>,
) -> Result<Vec<Location>, VpnError> {
    let (api_url, nym_vpn_api_url) = get_nym_urls()?;

    RUNTIME.block_on(get_gateway_countries(
        api_url,
        nym_vpn_api_url,
        gw_type,
        user_agent,
        min_gateway_performance,
    ))
}

async fn get_gateway_countries(
    api_url: Url,
    nym_vpn_api_url: Url,
    gw_type: GatewayType,
    user_agent: Option<UserAgent>,
    min_gateway_performance: Option<GatewayMinPerformance>,
) -> Result<Vec<Location>, VpnError> {
    let user_agent = user_agent
        .map(nym_sdk::UserAgent::from)
        .unwrap_or_else(crate::util::construct_user_agent);
    let min_gateway_performance = min_gateway_performance.map(|p| p.try_into()).transpose()?;
    let directory_config = nym_gateway_directory::Config {
        api_url,
        nym_vpn_api_url: Some(nym_vpn_api_url),
        min_gateway_performance,
    };
    GatewayClient::new(directory_config, user_agent)?
        .lookup_countries(gw_type.into())
        .await
        .map(|countries| countries.into_iter().map(Location::from).collect())
        .map_err(VpnError::from)
}

#[allow(non_snake_case)]
#[uniffi::export]
pub fn getLowLatencyEntryCountry(user_agent: UserAgent) -> Result<Location, VpnError> {
    let (api_url, vpn_api_url) = get_nym_urls()?;

    RUNTIME.block_on(get_low_latency_entry_country(
        api_url,
        vpn_api_url,
        user_agent,
    ))
}

async fn get_low_latency_entry_country(
    api_url: Url,
    vpn_api_url: Url,
    user_agent: UserAgent,
) -> Result<Location, VpnError> {
    let config = nym_gateway_directory::Config {
        api_url,
        nym_vpn_api_url: Some(vpn_api_url),
        min_gateway_performance: None,
    };
    GatewayClient::new(config, user_agent.into())?
        .lookup_low_latency_entry_gateway()
        .await
        .map_err(VpnError::from)
        .and_then(|gateway| {
            gateway.location.ok_or(VpnError::InternalError {
                details: "gateway does not contain a two character country ISO".to_string(),
            })
        })
        .map(Location::from)
}

#[derive(uniffi::Record)]
pub struct VPNConfig {
    pub entry_gateway: EntryPoint,
    pub exit_router: ExitPoint,
    pub enable_two_hop: bool,
    #[cfg(target_os = "android")]
    pub tun_provider: Arc<dyn AndroidTunProvider>,
    #[cfg(target_os = "ios")]
    pub tun_provider: Arc<dyn OSTunProvider>,
    pub credential_data_path: Option<PathBuf>,
    pub tun_status_listener: Option<Arc<dyn TunnelStatusListener>>,
}

#[uniffi::export(with_foreign)]
pub trait TunnelStatusListener: Send + Sync {
    fn on_event(&self, event: TunnelEvent);
}

struct StateMachineHandle {
    state_machine_handle: JoinHandle<()>,
    event_broadcaster_handler: JoinHandle<()>,
    command_sender: mpsc::UnboundedSender<TunnelCommand>,
    shutdown_token: CancellationToken,
}

impl StateMachineHandle {
    fn send_command(&self, command: TunnelCommand) {
        if let Err(e) = self.command_sender.send(command) {
            tracing::error!("Failed to send comamnd: {}", e);
        }
    }

    async fn shutdown_and_wait(self) {
        self.shutdown_token.cancel();

        if let Err(e) = self.state_machine_handle.await {
            tracing::error!("Failed to join on state machine handle: {}", e);
        }

        if let Err(e) = self.event_broadcaster_handler.await {
            tracing::error!("Failed to join on event broadcaster handle: {}", e);
        }
    }
}

fn get_api_url() -> Option<Url> {
    match env::var("NYM_API") {
        Ok(url) => Url::parse(&url).ok(),
        Err(_) => None,
    }
}

fn get_nym_api_url() -> Option<Url> {
    match env::var("NYM_VPN_API") {
        Ok(url) => Url::parse(&url).ok(),
        Err(_) => None,
    }
}

fn get_nym_urls() -> Result<(Url, Url), VpnError> {
    match (get_api_url(), get_nym_api_url()) {
        (Some(api_url), Some(nym_vpn_api_url)) => Ok((api_url, nym_vpn_api_url)),
        _ => Err(VpnError::InternalError {
            details: "NYM_API and NYM_VPN_API environment variables must be set".to_string(),
        }),
    }
}

async fn start_state_machine(config: VPNConfig) -> Result<StateMachineHandle, VpnError> {
    let tunnel_type = if config.enable_two_hop {
        TunnelType::Wireguard
    } else {
        TunnelType::Mixnet
    };

    let entry_point = nym_gateway_directory::EntryPoint::from(config.entry_gateway);
    let exit_point = nym_gateway_directory::ExitPoint::from(config.exit_router);

    let (api_url, nym_vpn_api_url) = get_nym_urls()?;

    let gateway_config = GatewayDirectoryConfig {
        api_url,
        nym_vpn_api_url: Some(nym_vpn_api_url),
        ..Default::default()
    };

    let nym_config = NymConfig {
        data_path: config.credential_data_path,
        gateway_config,
    };

    let tunnel_settings = TunnelSettings {
        tunnel_type,
        enable_credentials_mode: false,
        mixnet_tunnel_options: MixnetTunnelOptions::default(),
        wireguard_tunnel_options: WireguardTunnelOptions::default(),
        gateway_performance_options: GatewayPerformanceOptions::default(),
        mixnet_client_config: None,
        entry_point: Box::new(entry_point),
        exit_point: Box::new(exit_point),
        dns: DnsOptions::default(),
    };

    let (command_sender, command_receiver) = mpsc::unbounded_channel();
    let (event_sender, mut event_receiver) = mpsc::unbounded_channel();

    let state_listener = config.tun_status_listener;
    let event_broadcaster_handler = tokio::spawn(async move {
        while let Some(event) = event_receiver.recv().await {
            if let Some(ref state_listener) = state_listener {
                (*state_listener).on_event(event);
            }
        }
    });

    let shutdown_token = CancellationToken::new();
    let state_machine_handle = TunnelStateMachine::spawn(
        command_receiver,
        event_sender,
        nym_config,
        tunnel_settings,
        #[cfg(any(target_os = "ios", target_os = "android"))]
        config.tun_provider,
        shutdown_token.child_token(),
    )
    .await?;

    Ok(StateMachineHandle {
        state_machine_handle,
        event_broadcaster_handler,
        command_sender,
        shutdown_token,
    })
}

impl From<&TunnelState> for TunStatus {
    fn from(value: &TunnelState) -> Self {
        // TODO: this cannot be accurate so we must switch frontends to use TunnelState instead! But for now that will do.
        match value {
            TunnelState::Connecting { .. } => Self::EstablishingConnection,
            TunnelState::Connected { .. } => Self::Up,
            TunnelState::Disconnecting { .. } => Self::Disconnecting,
            TunnelState::Disconnected => Self::Down,
            TunnelState::Error(_) => Self::Down,
        }
    }
}

impl From<BandwidthEvent> for BandwidthStatus {
    fn from(value: BandwidthEvent) -> Self {
        match value {
            BandwidthEvent::NoBandwidth => Self::NoBandwidth,
            BandwidthEvent::RemainingBandwidth(bandwidth) => Self::RemainingBandwidth { bandwidth },
        }
    }
}

impl From<ConnectionEvent> for ConnectionStatus {
    fn from(value: ConnectionEvent) -> Self {
        match value {
            ConnectionEvent::ConnectedIpv4 => Self::ConnectedIpv4,
            ConnectionEvent::ConnectedIpv6 => Self::ConnectedIpv6,
            ConnectionEvent::EntryGatewayDown => Self::EntryGatewayDown,
            ConnectionEvent::ExitGatewayDownIpv4 => Self::ExitGatewayDownIpv4,
            ConnectionEvent::ExitGatewayDownIpv6 => Self::ExitGatewayDownIpv6,
            ConnectionEvent::ExitGatewayRoutingErrorIpv4 => Self::ExitGatewayRoutingErrorIpv4,
            ConnectionEvent::ExitGatewayRoutingErrorIpv6 => Self::ExitGatewayRoutingErrorIpv6,
        }
    }
}
