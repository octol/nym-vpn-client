// Copyright 2024 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: GPL-3.0-only

use maplit::hashmap;
use nym_vpn_proto::{error::ErrorType, Error as ProtoError};

use crate::service::{
    AccountNotReady, ConnectionFailedError, SetNetworkError, VpnServiceConnectError,
};

impl From<VpnServiceConnectError> for nym_vpn_proto::ConnectRequestError {
    fn from(err: VpnServiceConnectError) -> Self {
        match err {
            VpnServiceConnectError::Internal(ref _account_error) => {
                nym_vpn_proto::ConnectRequestError {
                    kind: nym_vpn_proto::connect_request_error::ConnectRequestErrorType::Internal
                        as i32,
                    message: err.to_string(),
                }
            }
            VpnServiceConnectError::Account(ref not_ready_to_connect) => {
                nym_vpn_proto::ConnectRequestError {
                    kind: nym_vpn_proto::connect_request_error::ConnectRequestErrorType::from(
                        not_ready_to_connect,
                    ) as i32,
                    message: not_ready_to_connect.to_string(),
                }
            }
            VpnServiceConnectError::Cancel => nym_vpn_proto::ConnectRequestError {
                kind: nym_vpn_proto::connect_request_error::ConnectRequestErrorType::Internal
                    as i32,
                message: err.to_string(),
            },
        }
    }
}

impl From<&AccountNotReady> for nym_vpn_proto::connect_request_error::ConnectRequestErrorType {
    fn from(not_ready: &AccountNotReady) -> Self {
        match not_ready {
            AccountNotReady::Pending => {
                nym_vpn_proto::connect_request_error::ConnectRequestErrorType::Pending
            }
            AccountNotReady::NoMnemonicStored => {
                nym_vpn_proto::connect_request_error::ConnectRequestErrorType::NoAccountStored
            }
            AccountNotReady::AccountNotActive => {
                nym_vpn_proto::connect_request_error::ConnectRequestErrorType::AccountNotActive
            }
            AccountNotReady::NoActiveSubscription => {
                nym_vpn_proto::connect_request_error::ConnectRequestErrorType::NoActiveSubscription
            }
            AccountNotReady::DeviceNotRegistered => {
                nym_vpn_proto::connect_request_error::ConnectRequestErrorType::DeviceNotRegistered
            }
            AccountNotReady::DeviceNotActive => {
                nym_vpn_proto::connect_request_error::ConnectRequestErrorType::DeviceNotActive
            }
        }
    }
}

