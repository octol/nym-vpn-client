// Copyright 2023 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: Apache-2.0
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use self::error::FFIError;
use crate::credentials::{check_credential_base58, import_credential_base58};
use crate::gateway_directory::GatewayClient;
use crate::platform::status_listener::VpnServiceStatusListener;
#[cfg(not(target_os = "ios"))]
use crate::spawn_nym_vpn;
use crate::routing::RoutingConfig;
use crate::uniffi_custom_impls::{
    BandwidthStatus, ConnectionStatus, EntryPoint, ExitPoint, ExitStatus, Location, NymVpnStatus,
    StatusEvent, TunStatus, UserAgent,
};
use crate::{
    NymVpn, NymVpnCtrlMessage, NymVpnExitError, NymVpnExitStatusMessage, NymVpnHandle, SpecificVpn,
};
use crate::{spawn_nym_vpn, MixnetVpn, NymVpn, NymVpnCtrlMessage, NymVpnExitError, NymVpnExitStatusMessage, NymVpnHandle, SpecificVpn};
    spawn_nym_vpn, MixnetVpn, NymVpn, NymVpnCtrlMessage, NymVpnExitError, NymVpnExitStatusMessage,
    NymVpnHandle, SpecificVpn,
};
use ipnetwork::IpNetwork;
use lazy_static::lazy_static;
use log::*;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::SystemTime;
use talpid_core::mpsc::Sender;
use tokio::runtime::Runtime;
use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use url::Url;
use crate::routing::RoutingConfig;
use talpid_types::net::wireguard::{
    PeerConfig as WgPeerConfig, PresharedKey, PrivateKey, PublicKey, TunnelConfig as WgTunnelConfig,
};
use tokio::runtime::Runtime;
use tokio::sync::{Mutex, Notify};
use url::Url;

#[cfg(target_os = "android")]
pub mod android;

pub(crate) mod error;
mod status_listener;
#[cfg(target_os = "ios")]
pub mod swift;
#[cfg(any(target_os = "ios", target_os = "macos"))]
pub mod swift;

lazy_static! {
    static ref VPN_SHUTDOWN_HANDLE: Mutex<Option<Arc<Notify>>> = Mutex::new(None);
    static ref RUNNING: AtomicBool = AtomicBool::new(false);
    static ref RUNTIME: Runtime = Runtime::new().unwrap();
    static ref LISTENER: std::sync::Mutex<Option<Arc<dyn TunnelStatusListener>>> =
        std::sync::Mutex::new(None);
}

#[cfg(target_os = "ios")]
use crate::ios::two_hop_tunnel::TwoHopTunnel;

#[cfg(target_os = "ios")]
struct ShutdownHandle {
    join_handle: JoinHandle<()>,
    shutdown_token: CancellationToken,
}

#[cfg(target_os = "ios")]
impl ShutdownHandle {
    async fn cancel_and_wait(self) {
        self.shutdown_token.cancel();
        if let Err(e) = self.join_handle.await {
            tracing::warn!("Failed to join on shutdown handle: {}", e);
        }
    }
}

#[cfg(target_os = "ios")]
lazy_static! {
    static ref TUNNEL_SHUTDOWN_HANDLE: std::sync::Mutex<Option<ShutdownHandle>> =
        std::sync::Mutex::new(None);
}

async fn set_shutdown_handle(handle: Arc<Notify>) -> Result<(), FFIError> {
    let mut guard = VPN_SHUTDOWN_HANDLE.lock().await;
    if guard.is_some() {
        return Err(FFIError::VpnNotStopped);
    }
    *guard = Some(handle);

    Ok(())
}

pub(crate) fn uniffi_set_listener_status(status: StatusEvent) {
    let mut guard = LISTENER.lock().unwrap();
    if let Some(listener) = &mut *guard {
        match status {
            StatusEvent::Tun(status) => listener.on_tun_status_change(status),
            StatusEvent::Bandwidth(status) => listener.on_bandwidth_status_change(status),
            StatusEvent::NymVpn(status) => listener.on_nym_vpn_status_change(status),
            StatusEvent::Connection(status) => listener.on_connection_status_change(status),
            StatusEvent::Exit(status) => listener.on_exit_status_change(status),
        }
    }
}

