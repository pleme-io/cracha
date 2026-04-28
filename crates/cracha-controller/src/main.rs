// crachá-controller — kube-rs reconciler binary.
//
// Watches AccessPolicy + ServiceCatalog CRDs cluster-wide; rebuilds
// the in-memory authz index on any change. Designed to be co-deployed
// with cracha-api (same Pod) so they share the index via Arc.
//
// In split-Pod deployments, this binary owns the index and exposes
// an internal HTTP /index endpoint that cracha-api polls — but that
// shape is not implemented here; one-Pod is the default.

use std::sync::Arc;

use clap::Parser;
use cracha_controller::{new_shared_index, reconcile, Context};
use kube::Client;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "cracha-controller")]
#[command(about = "Reconciles crachá AccessPolicy + ServiceCatalog CRDs", long_about = None)]
struct Args {
    #[arg(long, env = "CRACHA_CONTROLLER_METRICS_ADDR", default_value = "0.0.0.0:9100")]
    metrics_addr: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let _args = Args::parse();

    info!("cracha-controller starting");

    let client = Client::try_default().await?;
    let index = new_shared_index();
    let ctx = Arc::new(Context { client, index });

    reconcile::run(ctx).await;
    Ok(())
}