impl From<ConnectionFailedError> for ProtoError {
    fn from(err: ConnectionFailedError) -> Self {
        match err {
            ConnectionFailedError::Unhandled(ref reason) => ProtoError {
                kind: ErrorType::Unhandled as i32,
                message: err.to_string(),
                details: hashmap! {
                    "reason".to_string() => reason.clone(),
                },
            },
            ConnectionFailedError::UnhandledExit(ref reason) => ProtoError {
                kind: ErrorType::UnhandledExit as i32,
                message: err.to_string(),
                details: hashmap! {
                    "reason".to_string() => reason.clone(),
                },
            },
            ConnectionFailedError::InternalError(ref reason) => ProtoError {
                kind: ErrorType::Internal as i32,
                message: err.to_string(),
                details: hashmap! {
                    "reason".to_string() => reason.clone(),
                },
            },
            ConnectionFailedError::InvalidCredential => ProtoError {
                kind: ErrorType::NoValidCredentials as i32,
                message: err.to_string(),
                details: Default::default(),
            },
            ConnectionFailedError::FailedToSetupMixnetStoragePaths { ref reason } => ProtoError {
                kind: ErrorType::MixnetStoragePaths as i32,
                message: err.to_string(),
                details: hashmap! {
                    "reason".to_string() => reason.to_string(),
                },
            },
            ConnectionFailedError::FailedToCreateMixnetClientWithDefaultStorage { ref reason } => {
                ProtoError {
                    kind: ErrorType::MixnetDefaultStorage as i32,
                    message: err.to_string(),
                    details: hashmap! {
                        "reason".to_string() => reason.to_string(),
                    },
                }
            }
            ConnectionFailedError::FailedToBuildMixnetClient { ref reason } => ProtoError {
                kind: ErrorType::MixnetBuildClient as i32,
                message: err.to_string(),
                details: hashmap! {
                    "reason".to_string() => reason.to_string(),
                },
            },
            ConnectionFailedError::FailedToConnectToMixnet { ref reason } => ProtoError {
                kind: ErrorType::MixnetConnect as i32,
                message: err.to_string(),
                details: hashmap! {
                    "reason".to_string() => reason.to_string(),
                },
            },
            ConnectionFailedError::FailedToConnectToMixnetEntryGateway {
                ref gateway_id,
                ref reason,
            } => ProtoError {
                kind: ErrorType::MixnetEntryGateway as i32,
                message: err.to_string(),
                details: hashmap! {
                    "gateway_id".to_string() => gateway_id.clone(),
                    "reason".to_string() => reason.to_string(),
                },
            },
            ConnectionFailedError::StartMixnetTimeout(timeout) => ProtoError {
                kind: ErrorType::MixnetTimeout as i32,
                message: timeout.to_string(),
                details: Default::default(),
            },
            ConnectionFailedError::FailedToSetupGatewayDirectoryClient {
                ref config,
                ref reason,
            } => ProtoError {
                kind: ErrorType::GatewayDirectory as i32,
                message: err.to_string(),
                details: hashmap! {
                    "config".to_string() => config.to_string(),
                    "reason".to_string() => reason.clone(),
                },
            },
            ConnectionFailedError::FailedToConnectToIpPacketRouter { ref reason } => ProtoError {
                kind: ErrorType::IprFailedToConnect as i32,
                message: err.to_string(),
                details: hashmap! {
                    "reason".to_string() => reason.to_string(),
                },
            },
            ConnectionFailedError::FailedToConnectToAuthenticator {
                ref gateway_id,
                ref authenticator_address,
                ref reason,
            } => ProtoError {
                kind: ErrorType::AuthenticatorFailedToConnect as i32,
                message: err.to_string(),
                details: hashmap! {
                    "gateway_id".to_string() => gateway_id.to_string(),
                    "authenticator_address".to_string() => authenticator_address.to_string(),
                    "reason".to_string() => reason.to_string(),
                },
            },
            ConnectionFailedError::TimeoutWaitingForConnectResponseFromAuthenticator {
                ref gateway_id,
                ref authenticator_address,
                ref reason,
            } => ProtoError {
                kind: ErrorType::AuthenticatorConnectTimeout as i32,
                message: err.to_string(),
                details: hashmap! {
                    "gateway_id".to_string() => gateway_id.to_string(),
                    "authenticator_address".to_string() => authenticator_address.to_string(),
                    "reason".to_string() => reason.to_string(),
                },
            },
            ConnectionFailedError::InvalidGatewayAuthResponse {
                ref gateway_id,
                ref authenticator_address,
                ref reason,
            } => ProtoError {
                kind: ErrorType::AuthenticatorInvalidResponse as i32,
                message: err.to_string(),
                details: hashmap! {
                    "gateway_id".to_string() => gateway_id.to_string(),
                    "authenticator_address".to_string() => authenticator_address.to_string(),
                    "reason".to_string() => reason.to_string(),
                },
            },
            ConnectionFailedError::AuthenticatorRegistrationDataVerificationFailed {
                ref reason,
            } => ProtoError {
                kind: ErrorType::AuthenticatorRegistrationDataVerification as i32,
                message: err.to_string(),
                details: hashmap! {
                    "reason".to_string() => reason.clone(),
                },
            },
            ConnectionFailedError::WgEntryGatewaySocketAddrFailedToParse { ref reason } => {
                ProtoError {
                    kind: ErrorType::AuthenticatorEntryGatewaySocketAddr as i32,
                    message: err.to_string(),
                    details: hashmap! {
                        "reason".to_string() => reason.clone(),
                    },
                }
            }
            ConnectionFailedError::WgEntryGatewayIpv4FailedToParse { ref reason } => ProtoError {
                kind: ErrorType::AuthenticatorEntryGatewayIpv4 as i32,
                message: err.to_string(),
                details: hashmap! {
                    "reason".to_string() => reason.clone(),
                },
            },
            ConnectionFailedError::AuthenticatorRespondedWithWrongVersion {
                ref expected,
                ref received,
                ref gateway_id,
                ref authenticator_address,
            } => ProtoError {
                kind: ErrorType::AuthenticatorWrongVersion as i32,
                message: err.to_string(),
                details: hashmap! {
                    "expected".to_string() => expected.to_string(),
                    "received".to_string() => received.to_string(),
                    "gateway_id".to_string() => gateway_id.to_string(),
                    "authenticator_address".to_string() => authenticator_address.to_string(),
                },
            },
            ConnectionFailedError::MailformedAuthenticatorReply {
                ref gateway_id,
                ref authenticator_address,
                ref reason,
            } => ProtoError {
                kind: ErrorType::AuthenticatorMalformedReply as i32,
                message: err.to_string(),
                details: hashmap! {
                    "gateway_id".to_string() => gateway_id.to_string(),
                    "authenticator_address".to_string() => authenticator_address.to_string(),
                    "reason".to_string() => reason.to_string(),
                },
            },
            ConnectionFailedError::AuthenticatorAddressNotFound { ref gateway_id } => ProtoError {
                kind: ErrorType::AuthenticatorAddressNotFound as i32,
                message: err.to_string(),
                details: hashmap! {
                    "gateway_id".to_string() => gateway_id.to_string(),
                },
            },
            ConnectionFailedError::AuthenticationNotPossible { ref reason } => ProtoError {
                kind: ErrorType::AuthenticatorAuthenticationNotPossible as i32,
                message: err.to_string(),
                details: hashmap! {
                    "reason".to_string() => reason.clone(),
                },
            },
            ConnectionFailedError::FailedToLookupGateways { ref reason } => ProtoError {
                kind: ErrorType::GatewayDirectoryLookupGateways as i32,
                message: err.to_string(),
                details: hashmap! {
                    "reason".to_string() => reason.clone(),
                },
            },
            ConnectionFailedError::FailedToLookupGatewayIdentity { ref reason } => ProtoError {
                kind: ErrorType::GatewayDirectoryLookupGatewayIdentity as i32,
                message: err.to_string(),
                details: hashmap! {
                    "reason".to_string() => reason.clone(),
                },
            },
            ConnectionFailedError::FailedToLookupRouterAddress { ref reason } => ProtoError {
                kind: ErrorType::GatewayDirectoryLookupRouterAddress as i32,
                message: err.to_string(),
                details: hashmap! {
                    "reason".to_string() => reason.clone(),
                },
            },
            ConnectionFailedError::FailedToLookupGatewayIp {
                ref gateway_id,
                ref reason,
            } => ProtoError {
                kind: ErrorType::GatewayDirectoryLookupIp as i32,
                message: err.to_string(),
                details: hashmap! {
                    "gateway_id".to_string() => gateway_id.to_string(),
                    "reason".to_string() => reason.clone(),
                },
            },
            ConnectionFailedError::FailedToSelectEntryGateway { ref reason } => ProtoError {
                kind: ErrorType::GatewayDirectoryEntry as i32,
                message: err.to_string(),
                details: hashmap! {
                    "reason".to_string() => reason.clone(),
                },
            },
            ConnectionFailedError::FailedToSelectExitGateway { ref reason } => ProtoError {
                kind: ErrorType::GatewayDirectoryExit as i32,
                message: err.to_string(),
                details: hashmap! {
                    "reason".to_string() => reason.clone(),
                },
            },
            ConnectionFailedError::FailedToSelectEntryGatewayIdNotFound { ref requested_id } => {
                ProtoError {
                    kind: ErrorType::GatewayDirectoryEntryId as i32,
                    message: err.to_string(),
                    details: hashmap! {
                        "requested_id".to_string() => requested_id.clone(),
                    },
                }
            }
            ConnectionFailedError::FailedToSelectEntryGatewayLocation {
                ref requested_location,
                ref available_countries,
            } => ProtoError {
                kind: ErrorType::GatewayDirectoryEntryLocation as i32,
                message: err.to_string(),
                details: hashmap! {
                    "requested_location".to_string() => requested_location.clone(),
                    "available_countries".to_string() => available_countries.join(", "),
                },
            },
            ConnectionFailedError::FailedToSelectExitGatewayLocation {
                ref requested_location,
                ref available_countries,
            } => ProtoError {
                kind: ErrorType::GatewayDirectoryExitLocation as i32,
                message: err.to_string(),
                details: hashmap! {
                    "requested_location".to_string() => requested_location.clone(),
                    "available_countries".to_string() => available_countries.join(", "),
                },
            },
            ConnectionFailedError::SameEntryAndExitGatewayFromCountry {
                ref requested_location,
            } => ProtoError {
                kind: ErrorType::GatewayDirectorySameEntryAndExitGw as i32,
                message: err.to_string(),
                details: hashmap! {
                    "requested_location".to_string() => requested_location.clone(),
                },
            },
            ConnectionFailedError::OutOfBandwidth {
                ref gateway_id,
                ref authenticator_address,
            } => ProtoError {
                kind: ErrorType::OutOfBandwidth as i32,
                message: err.to_string(),
                details: hashmap! {
                    "gateway_id".to_string() => gateway_id.to_string(),
                    "authenticator_address".to_string() => authenticator_address.to_string(),
                },
            },
            ConnectionFailedError::OutOfBandwidthWhenSettingUpTunnel {
                ref gateway_id,
                ref authenticator_address,
            } => ProtoError {
                kind: ErrorType::OutOfBandwidthWhenSettingUpTunnel as i32,
                message: err.to_string(),
                details: hashmap! {
                    "gateway_id".to_string() => gateway_id.to_string(),
                    "authenticator_address".to_string() => authenticator_address.to_string(),
                },
            },
            ConnectionFailedError::FailedToBringInterfaceUp {
                ref gateway_id,
                ref public_key,
                ref reason,
            } => ProtoError {
                kind: ErrorType::BringInterfaceUp as i32,
                message: err.to_string(),
                details: hashmap! {
                    "gateway_id".to_string() => gateway_id.to_string(),
                    "public_key".to_string() => public_key.clone(),
                    "reason".to_string() => reason.to_string(),
                },
            },
            ConnectionFailedError::FailedToInitFirewall { ref reason } => ProtoError {
                kind: ErrorType::FirewallInit as i32,
                message: err.to_string(),
                details: hashmap! {
                    "reason".to_string() => reason.to_string(),
                },
            },
            ConnectionFailedError::FailedToResetFirewallPolicy { ref reason } => ProtoError {
                kind: ErrorType::FirewallResetPolicy as i32,
                message: err.to_string(),
                details: hashmap! {
                    "reason".to_string() => reason.to_string(),
                },
            },
            ConnectionFailedError::FailedToInitDns { ref reason } => ProtoError {
                kind: ErrorType::DnsInit as i32,
                message: err.to_string(),
                details: hashmap! {
                    "reason".to_string() => reason.to_string(),
                },
            },
            ConnectionFailedError::FailedToSetDns { ref reason } => ProtoError {
                kind: ErrorType::DnsSet as i32,
                message: err.to_string(),
                details: hashmap! {
                    "reason".to_string() => reason.to_string(),
                },
            },
            ConnectionFailedError::FailedToFindTheDefaultInterface { ref reason } => ProtoError {
                kind: ErrorType::FindDefaultInterface as i32,
                message: err.to_string(),
                details: hashmap! {
                    "reason".to_string() => reason.to_string(),
                },
            },
            ConnectionFailedError::FailedToAddIpv6Route { ref reason } => ProtoError {
                kind: ErrorType::AddIpv6Route as i32,
                message: err.to_string(),
                details: hashmap! {
                    "reason".to_string() => reason.to_string(),
                },
            },
            ConnectionFailedError::TunError { ref reason } => ProtoError {
                kind: ErrorType::Tun as i32,
                message: err.to_string(),
                details: hashmap! {
                    "reason".to_string() => reason.to_string(),
                },
            },
            ConnectionFailedError::RoutingError { ref reason } => ProtoError {
                kind: ErrorType::Routing as i32,
                message: err.to_string(),
                details: hashmap! {
                    "reason".to_string() => reason.to_string(),
                },
            },
            ConnectionFailedError::WireguardConfigError { ref reason } => ProtoError {
                kind: ErrorType::WireguardConfig as i32,
                message: err.to_string(),
                details: hashmap! {
                    "reason".to_string() => reason.to_string(),
                },
            },
            ConnectionFailedError::MixnetConnectionMonitorError(ref reason) => ProtoError {
                kind: ErrorType::MixnetConnectionMonitor as i32,
                message: err.to_string(),
                details: hashmap! {
                    "reason".to_string() => reason.to_string(),
                },
            },
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum VpnCommandSendError {
    #[error("failed to send command to VPN service task")]
    Send,

    #[error("failed to receive response from VPN service task")]
    Receive,
}

impl From<VpnCommandSendError> for tonic::Status {
    fn from(err: VpnCommandSendError) -> Self {
        match err {
            VpnCommandSendError::Send | VpnCommandSendError::Receive => {
                tonic::Status::internal(err.to_string())
            }
        }
    }
}

impl From<SetNetworkError> for nym_vpn_proto::SetNetworkRequestError {
    fn from(err: SetNetworkError) -> Self {
        match err {
            SetNetworkError::NetworkNotFound(ref err) => nym_vpn_proto::SetNetworkRequestError {
                kind: nym_vpn_proto::set_network_request_error::SetNetworkRequestErrorType::InvalidNetworkName as i32,
                message: err.to_string(),
            },
            SetNetworkError::ReadConfig { .. } => nym_vpn_proto::SetNetworkRequestError {
                kind: nym_vpn_proto::set_network_request_error::SetNetworkRequestErrorType::Internal
                    as i32,
                message: err.to_string(),
            },
            SetNetworkError::WriteConfig { .. } => nym_vpn_proto::SetNetworkRequestError {
                kind: nym_vpn_proto::set_network_request_error::SetNetworkRequestErrorType::Internal
                    as i32,
                message: err.to_string(),
            },
        }
    }
}
