// Copyright 2024 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: GPL-3.0-only

#[derive(thiserror::Error, uniffi::Error, Debug, Clone, PartialEq)]
pub enum VpnError {
    #[error("{details}")]
    InternalError { details: String },

    #[error("{details}")]
    NetworkConnectionError { details: String },

    #[error("{details}")]
    GatewayError { details: String },

    #[error("{details}")]
    InvalidCredential { details: String },

    #[error("Client is out of bandwidth")]
    OutOfBandwidth,

    #[error("{details}")]
    InvalidStateError { details: String },

    #[error("account state is ready to connect")]
    AccountReady,

    #[error("no account stored")]
    NoAccountStored,

    #[error("account not active")]
    AccountNotActive,

    #[error("no active subscription")]
    NoActiveSubscription,

    #[error("device not registered")]
    AccountDeviceNotRegistered,

    #[error("device not active")]
    AccountDeviceNotActive,

    #[error("account status unknown")]
    AccountStatusUnknown,
}

impl From<nym_vpn_account_controller::ReadyToConnect> for VpnError {
    fn from(value: nym_vpn_account_controller::ReadyToConnect) -> Self {
        match value {
            nym_vpn_account_controller::ReadyToConnect::Ready => Self::AccountReady,
            nym_vpn_account_controller::ReadyToConnect::NoMnemonicStored => Self::NoAccountStored,
            nym_vpn_account_controller::ReadyToConnect::AccountNotActive => Self::AccountNotActive,
            nym_vpn_account_controller::ReadyToConnect::NoActiveSubscription => {
                Self::NoActiveSubscription
            }
            nym_vpn_account_controller::ReadyToConnect::DeviceNotRegistered => {
                Self::AccountDeviceNotRegistered
            }
            nym_vpn_account_controller::ReadyToConnect::DeviceNotActive => {
                Self::AccountDeviceNotActive
            }
        }
    }
}

impl From<crate::Error> for VpnError {
    fn from(value: crate::Error) -> Self {
        Self::InternalError {
            details: value.to_string(),
        }
    }
}

impl From<nym_gateway_directory::Error> for VpnError {
    fn from(value: nym_gateway_directory::Error) -> Self {
        Self::NetworkConnectionError {
            details: value.to_string(),
        }
    }
}

impl From<nym_vpn_api_client::VpnApiClientError> for VpnError {
    fn from(value: nym_vpn_api_client::VpnApiClientError) -> Self {
        Self::NetworkConnectionError {
            details: value.to_string(),
        }
    }
}
