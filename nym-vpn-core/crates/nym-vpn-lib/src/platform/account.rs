// Copyright 2023 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: GPL-3.0-only

use std::{path::PathBuf, str::FromStr, sync::Arc, time::Duration};

use nym_vpn_account_controller::{AccountCommand, ReadyToConnect, SharedAccountState};
use nym_vpn_api_client::types::VpnApiAccount;
use nym_vpn_store::{keys::KeyStore, mnemonic::MnemonicStorage};
use tokio::{sync::mpsc::UnboundedSender, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::uniffi_custom_impls::AccountStateSummary;

use super::{error::VpnError, ACCOUNT_CONTROLLER_HANDLE};

pub(super) async fn start_account_controller_inner(data_dir: PathBuf) -> Result<(), VpnError> {
    let mut guard = ACCOUNT_CONTROLLER_HANDLE.lock().await;

    if guard.is_none() {
        let account_controller_handle = start_account_controller(data_dir).await?;
        *guard = Some(account_controller_handle);
        Ok(())
    } else {
        Err(VpnError::InvalidStateError {
            details: "Account controller is already running.".to_owned(),
        })
    }
}

pub(super) async fn stop_account_controller_inner() -> Result<(), VpnError> {
    let mut guard = ACCOUNT_CONTROLLER_HANDLE.lock().await;

    match guard.take() {
        Some(account_controller_handle) => {
            account_controller_handle.shutdown_and_wait().await;
            Ok(())
        }
        None => Err(VpnError::InvalidStateError {
            details: "Account controller is not running.".to_owned(),
        }),
    }
}

async fn start_account_controller(data_dir: PathBuf) -> Result<AccountControllerHandle, VpnError> {
    let storage = Arc::new(tokio::sync::Mutex::new(
        crate::storage::VpnClientOnDiskStorage::new(data_dir.clone()),
    ));
    // TODO: pass in as argument
    let user_agent = crate::util::construct_user_agent();
    let shutdown_token = CancellationToken::new();
    let account_controller = nym_vpn_account_controller::AccountController::new(
        Arc::clone(&storage),
        data_dir.clone(),
        user_agent,
        shutdown_token.child_token(),
    )
    .await
    .map_err(|err| VpnError::InternalError {
        details: err.to_string(),
    })?;

    let shared_account_state = account_controller.shared_state();
    let account_command_tx = account_controller.command_tx();
    let account_controller_handle = tokio::spawn(account_controller.run());

    Ok(AccountControllerHandle {
        command_sender: account_command_tx,
        shared_state: shared_account_state,
        handle: account_controller_handle,
        shutdown_token,
    })
}

pub(super) struct AccountControllerHandle {
    command_sender: UnboundedSender<AccountCommand>,
    shared_state: nym_vpn_account_controller::SharedAccountState,
    handle: JoinHandle<()>,
    shutdown_token: CancellationToken,
}

impl AccountControllerHandle {
    fn send_command(&self, command: AccountCommand) {
        if let Err(e) = self.command_sender.send(command) {
            tracing::error!("Failed to send comamnd: {}", e);
        }
    }

    async fn wait_for_ready_to_connect(&self, timeout: Duration) -> Option<ReadyToConnect> {
        self.shared_state.wait_for_ready_to_connect(timeout).await
    }

    async fn shutdown_and_wait(self) {
        self.shutdown_token.cancel();

        if let Err(e) = self.handle.await {
            tracing::error!("Failed to join on account controller handle: {}", e);
        }
    }
}

pub(super) async fn send_account_command(command: AccountCommand) -> Result<(), VpnError> {
    if let Some(guard) = &*ACCOUNT_CONTROLLER_HANDLE.lock().await {
        guard.send_command(command);
        Ok(())
    } else {
        Err(VpnError::InvalidStateError {
            details: "Account controller is not running.".to_owned(),
        })
    }
}

async fn get_shared_account_state() -> Result<SharedAccountState, VpnError> {
    if let Some(guard) = &*ACCOUNT_CONTROLLER_HANDLE.lock().await {
        Ok(guard.shared_state.clone())
    } else {
        Err(VpnError::InvalidStateError {
            details: "Account controller is not running.".to_owned(),
        })
    }
}

async fn wait_for_account_ready_to_connect(timeout: Duration) -> Result<ReadyToConnect, VpnError> {
    if let Some(guard) = &*ACCOUNT_CONTROLLER_HANDLE.lock().await {
        guard
            .wait_for_ready_to_connect(timeout)
            .await
            .ok_or(VpnError::AccountStatusUnknown)
    } else {
        Err(VpnError::InvalidStateError {
            details: "Account controller is not running.".to_owned(),
        })
    }
}

pub(super) async fn assert_account_ready_to_connect(timeout: Duration) -> Result<(), VpnError> {
    match wait_for_account_ready_to_connect(timeout).await? {
        ReadyToConnect::Ready => Ok(()),
        ReadyToConnect::NoMnemonicStored => Err(VpnError::NoAccountStored),
        ReadyToConnect::AccountNotActive => Err(VpnError::AccountNotActive),
        ReadyToConnect::NoActiveSubscription => Err(VpnError::NoActiveSubscription),
        ReadyToConnect::DeviceNotRegistered => Err(VpnError::AccountDeviceNotRegistered),
        ReadyToConnect::DeviceNotActive => Err(VpnError::AccountDeviceNotActive),
    }
}

fn setup_account_storage(path: &str) -> Result<crate::storage::VpnClientOnDiskStorage, VpnError> {
    let path = PathBuf::from_str(path).map_err(|err| VpnError::InternalError {
        details: err.to_string(),
    })?;
    Ok(crate::storage::VpnClientOnDiskStorage::new(path))
}

pub(super) async fn store_account_mnemonic(mnemonic: &str, path: &str) -> Result<(), VpnError> {
    // TODO: store the mnemonic by sending a command to the account controller instead of directly
    // interacting with the storage.

    let storage = setup_account_storage(path)?;

    let mnemonic = nym_vpn_store::mnemonic::Mnemonic::parse(mnemonic).map_err(|err| {
        VpnError::InternalError {
            details: err.to_string(),
        }
    })?;

    storage
        .store_mnemonic(mnemonic)
        .await
        .map_err(|err| VpnError::InternalError {
            details: err.to_string(),
        })?;

    Ok(())
}

pub(super) async fn is_account_mnemonic_stored(path: &str) -> Result<bool, VpnError> {
    // TODO: query the mnemonic by sending a command to the account controller instead of directly
    // interacting with the storage.

    let storage = setup_account_storage(path)?;
    storage
        .is_mnemonic_stored()
        .await
        .map_err(|err| VpnError::InternalError {
            details: err.to_string(),
        })
}

pub(super) async fn get_account_id(path: &str) -> Result<String, VpnError> {
    let storage = setup_account_storage(path)?;
    storage
        .load_mnemonic()
        .await
        .map(VpnApiAccount::from)
        .map(|account| account.id())
        .map_err(|err| VpnError::InternalError {
            details: err.to_string(),
        })
}

pub(super) async fn remove_account_mnemonic(path: &str) -> Result<bool, VpnError> {
    // TODO: remove the mnemonic by sending a command to the account controller instead of directly
    // interacting with the storage.

    let storage = setup_account_storage(path)?;
    let is_account_removed_success =
        storage
            .remove_mnemonic()
            .await
            .map(|_| true)
            .map_err(|err| VpnError::InternalError {
                details: err.to_string(),
            })?;

    Ok(is_account_removed_success)
}

pub(super) async fn reset_device_identity(path: &str) -> Result<(), VpnError> {
    let storage = setup_account_storage(path)?;
    storage
        .reset_keys(None)
        .await
        .map_err(|err| VpnError::InternalError {
            details: err.to_string(),
        })
}

pub(super) async fn update_account_state() -> Result<(), VpnError> {
    send_account_command(AccountCommand::UpdateAccountState).await
}

pub(super) async fn get_account_state() -> Result<AccountStateSummary, VpnError> {
    let shared_account_state = get_shared_account_state().await?;
    let account_state_summary = shared_account_state.lock().await.clone();
    Ok(AccountStateSummary::from(account_state_summary))
}
