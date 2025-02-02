// Copyright 2023-2024 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: GPL-3.0-only

mod error;

use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::PathBuf,
    str::FromStr,
    time::Duration,
};

pub use error::{Error, ErrorMessage};
use nym_authenticator_client::{AuthClient, ClientMessage};
use nym_authenticator_requests::v4::{
    registration::{FinalMessage, GatewayClient, InitMessage, RegistrationData},
    response::{
        AuthenticatorResponse, AuthenticatorResponseData, PendingRegistrationResponse,
        RegisteredResponse, RemainingBandwidthResponse, TopUpBandwidthResponse,
    },
    topup::TopUpMessage,
};
use nym_bandwidth_controller::PreparedCredential;
use nym_credentials_interface::{CredentialSpendingData, TicketType};
use nym_crypto::asymmetric::{encryption, x25519::KeyPair};
use nym_gateway_directory::Recipient;
use nym_node_requests::api::v1::gateway::client_interfaces::wireguard::models::PeerPublicKey;
use nym_pemstore::KeyPairPath;
use nym_sdk::mixnet::CredentialStorage;
use nym_validator_client::QueryHttpRpcNyxdClient;
use nym_wg_go::PublicKey;
use rand::{rngs::OsRng, CryptoRng, RngCore};
use tracing::{debug, error, info, warn};

use crate::error::Result;

const DEFAULT_PRIVATE_ENTRY_WIREGUARD_KEY_FILENAME: &str = "private_entry_wireguard.pem";
const DEFAULT_PUBLIC_ENTRY_WIREGUARD_KEY_FILENAME: &str = "public_entry_wireguard.pem";
const DEFAULT_PRIVATE_EXIT_WIREGUARD_KEY_FILENAME: &str = "private_exit_wireguard.pem";
const DEFAULT_PUBLIC_EXIT_WIREGUARD_KEY_FILENAME: &str = "public_exit_wireguard.pem";

pub const TICKETS_TO_SPEND: u32 = 1;
const RETRY_PERIOD: Duration = Duration::from_secs(30);

#[derive(Clone, Debug)]
pub struct GatewayData {
    pub public_key: PublicKey,
    pub endpoint: SocketAddr,
    pub private_ipv4: Ipv4Addr,
    pub private_ipv6: Ipv6Addr,
}
#[derive(Clone)]
pub struct WgGatewayLightClient {
    public_key: encryption::PublicKey,
    auth_client: AuthClient,
    auth_recipient: Recipient,
}

impl WgGatewayLightClient {
    pub fn auth_recipient(&self) -> Recipient {
        self.auth_recipient
    }

    pub async fn query_bandwidth(&mut self) -> Result<Option<i64>> {
        let query_message =
            ClientMessage::Query(PeerPublicKey::new(self.public_key.to_bytes().into()));
        let response = self
            .auth_client
            .send(query_message, self.auth_recipient)
            .await?;

        let remaining_bandwidth_data = match response.data {
            AuthenticatorResponseData::RemainingBandwidth(RemainingBandwidthResponse {
                reply: Some(remaining_bandwidth_data),
                ..
            }) => remaining_bandwidth_data,
            AuthenticatorResponseData::RemainingBandwidth(RemainingBandwidthResponse {
                reply: None,
                ..
            }) => return Ok(Some(0)),
            _ => return Err(Error::InvalidGatewayAuthResponse),
        };

        if remaining_bandwidth_data.available_bandwidth > 0 {
            let remaining_bi2 =
                si_scale::helpers::bibytes2(remaining_bandwidth_data.available_bandwidth as f64);

            info!(
                "Remaining wireguard bandwidth with gateway {} for today: {}",
                self.auth_recipient.gateway(),
                remaining_bi2
            );
        } else {
            info!(
                "Out of bandwidth with gateway {} for today",
                self.auth_recipient.gateway(),
            );
        }

        if remaining_bandwidth_data.available_bandwidth < 1024 * 1024 {
            warn!("Remaining bandwidth is under 1 MB. The wireguard mode will get suspended after that until tomorrow, UTC time. The client might shutdown with timeout soon");
        }
        Ok(Some(remaining_bandwidth_data.available_bandwidth))
    }

