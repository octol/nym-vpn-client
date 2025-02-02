// Copyright 2024 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: GPL-3.0-only

#[cfg(unix)]
use std::os::fd::{IntoRawFd, OwnedFd, RawFd};
use std::{
    ffi::{c_char, c_void, CString},
    fmt,
};

use super::{
    uapi::UapiConfigBuilder, Error, LoggingCallback, PeerConfig, PeerEndpointUpdate, PrivateKey,
    Result,
};

/// Classic WireGuard interface configuration.
pub struct InterfaceConfig {
    pub listen_port: Option<u16>,
    pub private_key: PrivateKey,
    pub mtu: u16,
    #[cfg(target_os = "linux")]
    pub fwmark: Option<u32>,
}

impl fmt::Debug for InterfaceConfig {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut d = f.debug_struct("InterfaceConfig");
        d.field("listen_port", &self.listen_port)
            .field("private_key", &"(hidden)")
            .field("mtu", &self.mtu);
        #[cfg(target_os = "linux")]
        d.field("fwmark", &self.fwmark);
        d.finish()
    }
}

/// Classic WireGuard configuration.
#[derive(Debug)]
pub struct Config {
    pub interface: InterfaceConfig,
    pub peers: Vec<PeerConfig>,
}

impl Config {
    fn as_uapi_config(&self) -> Vec<u8> {
        let mut config_builder = UapiConfigBuilder::new();
        config_builder.add(
            "private_key",
            self.interface.private_key.to_bytes().as_ref(),
        );

        if let Some(listen_port) = self.interface.listen_port {
            config_builder.add("listen_port", listen_port.to_string().as_str());
        }

        #[cfg(target_os = "linux")]
        if let Some(fwmark) = self.interface.fwmark {
            config_builder.add("fwmark", fwmark.to_string().as_str());
        }

        if !self.peers.is_empty() {
            config_builder.add("replace_peers", "true");
            for peer in self.peers.iter() {
                peer.append_to(&mut config_builder);
            }
        }

        config_builder.into_bytes()
    }
}

/// Classic WireGuard tunnel.
#[derive(Debug)]
pub struct Tunnel {
    handle: i32,
}

impl Tunnel {
    /// Start new WireGuard tunnel
    pub fn start(
        config: Config,
        #[cfg(not(windows))] tun_fd: OwnedFd,
        #[cfg(windows)] interface_name: &str,
    ) -> Result<Self> {
        let settings =
            CString::new(config.as_uapi_config()).map_err(|_| Error::ConfigContainsNulByte)?;
        #[cfg(windows)]
        let interface_name =
            CString::new(interface_name).map_err(|_| Error::InterfaceNameContainsNulByte)?;
        let handle = unsafe {
            wgTurnOn(
                #[cfg(windows)]
                interface_name.as_ptr(),
                // note: not all platforms accept mtu = 0
                #[cfg(any(target_os = "linux", target_os = "macos"))]
                i32::from(config.interface.mtu),
                settings.as_ptr(),
                #[cfg(not(windows))]
                tun_fd.into_raw_fd(),
                wg_logger_callback,
                std::ptr::null_mut(),
            )
        };

        if handle >= 0 {
            Ok(Self { handle })
        } else {
            Err(Error::StartTunnel(handle))
        }
    }

    /// Stop the tunnel.
    pub fn stop(mut self) {
        tracing::info!("Stopping the wg tunnel");
        self.stop_inner();
    }

    /// Re-attach itself to the tun interface.
    ///
    /// Typically used on default route change.
    #[cfg(target_os = "ios")]
    pub fn bump_sockets(&mut self) {
        unsafe { wgBumpSockets(self.handle) }
    }

    /// Update the endpoints of peers matched by public key.
    pub fn update_peers(&mut self, peer_updates: &[PeerEndpointUpdate]) -> Result<()> {
        let mut config_builder = UapiConfigBuilder::new();
        for peer_update in peer_updates {
            peer_update.append_to(&mut config_builder);
        }
        let settings =
            CString::new(config_builder.into_bytes()).map_err(|_| Error::ConfigContainsNulByte)?;
        let ret_code = unsafe { wgSetConfig(self.handle, settings.as_ptr()) };

        if ret_code == 0 {
            Ok(())
        } else {
            Err(Error::SetUapiConfig(i64::from(ret_code)))
        }
    }

    fn stop_inner(&mut self) {
        if self.handle >= 0 {
            unsafe { wgTurnOff(self.handle) };
            self.handle = -1;
        }
    }
}

impl Drop for Tunnel {
    fn drop(&mut self) {
        self.stop_inner()
    }
}

extern "C" {
    // Start the tunnel.
    fn wgTurnOn(
        #[cfg(windows)] interface_name: *const c_char,
        #[cfg(any(target_os = "linux", target_os = "macos"))] mtu: i32,
        settings: *const c_char,
        #[cfg(not(windows))] fd: RawFd,
        logging_callback: LoggingCallback,
        logging_context: *mut c_void,
    ) -> i32;

    // Pass a handle that was created by wgTurnOn to stop a wireguard tunnel.
    fn wgTurnOff(handle: i32);

    // Returns the config of the WireGuard interface.
    #[allow(unused)]
    fn wgGetConfig(handle: i32) -> *mut c_char;

    // Sets the config of the WireGuard interface.
    fn wgSetConfig(handle: i32, settings: *const c_char) -> i32;

    // Frees a pointer allocated by the go runtime - useful to free return value of wgGetConfig
    #[allow(unused)]
    fn wgFreePtr(ptr: *mut c_void);

    // Re-attach wireguard-go to the tunnel interface.
    #[cfg(target_os = "ios")]
    fn wgBumpSockets(handle: i32);
}

/// Callback used by libwg to pass wireguard-go logs.
///
/// # Safety
/// Do not call this method directly.
#[doc(hidden)]
pub unsafe extern "system" fn wg_logger_callback(
    _log_level: u32,
    msg: *const c_char,
    _ctx: *mut c_void,
) {
    if !msg.is_null() {
        let str = std::ffi::CStr::from_ptr(msg).to_string_lossy();
        tracing::debug!("{}", str);
    }
}