async fn stop_and_reset_shutdown_handle() -> Result<(), FFIError> {
    debug!("Getting shutdown handle");
    let mut guard = VPN_SHUTDOWN_HANDLE.lock().await;
    if let Some(sh) = &*guard {
        debug!("notifying waiters");
        sh.notify_waiters();
        debug!("waiting for waiters to be notified");
        sh.notified().await;
        debug!("waiters notified");
    } else {
        return Err(FFIError::VpnNotStarted);
    }
    *guard = None;
    debug!("VPN shutdown handle reset");
    uniffi_set_listener_status(StatusEvent::Tun(TunStatus::Down));
    Ok(())
}

async fn reset_shutdown_handle() -> Result<(), FFIError> {
    let mut guard = VPN_SHUTDOWN_HANDLE.lock().await;
    *guard = None;
    debug!("VPN shutdown handle reset");
    Ok(())
}

#[cfg(not(target_os = "ios"))]
async fn _async_run_vpn(vpn: SpecificVpn) -> Result<(Arc<Notify>, NymVpnHandle), FFIError> {
    debug!("creating new stop handle");
    let stop_handle = Arc::new(Notify::new());
    debug!("new stop handle created");
    set_shutdown_handle(stop_handle.clone()).await?;
    debug!("shutdown handle set with new stop handle");
    let handle = spawn_nym_vpn(vpn)?;
    debug!("spawned vpn handle");
    Ok((stop_handle, handle))
}

async fn wait_for_shutdown(
    stop_handle: Arc<Notify>,
    handle: NymVpnHandle,
) -> crate::error::Result<()> {
    let NymVpnHandle {
        vpn_ctrl_tx,
        vpn_status_rx,
        vpn_exit_rx,
    } = handle;

    RUNTIME.spawn(async move {
        stop_handle.notified().await;
        vpn_ctrl_tx.send(NymVpnCtrlMessage::Stop)
    });

    RUNTIME.spawn(async move {
        VpnServiceStatusListener::new().start(vpn_status_rx).await;
    });

    match vpn_exit_rx.await? {
        NymVpnExitStatusMessage::Failed(error) => {
            debug!("received exit status message for vpn");
            RUNNING.store(false, Ordering::Relaxed);
            let error = error
                .downcast_ref::<NymVpnExitError>()
                .ok_or(crate::Error::StopError)?;
            uniffi_set_listener_status(StatusEvent::Exit(ExitStatus::Failed {
                error: error.to_string(),
            }));
            error!("Stopped Nym VPN with error: {:?}", error);
        }
        NymVpnExitStatusMessage::Stopped => {
            uniffi_set_listener_status(StatusEvent::Exit(ExitStatus::Stopped));
            debug!("Stopped Nym VPN")
        }
    }
    Ok(())
}

#[derive(uniffi::Record)]
pub struct VPNConfig {
    pub api_url: Url,
    pub vpn_api_url: Option<Url>,
    pub entry_gateway: EntryPoint,
    pub exit_router: ExitPoint,
    pub enable_two_hop: bool,
    #[cfg(target_os = "android")]
    pub tun_provider: Arc<dyn crate::AndroidTunProvider>,
    #[cfg(target_os = "ios")]
    pub tun_provider: Arc<dyn crate::OSTunProvider>,
    pub credential_data_path: Option<PathBuf>,
    pub tun_status_listener: Option<Arc<dyn TunnelStatusListener>>,
}

