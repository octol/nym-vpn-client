use super::super::tunnel_settings::TunnelNetworkSettings;
use crate::mobile::ios::default_path_observer::OSDefaultPathObserver;
use crate::platform::error::FFIError;
use std::sync::Arc;

#[uniffi::export(with_foreign)]
#[async_trait::async_trait]
pub trait OSTunProvider: Send + Sync + std::fmt::Debug {
    /// Set network settings including tun, dns, ip.
    async fn set_tunnel_network_settings(
        &self,
        tunnel_settings: TunnelNetworkSettings,
    ) -> std::result::Result<(), FFIError>;

    /// Set or unset the default path observer.
    fn set_default_path_observer(
        &self,
        observer: Option<Arc<dyn OSDefaultPathObserver>>,
    ) -> std::result::Result<(), FFIError>;
}
