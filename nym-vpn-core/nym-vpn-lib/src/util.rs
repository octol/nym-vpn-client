// Copyright 2023 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: GPL-3.0-only

#[cfg(not(target_os = "ios"))]
use crate::tunnel_setup::WgTunnelSetup;
use crate::{error::*, NymVpnCtrlMessage};
use futures::{channel::mpsc, StreamExt};
use log::*;
#[cfg(not(target_os = "ios"))]
use talpid_routing::RouteManager;

#[cfg(not(target_os = "ios"))]
pub(crate) async fn wait_and_handle_interrupt(
    task_manager: &mut nym_task::TaskManager,
    #[cfg(not(target_os = "ios"))] route_manager: RouteManager,
    #[cfg(not(target_os = "ios"))] wireguard_waiting: Option<[WgTunnelSetup; 2]>,
) {
    wait_for_interrupt(task_manager).await;
    #[cfg(not(target_os = "ios"))]
    handle_interrupt(route_manager, wireguard_waiting).await;
    log::info!("Waiting for tasks to finish... (Press ctrl-c to force)");
    task_manager.wait_for_shutdown().await;
}

pub(crate) async fn wait_for_interrupt(task_manager: &mut nym_task::TaskManager) {
    let res = nym_task::wait_for_signal_and_error(task_manager).await;

    log::info!("Sending shutdown");
    task_manager.signal_shutdown().ok();

    if let Err(e) = res {
        error!("Could not wait for interrupts anymore - {e}. Shutting down the tunnel.");
    }
}

pub(crate) async fn wait_for_interrupt_and_signal(
    mut task_manager: Option<nym_task::TaskManager>,
    mut vpn_ctrl_rx: mpsc::UnboundedReceiver<NymVpnCtrlMessage>,
    #[cfg(not(target_os = "ios"))] route_manager: RouteManager,
    #[cfg(not(target_os = "ios"))] wireguard_waiting: Option<[WgTunnelSetup; 2]>,
) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    let task_manager_wait = async {
        if let Some(task_manager) = &mut task_manager {
            task_manager.wait_for_error().await
        } else {
            std::future::pending().await
        }
    };
    let res = tokio::select! {
        biased;
        message = vpn_ctrl_rx.next() => {
            log::debug!("Received message: {:?}", message);
            match message {
                Some(NymVpnCtrlMessage::Stop) => {
                    log::info!("Received stop message");
                }
                None => {
                    log::info!("Channel closed, stopping");
                }
            }
            Ok(())
        }
        Some(msg) = task_manager_wait => {
            log::info!("Task error: {:?}", msg);
            Err(msg)
        }
        _ = tokio::signal::ctrl_c() => {
            log::info!("Received SIGINT");
            Ok(())
        },
    };
    if let Some(mut task_manager) = task_manager {
        info!("Sending shutdown signal");
        task_manager.signal_shutdown().ok();

        #[cfg(not(target_os = "ios"))]
        handle_interrupt(route_manager, wireguard_waiting).await;

        info!("Waiting for tasks to finish... (Press ctrl-c to force)");
        task_manager.wait_for_shutdown().await;

        info!("Stopping mixnet client");
    }
    res
}

#[cfg_attr(target_os = "windows", allow(unused_mut))]
#[cfg(not(target_os = "ios"))]
pub(crate) async fn handle_interrupt(
    route_manager: RouteManager,
    wireguard_waiting: Option<[WgTunnelSetup; 2]>,
) {
    tokio::task::spawn_blocking(|| drop(route_manager))
        .await
        .ok();
    let Some(wireguard_waiting) = wireguard_waiting else {
        return;
    };
    let [entry, exit] = wireguard_waiting;

    entry.tunnel_close_tx.send(()).ok();
    exit.tunnel_close_tx.send(()).ok();

    let ret1 = entry.handle.await;
    let ret2 = exit.handle.await;
    if ret1.is_err() || ret2.is_err() {
        error!("Error on tunnel handle");
    }

    let ret1 = entry.receiver.await;
    let ret2 = exit.receiver.await;
    if ret1.is_err() || ret2.is_err() {
        error!("Error on signal handle");
    }
}

#[cfg(unix)]
pub fn unix_has_root(binary_name: &str) -> Result<()> {
    if nix::unistd::geteuid().is_root() {
        debug!("Root privileges acquired");
        Ok(())
    } else {
        Err(Error::RootPrivilegesRequired {
            binary_name: binary_name.to_string(),
        })
    }
}

#[cfg(windows)]
pub fn win_has_admin(binary_name: &str) -> Result<()> {
    if is_elevated::is_elevated() {
        debug!("Admin privileges acquired");
        Ok(())
    } else {
        Err(Error::AdminPrivilegesRequired {
            binary_name: binary_name.to_string(),
        })
    }
}