fn sync_run_vpn(config: VPNConfig) -> Result<NymVpn<MixnetVpn>, FFIError> {
    let mut vpn = NymVpn::new_mixnet_vpn(
        config.entry_gateway.into(),
        config.exit_router.into(),
        #[cfg(target_os = "android")]
        config.tun_provider,
        #[cfg(target_os = "ios")]
        config.tun_provider,
    );
    debug!("Created new mixnet vpn");
    vpn.generic_config.gateway_config.api_url = config.api_url;
    vpn.generic_config
        .data_path
        .clone_from(&config.credential_data_path);
    Ok(vpn)
}

#[allow(non_snake_case)]
#[uniffi::export]
pub fn initLogger(level: String) {
    #[cfg(target_os = "ios")]
    swift::init_logs(level);
    #[cfg(target_os = "android")]
    android::init_logs(level);
}

#[allow(non_snake_case)]
#[uniffi::export]
pub fn startVPN(config: VPNConfig) -> Result<(), FFIError> {
    if RUNNING.fetch_or(true, Ordering::Relaxed) {
        return Err(FFIError::VpnAlreadyRunning);
    }

    #[cfg(any(target_os = "ios", target_os = "macos"))]
    crate::platform::swift::init_logs();

    LISTENER
        .lock()
        .unwrap()
        .clone_from(&config.tun_status_listener);

    uniffi_set_listener_status(StatusEvent::Tun(TunStatus::InitializingClient));

    #[cfg(target_os = "ios")]
    {
        RUNTIME.block_on(async move {
            tracing::debug!("Starting VPN tunnel...");

            let shutdown_token = CancellationToken::new();
            let cloned_shutdown_token = shutdown_token.clone();
            let join_handle = tokio::spawn(async move {
                // todo: set this only when two hop tunnel is actually up.
                uniffi_set_listener_status(StatusEvent::Tun(TunStatus::Up));

                match TwoHopTunnel::start(config.tun_provider, cloned_shutdown_token).await {
                    Ok(()) => {
                        tracing::debug!("Tunnel has finished execution");
                    }
                    Err(e) => {
                        tracing::error!("Tunnel exited with error: {}", e);
                    }
                }

                uniffi_set_listener_status(StatusEvent::Tun(TunStatus::Down));
            });

            *TUNNEL_SHUTDOWN_HANDLE.lock().unwrap() = Some(ShutdownHandle {
                join_handle,
                shutdown_token,
            });
        });

        Ok(())
    }

    #[cfg(not(target_os = "ios"))]
    {
        debug!("Trying to run VPN");
        let vpn = sync_run_vpn(config);
        debug!("Got VPN");
        if vpn.is_err() {
            error!("Err creating VPN");
            uniffi_set_listener_status(StatusEvent::Tun(TunStatus::Down));
            RUNNING.store(false, Ordering::Relaxed);
        }
        let ret = RUNTIME.block_on(run_vpn(vpn?.into()));
        if ret.is_err() {
            error!("Error running VPN");
            uniffi_set_listener_status(StatusEvent::Tun(TunStatus::Down));
            RUNNING.store(false, Ordering::Relaxed);
        }
        ret
    }
}

#[allow(non_snake_case)]
#[uniffi::export]
pub fn importCredential(credential: String, path: String) -> Result<Option<SystemTime>, FFIError> {
    RUNTIME.block_on(import_credential_from_string(&credential, &path))
}

async fn import_credential_from_string(
    credential: &str,
    path: &str,
) -> Result<Option<SystemTime>, FFIError> {
    let path_result = PathBuf::from_str(path);
    let path_buf = match path_result {
        Ok(p) => p,
        Err(_) => return Err(FFIError::InvalidPath),
    };
    match import_credential_base58(credential, path_buf).await {
        Ok(time) => match time {
            None => Ok(None),
            Some(t) => Ok(Some(SystemTime::from(t))),
        },
        Err(_) => Err(FFIError::InvalidCredential),
    }
}

#[allow(non_snake_case)]
#[uniffi::export]
pub fn checkCredential(credential: String) -> Result<Option<SystemTime>, FFIError> {
    RUNTIME.block_on(check_credential_string(&credential))
}