    pub async fn suspended(&mut self) -> Result<bool> {
        Ok(self.query_bandwidth().await?.is_none())
    }

    async fn send(&mut self, msg: ClientMessage) -> Result<AuthenticatorResponse> {
        if msg.is_wasteful() {
            let now = std::time::Instant::now();
            while now.elapsed() < RETRY_PERIOD {
                match self
                    .auth_client
                    .send(msg.clone(), self.auth_recipient)
                    .await
                {
                    Ok(response) => return Ok(response),
                    Err(nym_authenticator_client::Error::TimeoutWaitingForConnectResponse) => {
                        continue
                    }
                    Err(source) => return Err(Error::NoRetry { source }),
                }
            }
            Err(Error::NoRetry {
                source: nym_authenticator_client::Error::TimeoutWaitingForConnectResponse,
            })
        } else {
            Ok(self
                .auth_client
                .send(msg.clone(), self.auth_recipient)
                .await?)
        }
    }

    pub async fn top_up(&mut self, credential: CredentialSpendingData) -> Result<i64> {
        let top_up_message = ClientMessage::TopUp(Box::new(TopUpMessage {
            pub_key: PeerPublicKey::new(self.public_key.to_bytes().into()),
            credential,
        }));
        let response = self.send(top_up_message).await?;

        let remaining_bandwidth = match response.data {
            AuthenticatorResponseData::TopUpBandwidth(TopUpBandwidthResponse { reply, .. }) => {
                reply.available_bandwidth
            }
            _ => return Err(Error::InvalidGatewayAuthResponse),
        };

        Ok(remaining_bandwidth)
    }
}

pub struct WgGatewayClient {
    keypair: encryption::KeyPair,
    auth_client: AuthClient,
    auth_recipient: Recipient,
}

impl WgGatewayClient {
    pub fn light_client(&self) -> WgGatewayLightClient {
        WgGatewayLightClient {
            public_key: *self.keypair.public_key(),
            auth_client: self.auth_client.clone(),
            auth_recipient: self.auth_recipient,
        }
    }

    fn new_type(
        data_path: &Option<PathBuf>,
        auth_client: AuthClient,
        auth_recipient: Recipient,
        private_file_name: &str,
        public_file_name: &str,
    ) -> Self {
        let mut rng = OsRng;
        if let Some(data_path) = data_path {
            let paths = KeyPairPath::new(
                data_path.join(private_file_name),
                data_path.join(public_file_name),
            );
            let keypair = load_or_generate_keypair(&mut rng, paths);
            WgGatewayClient {
                keypair,
                auth_client,
                auth_recipient,
            }
        } else {
            WgGatewayClient {
                keypair: KeyPair::new(&mut rng),
                auth_client,
                auth_recipient,
            }
        }
    }

    pub fn new_entry(
        data_path: &Option<PathBuf>,
        auth_client: AuthClient,
        auth_recipient: Recipient,
    ) -> Self {
        Self::new_type(
            data_path,
            auth_client,
            auth_recipient,
            DEFAULT_PRIVATE_ENTRY_WIREGUARD_KEY_FILENAME,
            DEFAULT_PUBLIC_ENTRY_WIREGUARD_KEY_FILENAME,
        )
    }

    pub fn new_exit(
        data_path: &Option<PathBuf>,
        auth_client: AuthClient,
        auth_recipient: Recipient,
    ) -> Self {
        Self::new_type(
            data_path,
            auth_client,
            auth_recipient,
            DEFAULT_PRIVATE_EXIT_WIREGUARD_KEY_FILENAME,
            DEFAULT_PUBLIC_EXIT_WIREGUARD_KEY_FILENAME,
        )
    }

    pub fn keypair(&self) -> &encryption::KeyPair {
        &self.keypair
    }

    pub fn auth_recipient(&self) -> Recipient {
        self.auth_recipient
    }

    pub async fn request_bandwidth<St: CredentialStorage>(
        wg_gateway_client: &mut WgGatewayLightClient,
        controller: &nym_bandwidth_controller::BandwidthController<QueryHttpRpcNyxdClient, St>,
        ticketbook_type: TicketType,
    ) -> Result<PreparedCredential>
    where
        <St as CredentialStorage>::StorageError: Send + Sync + 'static,
    {
        let credential = controller
            .prepare_ecash_ticket(
                ticketbook_type,
                wg_gateway_client.auth_recipient().gateway().to_bytes(),
                TICKETS_TO_SPEND,
            )
            .await
            .map_err(|source| Error::GetTicket {
                ticketbook_type,
                source,
            })?;
        Ok(credential)
    }

