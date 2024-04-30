use anyhow::{anyhow, Result};
use nym_vpn_proto::{
    health_check_response::ServingStatus, health_client::HealthClient,
    nym_vpnd_client::NymVpndClient, HealthCheckRequest,
};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tokio::sync::mpsc;
use tonic::{transport::Channel, Request};
use tracing::{debug, error, instrument, warn};
use ts_rs::TS;

use crate::events::AppHandleEventEmitter;

const VPND_SERVICE: &str = "nym.vpn.NymVpnd";

#[derive(Serialize, Deserialize, Default, Clone, Debug, TS)]
pub enum VpndStatus {
    Ok,
    #[default]
    NotOk,
}

#[derive(Debug, Default, Clone)]
pub struct GrpcClient {
    pub endpoint: String,
    status: ServingStatus,
}

impl GrpcClient {
    pub fn new(address: &str) -> Self {
        Self {
            endpoint: address.to_string(),
            status: ServingStatus::Unknown,
        }
    }

    /// Get the Vpnd service client
    #[instrument(skip_all)]
    pub async fn vpnd(&self) -> Result<NymVpndClient<Channel>> {
        NymVpndClient::connect(self.endpoint.clone())
            .await
            .inspect_err(|e| {
                warn!("failed to connect to the daemon: {:?}", e);
            })
            .map_err(|e| anyhow!("failed to connect to the daemon: {}", e))
    }

    /// Get the Health service client
    #[instrument(skip_all)]
    pub async fn health(&self) -> Result<HealthClient<Channel>> {
        HealthClient::connect(self.endpoint.clone())
            .await
            .inspect_err(|e| {
                warn!("failed to connect to the daemon: {:?}", e);
            })
            .map_err(|e| anyhow!("failed to connect to the daemon: {}", e))
    }

    /// Get latest reported connection status with the grpc server
    #[instrument(skip_all)]
    pub fn status(&self) -> VpndStatus {
        self.status.into()
    }

    /// Check the connection with the grpc server
    #[instrument(skip_all)]
    pub async fn check(&mut self) -> Result<VpndStatus> {
        let mut health = self.health().await?;

        let request = Request::new(HealthCheckRequest {
            service: VPND_SERVICE.into(),
        });
        let response = health
            .check(request)
            .await
            .inspect_err(|e| {
                error!("health check failed: {}", e);
            })?
            .into_inner();
        let status = response.status();
        self.status = status;

        Ok(status.into())
    }

    /// Watch the connection with the grpc server
    #[instrument(skip_all)]
    pub async fn watch(&mut self, app: &AppHandle) -> Result<()> {
        let mut health = self.health().await?;

        let request = Request::new(HealthCheckRequest {
            service: VPND_SERVICE.into(),
        });
        let mut stream = health
            .watch(request)
            .await
            .inspect_err(|e| {
                error!("health check failed: {}", e);
            })?
            .into_inner();

        let (tx, mut rx) = mpsc::channel(32);
        tokio::spawn(async move {
            loop {
                match stream.message().await {
                    Ok(Some(res)) => {
                        tx.send(res.status()).await.unwrap();
                    }
                    Ok(None) => {
                        warn!("watch health stream closed by the server");
                        tx.send(ServingStatus::NotServing).await.unwrap();
                        return;
                    }
                    Err(e) => {
                        warn!("watch health stream get a grpc error: {}", e);
                    }
                }
            }
        });

        while let Some(status) = rx.recv().await {
            debug!("health check status: {:?}", status);
            self.status = status;
            app.emit_vpnd_status(status.into());
        }

        Ok(())
    }
}

impl From<ServingStatus> for VpndStatus {
    fn from(status: ServingStatus) -> Self {
        match status {
            ServingStatus::Serving => VpndStatus::Ok,
            _ => VpndStatus::NotOk,
        }
    }
}