async fn check_credential_string(credential: &str) -> Result<Option<SystemTime>, FFIError> {
    check_credential_base58(credential)
        .await
        .map_err(|_| FFIError::InvalidCredential)
}

#[cfg(not(target_os = "ios"))]
async fn run_vpn(vpn: SpecificVpn) -> Result<(), FFIError> {
    match _async_run_vpn(vpn).await {
        Err(err) => {
            debug!("Stopping and resetting shutdown handle");
            reset_shutdown_handle()
                .await
                .expect("Failed to reset shutdown handle");
            RUNNING.store(false, Ordering::Relaxed);
            error!("Could not start the VPN: {:?}", err);
            uniffi_set_listener_status(StatusEvent::Exit(ExitStatus::Failed {
                error: err.to_string(),
            }));
            uniffi_set_listener_status(StatusEvent::Tun(TunStatus::Down));
            Err(err)
        }
        Ok((stop_handle, handle)) => {
            debug!("Spawning wait for shutdown");
            RUNTIME.spawn(async move {
                wait_for_shutdown(stop_handle.clone(), handle)
                    .await
                    .map_err(|err| {
                        warn!("error during vpn run: {}", err);
                    })
                    .ok();
                stop_handle.notify_one();
            });
            Ok(())
        }
    }
}

#[allow(non_snake_case)]
#[uniffi::export]
pub fn stopVPN() -> Result<(), FFIError> {
    if !RUNNING.fetch_and(false, Ordering::Relaxed) {
        return Err(FFIError::VpnNotStarted);
    }
    uniffi_set_listener_status(StatusEvent::Tun(TunStatus::Disconnecting));
    debug!("Stopping VPN");

    #[cfg(not(target_os = "ios"))]
    RUNTIME.block_on(stop_vpn())?;

    #[cfg(target_os = "ios")]
    RUNTIME.block_on(async move {
        let shutdown_handle = TUNNEL_SHUTDOWN_HANDLE.lock().unwrap().take();
        if let Some(shutdown_handle) = shutdown_handle {
            shutdown_handle.cancel_and_wait().await;
        }
    });

    Ok(())
}

async fn stop_vpn() -> Result<(), FFIError> {
    debug!("Resetting shutdown handle");
    stop_and_reset_shutdown_handle().await
}

#[allow(non_snake_case)]
#[uniffi::export]
pub fn getGatewayCountries(
    api_url: Url,
    nym_vpn_api_url: Option<Url>,
    exit_only: bool,
    user_agent: Option<UserAgent>,
) -> Result<Vec<Location>, FFIError> {
    RUNTIME.block_on(get_gateway_countries(
        api_url,
        nym_vpn_api_url,
        exit_only,
        user_agent,
    ))
}

async fn get_gateway_countries(
    api_url: Url,
    nym_vpn_api_url: Option<Url>,
    exit_only: bool,
    user_agent: Option<UserAgent>,
) -> Result<Vec<Location>, FFIError> {
    let user_agent = user_agent
        .map(nym_sdk::UserAgent::from)
        .unwrap_or_else(|| nym_bin_common::bin_info!().into());
    let directory_config = nym_gateway_directory::Config {
        api_url,
        nym_vpn_api_url,
    };
    let directory_client = GatewayClient::new(directory_config, user_agent)?;
    let locations = if !exit_only {
        directory_client.lookup_entry_countries().await
    } else {
        directory_client.lookup_exit_countries().await
    }?;
    Ok(locations.into_iter().map(Location::from).collect())
}

#[allow(non_snake_case)]
#[uniffi::export]
pub fn getLowLatencyEntryCountry(
    api_url: Url,
    vpn_api_url: Option<Url>,
    harbour_master_url: Option<Url>,
) -> Result<Location, FFIError> {
    RUNTIME.block_on(get_low_latency_entry_country(
        api_url,
        vpn_api_url,
        harbour_master_url,
        None,
    ))
}

