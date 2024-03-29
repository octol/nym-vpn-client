use tonic::{transport::Server, Request, Response, Status};

use pb::{
    nym_vpn_service_server::{NymVpnService as PbNymVpnService, NymVpnServiceServer},
    PingRequest,
};

pub mod pb {
    tonic::include_proto!("nym.vpn");
}

#[derive(Debug, Default)]
pub struct VpnService {}

#[tonic::async_trait]
impl PbNymVpnService for VpnService {
    #[tracing::instrument]
    async fn ping(&self, request: Request<PingRequest>) -> Result<Response<pb::Empty>, Status> {
        tracing::info!("received request {:?}", request);

        let empty = pb::Empty {};
        Ok(Response::new(empty))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let filter = tracing_subscriber::EnvFilter::builder()
        .with_default_directive(tracing_subscriber::filter::LevelFilter::INFO.into())
        .from_env()
        .unwrap()
        .add_directive("hyper::proto=info".parse().unwrap())
        .add_directive("tonic=info".parse().unwrap());

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .compact()
        .init();

    let addr = "[::1]:4000".parse().unwrap();
    let vpn = VpnService::default();

    tracing::info!(message = "Starting server.", %addr);

    Server::builder()
        .trace_fn(|_| tracing::info_span!("vpn_service"))
        .add_service(NymVpnServiceServer::new(vpn))
        .serve(addr)
        .await?;

    Ok(())
}
