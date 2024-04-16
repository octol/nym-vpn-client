use anyhow::anyhow;
use nym_gateway_directory::NodeIdentity;
use nym_vpn_lib::{gateway_directory::EntryPoint, nym_config::defaults::setup_env};
use rand::seq::IteratorRandom;
use std::path::PathBuf;
use tracing::*;

use nym_ip_pinger::PingResult;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if let Err(err) = run().await {
        error!("Exit with error: {err}");
        eprintln!("An error occurred: {err}");
        std::process::exit(1)
    }
    Ok(())
}

fn setup_logging() {
    let filter = tracing_subscriber::EnvFilter::builder()
        .with_default_directive(tracing_subscriber::filter::LevelFilter::INFO.into())
        .from_env()
        .unwrap()
        .add_directive("hyper::proto=info".parse().unwrap())
        .add_directive("netlink_proto=info".parse().unwrap());

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .compact()
        .init();
}

async fn run() -> anyhow::Result<PingResult> {
    setup_logging();
    debug!("{:?}", nym_vpn_lib::nym_bin_common::bin_info!());
    // mainnet by default
    setup_env::<PathBuf>(None);

    // Just pick a random gateway to probe.
    let gateway = fetch_random_gateway().await?;

    let result = nym_ip_pinger::probe(gateway).await;
    match result {
        Ok(ref result) => {
            println!("{:#?}", result);
        }
        Err(ref err) => {
            println!("Error: {err}");
        }
    };
    result
}

async fn fetch_random_gateway() -> anyhow::Result<EntryPoint> {
    let gateways = nym_ip_pinger::fetch_gateways().await?;
    let gateway = gateways
        .iter()
        .choose(&mut rand::thread_rng())
        .ok_or(anyhow!("No gateways returned by nym-api"))?;
    let identity = NodeIdentity::from_base58_string(gateway.identity_key())?;
    Ok(EntryPoint::Gateway { identity })
}