#[allow(non_snake_case)]
#[uniffi::export]
pub fn getLowLatencyEntryCountryUserAgent(
    api_url: Url,
    vpn_api_url: Option<Url>,
    harbour_master_url: Option<Url>,
    user_agent: UserAgent,
) -> Result<Location, FFIError> {
    RUNTIME.block_on(get_low_latency_entry_country(
        api_url,
        vpn_api_url,
        harbour_master_url,
        Some(user_agent),
    ))
}

async fn get_low_latency_entry_country(
    api_url: Url,
    vpn_api_url: Option<Url>,
    _harbour_master_url: Option<Url>,
    user_agent: Option<UserAgent>,
) -> Result<Location, FFIError> {
    let config = nym_gateway_directory::Config {
        api_url,
        nym_vpn_api_url: vpn_api_url,
    };
    let user_agent = user_agent
        .map(nym_sdk::UserAgent::from)
        .unwrap_or_else(|| nym_bin_common::bin_info!().into());
    let gateway_client = GatewayClient::new(config, user_agent)?;
    let gateway = gateway_client.lookup_low_latency_entry_gateway().await?;
    let country = gateway
        .location
        .ok_or(crate::Error::CountryCodeNotFound)?
        .into();

    Ok(country)
}

#[derive(uniffi::Record, Clone)]
pub struct TunnelConfig {
    pub private_key: PrivateKey,
    pub addresses: Vec<IpAddr>,
}

impl From<WgTunnelConfig> for TunnelConfig {
    fn from(value: WgTunnelConfig) -> Self {
        TunnelConfig {
            private_key: value.private_key,
            addresses: value.addresses,
        }
    }
}

#[derive(uniffi::Record, Clone)]
pub struct PeerConfig {
    pub public_key: PublicKey,
    pub allowed_ips: Vec<IpNetwork>,
    pub endpoint: SocketAddr,
    pub psk: Option<PresharedKey>,
}

impl From<WgPeerConfig> for PeerConfig {
    fn from(value: WgPeerConfig) -> Self {
        PeerConfig {
            public_key: value.public_key,
            allowed_ips: value.allowed_ips,
            endpoint: value.endpoint,
            psk: value.psk,
        }
    }
}

#[derive(uniffi::Record, Clone)]
pub struct WgConfig {
    pub tunnel: TunnelConfig,
    pub peers: Vec<PeerConfig>,
    pub ipv4_gateway: Ipv4Addr,
    pub ipv6_gateway: Option<Ipv6Addr>,
    pub mtu: u16,
}

impl From<talpid_wireguard::config::Config> for WgConfig {
    fn from(value: talpid_wireguard::config::Config) -> Self {
        WgConfig {
            tunnel: value.tunnel.into(),
            peers: value.peers.into_iter().map(Into::into).collect(),
            ipv4_gateway: value.ipv4_gateway,
            ipv6_gateway: value.ipv6_gateway,
            mtu: value.mtu,
        }
    }
}

#[derive(uniffi::Record, Clone)]
pub struct NymConfig {
    pub ipv4_addr: Ipv4Addr,
    pub ipv6_addr: Ipv6Addr,
    pub dns_ips: Vec<IpAddr>,
    pub mtu: u16,
    pub entry_mixnet_gateway_ip: Option<IpAddr>,
}

impl From<RoutingConfig> for NymConfig {
    fn from(value: RoutingConfig) -> Self {
        NymConfig {
            ipv4_addr: value.tun_ips().ipv4,
            ipv6_addr: value.tun_ips().ipv6,
            dns_ips: value.dns_ips.clone(),
            mtu: value.mtu(),
            entry_mixnet_gateway_ip: None,
        }
    }
}

#[uniffi::export(with_foreign)]
pub trait TunnelStatusListener: Send + Sync {
    fn on_tun_status_change(&self, status: TunStatus);
    fn on_bandwidth_status_change(&self, status: BandwidthStatus);
    fn on_connection_status_change(&self, status: ConnectionStatus);
    fn on_nym_vpn_status_change(&self, status: NymVpnStatus);
    fn on_exit_status_change(&self, status: ExitStatus);
}
