mod dns_handler;
mod firewall_handler;
mod route_handler;
mod states;
mod tun_ipv6;
mod tunnel;

use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use dns_handler::DnsHandler;
use firewall_handler::FirewallHandler;
use route_handler::RouteHandler;
use states::DisconnectedState;

use crate::GenericNymVpnConfig;

#[async_trait::async_trait]
trait TunnelStateHandler: Send {
    async fn handle_event(
        mut self: Box<Self>,
        shutdown_token: &CancellationToken,
        command_rx: &'async_trait mut mpsc::UnboundedReceiver<TunnelCommand>,
        shared_state: &'async_trait mut SharedState,
    ) -> NextTunnelState;
}

enum NextTunnelState {
    NewState((Box<dyn TunnelStateHandler>, TunnelState)),
    SameState(Box<dyn TunnelStateHandler>),
    Finished,
}

#[derive(Debug)]
pub enum TunnelCommand {
    Connect,
    Disconnect,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TunnelState {
    Disconnected,
    Connecting,
    Connected,
    Disconnecting {
        after_disconnect: ActionAfterDisconnect,
    },
    Error(ErrorStateReason),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ActionAfterDisconnect {
    Nothing,
    Reconnect,
    Error(ErrorStateReason),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ErrorStateReason {
    /// Issues related to firewall configuration.
    Firewall,

    /// Failure to configure routing.
    Routing,

    /// Failure to configure dns.
    Dns,

    /// Failure to configure tunnel device.
    TunDevice,

    /// Failure to establish mixnet connection.
    EstablishMixnetConnection,

    /// Tunnel went down at runtime.
    TunnelDown,
}

#[derive(Debug)]
pub enum TunnelEvent {
    NewState(TunnelState),
}

pub struct SharedState {
    route_handler: RouteHandler,
    firewall_handler: FirewallHandler,
    dns_handler: DnsHandler,
    config: GenericNymVpnConfig,
}

pub struct TunnelStateMachine {
    current_state_handler: Box<dyn TunnelStateHandler>,
    shared_state: SharedState,
    command_receiver: mpsc::UnboundedReceiver<TunnelCommand>,
    event_sender: mpsc::UnboundedSender<TunnelEvent>,
    shutdown_token: CancellationToken,
}

impl TunnelStateMachine {
    pub async fn spawn(
        command_receiver: mpsc::UnboundedReceiver<TunnelCommand>,
        event_sender: mpsc::UnboundedSender<TunnelEvent>,
        config: GenericNymVpnConfig,
        shutdown_token: CancellationToken,
    ) -> Result<JoinHandle<()>> {
        let (current_state_handler, _) = DisconnectedState::enter();

        let route_handler = RouteHandler::new()
            .await
            .map_err(Error::CreateRouteHandler)?;
        let dns_handler = DnsHandler::new(
            #[cfg(target_os = "linux")]
            &route_handler,
        )
        .await
        .map_err(Error::CreateDnsHandler)?;
        let firewall_handler = FirewallHandler::new().map_err(Error::CreateFirewallHandler)?;

        let shared_state = SharedState {
            route_handler,
            firewall_handler,
            dns_handler,
            config,
        };

        let tunnel_state_machine = Self {
            current_state_handler,
            shared_state,
            command_receiver,
            event_sender,
            shutdown_token,
        };

        Ok(tokio::spawn(tunnel_state_machine.run()))
    }

    async fn run(mut self) {
        loop {
            let next_state = self
                .current_state_handler
                .handle_event(
                    &self.shutdown_token,
                    &mut self.command_receiver,
                    &mut self.shared_state,
                )
                .await;

            match next_state {
                NextTunnelState::NewState((new_state_handler, new_state)) => {
                    self.current_state_handler = new_state_handler;

                    log::debug!("New tunnel state: {:?}", new_state);
                    let _ = self.event_sender.send(TunnelEvent::NewState(new_state));
                }
                NextTunnelState::SameState(same_state) => {
                    self.current_state_handler = same_state;
                }
                NextTunnelState::Finished => break,
            }
        }

        log::debug!("Tunnel state machine is exiting...");
        self.shared_state.route_handler.stop().await;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to create a route handler")]
    CreateRouteHandler(#[source] route_handler::Error),

    #[error("failed to create a dns handler")]
    CreateDnsHandler(#[source] dns_handler::Error),

    #[error("failed to create firewall handler")]
    CreateFirewallHandler(#[source] firewall_handler::Error),

    #[error("failed to create tunnel device")]
    CreateTunDevice(#[source] tun2::Error),

    #[error("failed to get tunnel device name")]
    GetTunDeviceName(#[source] tun2::Error),

    #[error("failed to set tunnel device ipv6 address")]
    SetTunDeviceIpv6Addr(#[source] std::io::Error),

    #[error("failed to add routes")]
    AddRoutes(#[source] route_handler::Error),

    #[error("failed to set dns")]
    SetDns(#[source] dns_handler::Error),

    #[error("failed to connect mixnet client")]
    ConnectMixnetClient(#[source] tunnel::Error),

    #[error("failed to connect mixnet tunnel")]
    ConnectMixnetTunnel(#[source] tunnel::Error),
}

impl Error {
    fn error_state_reason(&self) -> ErrorStateReason {
        match self {
            Self::CreateRouteHandler(_) | Self::AddRoutes(_) => ErrorStateReason::Routing,
            Self::CreateDnsHandler(_) | Self::SetDns(_) => ErrorStateReason::Dns,
            Self::CreateFirewallHandler(_) => ErrorStateReason::Firewall,
            Self::CreateTunDevice(_)
            | Self::GetTunDeviceName(_)
            | Self::SetTunDeviceIpv6Addr(_) => ErrorStateReason::TunDevice,
            Self::ConnectMixnetTunnel(_) | Self::ConnectMixnetClient(_) => {
                // todo: add detail
                ErrorStateReason::EstablishMixnetConnection
            }
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