    pub async fn register_wireguard<St: CredentialStorage>(
        &mut self,
        gateway_host: IpAddr,
        controller: &nym_bandwidth_controller::BandwidthController<QueryHttpRpcNyxdClient, St>,
        enable_credentials_mode: bool,
        ticketbook_type: TicketType,
    ) -> Result<GatewayData>
    where
        <St as CredentialStorage>::StorageError: Send + Sync + 'static,
    {
        debug!("Registering with the wg gateway...");
        let init_message = ClientMessage::Initial(InitMessage {
            pub_key: PeerPublicKey::new(self.keypair.public_key().to_bytes().into()),
        });
        let response = self
            .auth_client
            .send(init_message, self.auth_recipient)
            .await?;
        let registered_data = match response.data {
            AuthenticatorResponseData::PendingRegistration(PendingRegistrationResponse {
                reply:
                    RegistrationData {
                        nonce,
                        gateway_data,
                        ..
                    },
                ..
            }) => {
                // Unwrap since we have already checked that we have the keypair.
                debug!("Verifying data");
                gateway_data
                    .verify(self.keypair.private_key(), nonce)
                    .map_err(Error::VerificationFailed)?;

                let credential = if enable_credentials_mode {
                    let cred = Self::request_bandwidth(
                        &mut self.light_client(),
                        controller,
                        ticketbook_type,
                    )
                    .await?;
                    Some(cred.data)
                } else {
                    None
                };

                let finalized_message = ClientMessage::Final(Box::new(FinalMessage {
                    gateway_client: GatewayClient::new(
                        self.keypair.private_key(),
                        gateway_data.pub_key().inner(),
                        gateway_data.private_ips,
                        nonce,
                    ),
                    credential,
                }));
                let response = self.light_client().send(finalized_message).await?;
                let AuthenticatorResponseData::Registered(RegisteredResponse { reply, .. }) =
                    response.data
                else {
                    return Err(Error::InvalidGatewayAuthResponse);
                };
                reply
            }
            AuthenticatorResponseData::Registered(RegisteredResponse { reply, .. }) => reply,
            _ => return Err(Error::InvalidGatewayAuthResponse),
        };

        let gateway_data = GatewayData {
            public_key: PublicKey::from(registered_data.pub_key.to_bytes()),
            endpoint: SocketAddr::from_str(&format!(
                "{}:{}",
                gateway_host, registered_data.wg_port
            ))
            .map_err(Error::FailedToParseEntryGatewaySocketAddr)?,
            private_ipv4: registered_data.private_ips.ipv4,
            private_ipv6: registered_data.private_ips.ipv6,
        };

        Ok(gateway_data)
    }

    pub async fn top_up_wireguard<St: CredentialStorage>(
        wg_gateway_client: &mut WgGatewayLightClient,
        controller: &nym_bandwidth_controller::BandwidthController<QueryHttpRpcNyxdClient, St>,
        ticketbook_type: TicketType,
    ) -> Result<i64>
    where
        <St as CredentialStorage>::StorageError: Send + Sync + 'static,
    {
        let credential =
            Self::request_bandwidth(wg_gateway_client, controller, ticketbook_type).await?;
        let remaining_bandwidth = wg_gateway_client.top_up(credential.data).await?;

        Ok(remaining_bandwidth)
    }
}

fn load_or_generate_keypair<R: RngCore + CryptoRng>(rng: &mut R, paths: KeyPairPath) -> KeyPair {
    match nym_pemstore::load_keypair(&paths) {
        Ok(keypair) => keypair,
        Err(_) => {
            let keypair = KeyPair::new(rng);
            if let Err(e) = nym_pemstore::store_keypair(&keypair, &paths) {
                error!(
                    "could not store generated keypair at {:?} - {:?}; will use ephemeral keys",
                    paths, e
                );
            }
            keypair
        }
    }
}
